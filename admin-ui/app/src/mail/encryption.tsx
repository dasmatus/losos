/* OpenPGP in the Mail screen: the person's key (make, import, back up,
 * remove), the passphrase prompt, the key every recipient has, and the line
 * under a message that says how it came. The cryptography is ./pgp.ts; the
 * box's half is backend/src/mail_pgp.rs. */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  Alert01Icon,
  AlertDiamondIcon,
  CheckmarkBadge01Icon,
  Download04Icon,
  Key01Icon,
  SquareLock02Icon,
  SquareUnlock02Icon,
} from "@hugeicons/core-free-icons";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Dialog, DialogBody, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Field, FieldDescription, FieldError, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";
import { toast } from "@/components/ui/toast";
import {
  deleteMailKey,
  findMailKeys,
  getMailKey,
  putMailKey,
  type MailFoundKey,
  type MailKeySource,
  type MailOwnKey,
} from "@/lib/api";
import { useLocale, useT } from "@/lib/i18n-react";
import type { MessageKey } from "@/lib/i18n";
import { cn } from "@/lib/utils";
import {
  grouped,
  importKey,
  ImportProblem,
  keyFacts,
  lockKey,
  makeKey,
  MIN_PASSPHRASE,
  onKeyChange,
  unlockedKey,
  unlockKey,
  WrongPassphrase,
  type Check,
} from "./pgp";
import type * as OpenPGP from "openpgp";

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/* ── The person's key ─────────────────────────────────────────────────── */

export type OwnKey = { state: "loading" } | { state: "none" } | { state: "failed" } | { state: "set"; key: MailOwnKey };

export function useOwnKey(): [OwnKey, (next: MailOwnKey | null) => void] {
  const [own, setOwn] = React.useState<OwnKey>({ state: "loading" });
  React.useEffect(() => {
    const controller = new AbortController();
    getMailKey({ signal: controller.signal })
      .then(({ key }) => setOwn(key === null ? { state: "none" } : { state: "set", key }))
      .catch(() => {
        if (!controller.signal.aborted) setOwn({ state: "failed" });
      });
    return () => controller.abort();
  }, []);
  const set = React.useCallback((next: MailOwnKey | null) => {
    setOwn(next === null ? { state: "none" } : { state: "set", key: next });
  }, []);
  return [own, set];
}

/** The unlocked private key for `own`, kept in step with Lock and Unlock. */
export function useUnlocked(own: OwnKey): OpenPGP.PrivateKey | null {
  const fingerprint = own.state === "set" ? own.key.fingerprint : null;
  const [, tick] = React.useReducer((n: number) => n + 1, 0);
  React.useEffect(() => onKeyChange(tick), []);
  return unlockedKey(fingerprint);
}

function download(name: string, text: string) {
  const url = URL.createObjectURL(new Blob([text], { type: "application/pgp-keys" }));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  document.body.append(a);
  a.click();
  a.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 1000);
}

/* Two fields for a new passphrase, checked as they are typed. */
function NewPassphrase({
  id,
  value,
  again,
  onValue,
  onAgain,
  optional = false,
}: {
  id: string;
  value: string;
  again: string;
  onValue: (v: string) => void;
  onAgain: (v: string) => void;
  optional?: boolean;
}) {
  const t = useT();
  const short = value !== "" && value.length < MIN_PASSPHRASE;
  const differs = again !== "" && again !== value;
  return (
    <>
      <Field invalid={short}>
        <FieldLabel htmlFor={`${id}-pass`}>{optional ? t("mail.pgp.newPassphraseOptional") : t("mail.pgp.passphrase")}</FieldLabel>
        <Input
          id={`${id}-pass`}
          type="password"
          autoComplete="new-password"
          value={value}
          onChange={(e) => onValue(e.target.value)}
        />
        {short ? (
          <FieldError>{t("mail.pgp.tooShort", { n: MIN_PASSPHRASE })}</FieldError>
        ) : (
          <FieldDescription>{t("mail.pgp.passphraseHelp")}</FieldDescription>
        )}
      </Field>
      <Field invalid={differs}>
        <FieldLabel htmlFor={`${id}-again`}>{t("mail.pgp.again")}</FieldLabel>
        <Input
          id={`${id}-again`}
          type="password"
          autoComplete="new-password"
          value={again}
          onChange={(e) => onAgain(e.target.value)}
        />
        {differs && <FieldError>{t("mail.pgp.differs")}</FieldError>}
      </Field>
    </>
  );
}

const SEALED: Record<"tpm2" | "host", MessageKey> = {
  tpm2: "mail.pgp.sealedTpm",
  host: "mail.pgp.sealedHost",
};

/** Encryption: make or import a key, or see, back up, lock and remove it. */
export function KeyDialog({
  own,
  address,
  name,
  onOwn,
  onClose,
}: {
  own: OwnKey;
  address: string;
  name: string;
  onOwn: (key: MailOwnKey | null) => void;
  onClose: () => void;
}) {
  const t = useT();
  const locale = useLocale();
  const unlocked = useUnlocked(own);
  const [tab, setTab] = React.useState("make");
  const [pass, setPass] = React.useState("");
  const [again, setAgain] = React.useState("");
  const [armored, setArmored] = React.useState("");
  const [current, setCurrent] = React.useState("");
  const [busy, setBusy] = React.useState(false);
  const [problem, setProblem] = React.useState<string | null>(null);
  const [confirmRemove, setConfirmRemove] = React.useState(false);

  const newOk = pass.length >= MIN_PASSPHRASE && pass === again;
  const importOk =
    armored.includes("PRIVATE KEY BLOCK") && (pass === "" ? again === "" : newOk);

  const work = async (task: () => Promise<void>) => {
    setBusy(true);
    setProblem(null);
    try {
      await task();
    } catch (error) {
      if (error instanceof ImportProblem) {
        const words: Record<ImportProblem["kind"], string> = {
          notPrivate: t("mail.pgp.import.notPrivate"),
          wrongPassphrase: t("mail.pgp.wrongPassphrase"),
          otherAddress: t("mail.pgp.import.otherAddress", { address, other: error.detail || "?" }),
          noEncryption: t("mail.pgp.import.noEncryption"),
        };
        setProblem(words[error.kind]);
      } else {
        setProblem(message(error));
      }
    } finally {
      setBusy(false);
    }
  };

  const keep = async (pair: { publicKey: string; lockedKey: string }) => {
    const { key } = await putMailKey(pair.publicKey, pair.lockedKey);
    onOwn(key);
    setPass("");
    setAgain("");
    setArmored("");
    setCurrent("");
    toast.done(t("mail.pgp.done.set"));
  };

  const readFile = async (file: File | undefined) => {
    if (file === undefined) return;
    if (file.size > 512 * 1024) {
      setProblem(t("mail.pgp.import.notPrivate"));
      return;
    }
    setArmored(await file.text());
  };

  return (
    <Dialog
      open
      onOpenChange={(next) => {
        if (!next && !busy) onClose();
      }}
      labelledBy="mail-key-title"
      dialogClassName="w-[min(36rem,calc(100vw-1rem))]"
    >
      <DialogHeader>
        <DialogTitle id="mail-key-title">{t("mail.pgp.title")}</DialogTitle>
        <DialogDescription>{t("mail.pgp.summary")}</DialogDescription>
      </DialogHeader>
      <DialogBody className="flex flex-col gap-4">
        {own.state === "loading" && (
          <div className="grid place-items-center p-6">
            <Spinner />
          </div>
        )}
        {own.state === "failed" && (
          <Alert variant="crit" role="alert">
            <HugeiconsIcon icon={Alert01Icon} strokeWidth={1.5} aria-hidden="true" />
            <AlertDescription>{t("mail.pgp.loadFailed")}</AlertDescription>
          </Alert>
        )}
        {own.state === "none" && (
          <Tabs value={tab} onValueChange={setTab}>
            <TabsList>
              <TabsTrigger value="make">{t("mail.pgp.tab.make")}</TabsTrigger>
              <TabsTrigger value="import">{t("mail.pgp.tab.import")}</TabsTrigger>
            </TabsList>
            <TabsContent value="make" className="flex flex-col gap-3">
              <p className="text-[13px] text-muted">{t("mail.pgp.make.text", { address })}</p>
              <NewPassphrase id="mail-key-make" value={pass} again={again} onValue={setPass} onAgain={setAgain} />
            </TabsContent>
            <TabsContent value="import" className="flex flex-col gap-3">
              <Field>
                <FieldLabel htmlFor="mail-key-armored">{t("mail.pgp.import.key")}</FieldLabel>
                <Textarea
                  id="mail-key-armored"
                  value={armored}
                  onChange={(e) => setArmored(e.target.value)}
                  rows={5}
                  spellCheck={false}
                  className="font-mono text-[12px]"
                  placeholder="-----BEGIN PGP PRIVATE KEY BLOCK-----"
                />
                <FieldDescription>
                  {t("mail.pgp.import.help")}{" "}
                  <label className="cursor-pointer text-accent underline-offset-2 hover:underline">
                    {t("mail.pgp.import.file")}
                    <input
                      type="file"
                      accept=".asc,.gpg,.key,.txt,application/pgp-keys"
                      className="sr-only"
                      onChange={(e) => void readFile(e.target.files?.[0])}
                    />
                  </label>
                </FieldDescription>
              </Field>
              <Field>
                <FieldLabel htmlFor="mail-key-current">{t("mail.pgp.import.current")}</FieldLabel>
                <Input
                  id="mail-key-current"
                  type="password"
                  autoComplete="off"
                  value={current}
                  onChange={(e) => setCurrent(e.target.value)}
                />
              </Field>
              <NewPassphrase
                id="mail-key-import"
                value={pass}
                again={again}
                onValue={setPass}
                onAgain={setAgain}
                optional
              />
            </TabsContent>
          </Tabs>
        )}
        {own.state === "set" && (
          <>
            <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 text-sm">
              <dt className="text-muted">{t("mail.pgp.fingerprint")}</dt>
              <dd className="font-mono text-[13px] break-all" data-testid="mail-key-fingerprint">
                {grouped(own.key.fingerprint)}
              </dd>
              <dt className="text-muted">{t("mail.pgp.address")}</dt>
              <dd className="break-all">{own.key.userIds.join(", ")}</dd>
              <dt className="text-muted">{t("mail.pgp.made")}</dt>
              <dd>
                {new Intl.DateTimeFormat(locale, { dateStyle: "medium" }).format(new Date(own.key.created * 1000))}
              </dd>
            </dl>
            <ul className="flex flex-col gap-1.5 text-[13px] text-muted">
              <li className="flex gap-2">
                <HugeiconsIcon icon={SquareLock02Icon} size={16} strokeWidth={1.5} className="mt-px shrink-0 text-ok" aria-hidden="true" />
                {own.key.sealed !== null ? t(SEALED[own.key.sealed]) : t("mail.pgp.sealedNone")}
              </li>
              <li className="flex gap-2">
                <HugeiconsIcon icon={Key01Icon} size={16} strokeWidth={1.5} className="mt-px shrink-0 text-accent" aria-hidden="true" />
                <span className="min-w-0 break-words">{t("mail.pgp.published")}</span>
              </li>
            </ul>
            <div className="flex flex-wrap gap-2">
              {unlocked !== null ? (
                <Button variant="secondary" size="sm" onClick={lockKey}>
                  <HugeiconsIcon icon={SquareLock02Icon} strokeWidth={1.5} aria-hidden="true" />
                  {t("mail.pgp.lock")}
                </Button>
              ) : (
                <UnlockButton own={own.key} />
              )}
              <Button
                variant="secondary"
                size="sm"
                onClick={() => download(`${address}-private-locked.asc`, own.key.lockedKey)}
              >
                <HugeiconsIcon icon={Download04Icon} strokeWidth={1.5} aria-hidden="true" />
                {t("mail.pgp.backup")}
              </Button>
              <Button variant="ghost" size="sm" onClick={() => download(`${address}.asc`, own.key.publicKey)}>
                {t("mail.pgp.publicDownload")}
              </Button>
            </div>
            <p className="text-[12px] text-muted">{t("mail.pgp.backupHelp")}</p>
            {confirmRemove ? (
              <Alert variant="crit" role="alert">
                <HugeiconsIcon icon={AlertDiamondIcon} strokeWidth={1.5} aria-hidden="true" />
                <AlertTitle>{t("mail.pgp.remove.title")}</AlertTitle>
                <AlertDescription>
                  <p>{t("mail.pgp.remove.text")}</p>
                  <div className="mt-2 flex gap-2">
                    <Button
                      size="sm"
                      variant="destructive"
                      disabled={busy}
                      onClick={() =>
                        void work(async () => {
                          await deleteMailKey();
                          lockKey();
                          onOwn(null);
                          setConfirmRemove(false);
                          toast.done(t("mail.pgp.done.removed"));
                        })
                      }
                    >
                      {t("mail.pgp.remove.confirm")}
                    </Button>
                    <Button size="sm" variant="ghost" onClick={() => setConfirmRemove(false)}>
                      {t("mail.pgp.cancel")}
                    </Button>
                  </div>
                </AlertDescription>
              </Alert>
            ) : (
              <div>
                <Button variant="link" size="xs" className="px-0 text-crit" onClick={() => setConfirmRemove(true)}>
                  {t("mail.pgp.remove.button")}
                </Button>
              </div>
            )}
          </>
        )}
        {problem !== null && (
          <Alert variant="crit" role="alert">
            <HugeiconsIcon icon={Alert01Icon} strokeWidth={1.5} aria-hidden="true" />
            <AlertDescription>{problem}</AlertDescription>
          </Alert>
        )}
      </DialogBody>
      <DialogFooter>
        <Button variant="ghost" onClick={onClose} disabled={busy}>
          {own.state === "none" ? t("mail.pgp.cancel") : t("mail.pgp.close")}
        </Button>
        {own.state === "none" && tab === "make" && (
          <Button
            disabled={busy || !newOk}
            onClick={() => void work(async () => keep(await makeKey(name, address, pass)))}
          >
            {busy ? <Spinner className="size-4" /> : <HugeiconsIcon icon={Key01Icon} strokeWidth={1.5} aria-hidden="true" />}
            {t("mail.pgp.make.button")}
          </Button>
        )}
        {own.state === "none" && tab === "import" && (
          <Button
            disabled={busy || !importOk}
            onClick={() => void work(async () => keep(await importKey(armored, current, pass, address)))}
          >
            {busy ? <Spinner className="size-4" /> : <HugeiconsIcon icon={Key01Icon} strokeWidth={1.5} aria-hidden="true" />}
            {t("mail.pgp.import.button")}
          </Button>
        )}
      </DialogFooter>
    </Dialog>
  );
}

/* ── Unlocking ────────────────────────────────────────────────────────── */

/** Ask for the passphrase and unlock the key for this tab. */
export function UnlockDialog({ own, onClose }: { own: MailOwnKey; onClose: (unlocked: boolean) => void }) {
  const t = useT();
  const [pass, setPass] = React.useState("");
  const [busy, setBusy] = React.useState(false);
  const [wrong, setWrong] = React.useState(false);
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setWrong(false);
    try {
      await unlockKey(own.lockedKey, pass);
      onClose(true);
    } catch (error) {
      if (error instanceof WrongPassphrase) setWrong(true);
      else toast.error(t("mail.pgp.unlockFailed"), message(error), { help: "mail" });
      setBusy(false);
    }
  };
  return (
    <Dialog
      open
      onOpenChange={(next) => {
        if (!next && !busy) onClose(false);
      }}
      labelledBy="mail-unlock-title"
      dialogClassName="w-[min(26rem,calc(100vw-1rem))]"
    >
      <form onSubmit={(e) => void submit(e)}>
        <DialogHeader>
          <DialogTitle id="mail-unlock-title">{t("mail.pgp.unlock.title")}</DialogTitle>
          <DialogDescription>{t("mail.pgp.unlock.text")}</DialogDescription>
        </DialogHeader>
        <DialogBody>
          <Field invalid={wrong}>
            <FieldLabel htmlFor="mail-unlock-pass">{t("mail.pgp.passphrase")}</FieldLabel>
            <Input
              id="mail-unlock-pass"
              type="password"
              autoComplete="current-password"
              autoFocus
              value={pass}
              onChange={(e) => setPass(e.target.value)}
            />
            {wrong && <FieldError>{t("mail.pgp.wrongPassphrase")}</FieldError>}
          </Field>
        </DialogBody>
        <DialogFooter>
          <Button type="button" variant="ghost" onClick={() => onClose(false)} disabled={busy}>
            {t("mail.pgp.cancel")}
          </Button>
          <Button type="submit" disabled={busy || pass === ""}>
            {busy ? <Spinner className="size-4" /> : <HugeiconsIcon icon={SquareUnlock02Icon} strokeWidth={1.5} aria-hidden="true" />}
            {t("mail.pgp.unlock.button")}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}

function UnlockButton({ own }: { own: MailOwnKey }) {
  const t = useT();
  const [asking, setAsking] = React.useState(false);
  return (
    <>
      <Button variant="secondary" size="sm" onClick={() => setAsking(true)}>
        <HugeiconsIcon icon={SquareUnlock02Icon} strokeWidth={1.5} aria-hidden="true" />
        {t("mail.pgp.unlock.button")}
      </Button>
      {asking && <UnlockDialog own={own} onClose={() => setAsking(false)} />}
    </>
  );
}

/* ── Recipients' keys ─────────────────────────────────────────────────── */

export type Recipient =
  | { state: "looking" }
  | { state: "none" }
  | { state: "found"; key: MailFoundKey; fingerprint: string };

/** The key mail to each of `addresses` can be encrypted with: asked of the
 *  box once per address, checked here, and kept while the page is open. */
const known = new Map<string, Promise<Recipient>>();

async function lookUp(address: string): Promise<Recipient> {
  try {
    const { keys } = await findMailKeys(address);
    for (const key of keys) {
      const facts = await keyFacts(key.publicKey);
      if (facts?.canEncrypt === true && (key.source === "autocrypt" || facts.emails.includes(address))) {
        return { state: "found", key, fingerprint: facts.fingerprint };
      }
    }
    return { state: "none" };
  } catch {
    known.delete(address);
    return { state: "none" };
  }
}

export function recipientKey(address: string): Promise<Recipient> {
  const a = address.trim().toLowerCase();
  let p = known.get(a);
  if (p === undefined) {
    p = lookUp(a);
    known.set(a, p);
  }
  return p;
}

/** Forget what was found for an address (a key changed). */
export function forgetRecipient(address: string): void {
  known.delete(address.trim().toLowerCase());
}

export function useRecipients(addresses: readonly string[]): ReadonlyMap<string, Recipient> {
  const [found, setFound] = React.useState<ReadonlyMap<string, Recipient>>(new Map());
  const key = addresses.map((a) => a.toLowerCase()).join(",");
  React.useEffect(() => {
    let live = true;
    const list = key === "" ? [] : key.split(",");
    const timer = window.setTimeout(() => {
      setFound((prev) => new Map(list.map((a) => [a, prev.get(a) ?? { state: "looking" }])));
      for (const a of list) {
        void recipientKey(a).then((r) => {
          if (live) setFound((prev) => new Map(prev).set(a, r));
        });
      }
    }, 350);
    return () => {
      live = false;
      window.clearTimeout(timer);
    };
  }, [key]);
  return found;
}

const SOURCE: Record<MailKeySource, MessageKey> = {
  box: "mail.pgp.source.box",
  contacts: "mail.pgp.source.contacts",
  autocrypt: "mail.pgp.source.autocrypt",
  wkd: "mail.pgp.source.wkd",
  keyserver: "mail.pgp.source.keyserver",
};

export function sourceName(source: MailKeySource, t: ReturnType<typeof useT>): string {
  return t(SOURCE[source]);
}

/* ── How a message came ───────────────────────────────────────────────── */

export type Reading =
  | { state: "opening" }
  | { state: "locked" }
  | { state: "noKey" }
  | { state: "notForMe" }
  | { state: "broken" }
  | { state: "open"; kind: "encrypted" | "signed"; inline: boolean; signature: Check; from: string };

/** The line under a message's headers: encrypted or not, signed by whom,
 *  and what to do when it cannot be read. */
export function SecurityNote({
  reading,
  onUnlock,
  onSetUp,
}: {
  reading: Reading;
  onUnlock: () => void;
  onSetUp: () => void;
}) {
  const t = useT();
  if (reading.state === "opening") {
    return (
      <p className="flex items-center gap-2 text-[13px] text-muted" role="status">
        <Spinner className="size-4" />
        {t("mail.pgp.read.opening")}
      </p>
    );
  }
  if (reading.state === "locked" || reading.state === "noKey") {
    return (
      <Alert role="status" data-testid="mail-security">
        <HugeiconsIcon icon={SquareLock02Icon} strokeWidth={1.5} aria-hidden="true" />
        <AlertTitle>{t("mail.pgp.read.encryptedTitle")}</AlertTitle>
        <AlertDescription>
          <p>{reading.state === "locked" ? t("mail.pgp.read.locked") : t("mail.pgp.read.noKey")}</p>
          <Button size="sm" variant="secondary" className="mt-2" onClick={reading.state === "locked" ? onUnlock : onSetUp}>
            {reading.state === "locked" ? t("mail.pgp.unlock.button") : t("mail.pgp.setUp")}
          </Button>
        </AlertDescription>
      </Alert>
    );
  }
  if (reading.state === "notForMe" || reading.state === "broken") {
    return (
      <Alert variant="crit" role="alert" data-testid="mail-security">
        <HugeiconsIcon icon={AlertDiamondIcon} strokeWidth={1.5} aria-hidden="true" />
        <AlertDescription>{reading.state === "notForMe" ? t("mail.pgp.read.notForMe") : t("mail.pgp.read.broken")}</AlertDescription>
      </Alert>
    );
  }
  const { kind, signature, from } = reading;
  const encrypted = kind === "encrypted";
  const signer = signature.state === "good" ? (signature.emails.includes(from.toLowerCase()) ? from : signature.emails[0] ?? "") : "";
  const elsewhere = signature.state === "good" && !signature.emails.includes(from.toLowerCase());
  const tone = signature.state === "bad" || elsewhere ? "crit" : signature.state === "good" ? "ok" : encrypted ? "ok" : "warn";
  let line: string;
  if (signature.state === "good" && !elsewhere) {
    line = encrypted ? t("mail.pgp.read.encryptedSigned", { who: signer }) : t("mail.pgp.read.signed", { who: signer });
  } else if (elsewhere) {
    line = t("mail.pgp.read.otherSigner", { who: signer, from });
  } else if (signature.state === "bad") {
    line = t("mail.pgp.read.bad");
  } else if (signature.state === "unknown") {
    line = encrypted
      ? t("mail.pgp.read.encryptedUnknown", { id: signature.keyId.slice(-16) })
      : t("mail.pgp.read.unknown", { id: signature.keyId.slice(-16) });
  } else {
    line = t("mail.pgp.read.encryptedOnly");
  }
  return (
    <div
      data-testid="mail-security"
      data-state={tone}
      className={cn(
        "flex items-start gap-2 rounded-control border px-3 py-2 text-[13px] animate-fade-in",
        tone === "ok" && "border-ok/25 bg-ok/10",
        tone === "warn" && "border-warn/25 bg-warn/10",
        tone === "crit" && "border-crit/25 bg-crit/10",
      )}
    >
      <HugeiconsIcon
        icon={tone === "crit" ? AlertDiamondIcon : encrypted ? SquareLock02Icon : CheckmarkBadge01Icon}
        size={16}
        strokeWidth={1.5}
        aria-hidden="true"
        className={cn("mt-px shrink-0", tone === "ok" && "text-ok", tone === "warn" && "text-warn", tone === "crit" && "text-crit")}
      />
      <span className="min-w-0">
        <span className="block">{line}</span>
        {signature.state === "good" && (
          <span className="block font-mono text-[11.5px] text-muted">{grouped(signature.fingerprint)}</span>
        )}
        {reading.inline && <span className="block text-[12px] text-muted">{t("mail.pgp.read.inline")}</span>}
      </span>
    </div>
  );
}
