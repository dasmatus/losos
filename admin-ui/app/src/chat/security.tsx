import * as React from "react";
import type { MatrixClient } from "matrix-js-sdk";
import {
  CryptoEvent,
  type ShowSasCallbacks,
  type VerificationRequest,
  VerificationPhase,
  VerificationRequestEvent,
  VerifierEvent,
} from "matrix-js-sdk/lib/crypto-api";
import { Button } from "@/components/ui/button";
import { Dialog, DialogBody, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import { toast } from "@/components/ui/toast";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { recoveryKeyBytes, type SecretKeys } from "./session";

/* End-to-end encryption on this page: cross-signing, the recovery key that
 * keeps the keys in secret storage on the box (encrypted with it, so the box
 * cannot read them), the key backup that brings old messages to a new
 * device, and verifying a device by comparing emoji. */

export type Trust = "loading" | "new" | "unverified" | "ready";

/** Where this device stands. Re-read when the crypto says something changed. */
export function useTrust(client: MatrixClient): [Trust, () => void] {
  const [trust, setTrust] = React.useState<Trust>("loading");
  const [tick, setTick] = React.useState(0);
  const recheck = React.useCallback(() => setTick((n) => n + 1), []);
  React.useEffect(() => {
    const crypto = client.getCrypto();
    if (crypto === undefined) return;
    let live = true;
    (async () => {
      const me = client.getSafeUserId();
      const has = await crypto.userHasCrossSigningKeys(me, true);
      if (!has) return "new" as const;
      const status = await crypto.getDeviceVerificationStatus(me, client.getDeviceId() ?? "");
      return status?.crossSigningVerified === true ? ("ready" as const) : ("unverified" as const);
    })()
      .then((found) => live && setTrust(found))
      .catch(() => live && setTrust("unverified"));
    const changed = (): void => recheck();
    client.on(CryptoEvent.UserTrustStatusChanged, changed);
    client.on(CryptoEvent.KeysChanged, changed);
    return () => {
      live = false;
      client.off(CryptoEvent.UserTrustStatusChanged, changed);
      client.off(CryptoEvent.KeysChanged, changed);
    };
  }, [client, tick, recheck]);
  return [trust, recheck];
}

function saveText(name: string, text: string): void {
  const url = URL.createObjectURL(new Blob([text], { type: "text/plain" }));
  const link = document.createElement("a");
  link.href = url;
  link.download = name;
  document.body.append(link);
  link.click();
  link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

/** First device: make cross-signing keys, a recovery key and a backup. */
function SetUp({ client, onDone }: { client: MatrixClient; onDone: () => void }) {
  const t = useT();
  const [key, setKey] = React.useState<string | null>(null);
  const [busy, setBusy] = React.useState(false);
  const [saved, setSaved] = React.useState(false);
  const begin = async (): Promise<void> => {
    const crypto = client.getCrypto();
    if (crypto === undefined) return;
    setBusy(true);
    try {
      await crypto.bootstrapCrossSigning({
        // A first upload needs no password on this homeserver (MSC3967),
        // and chat has none to give.
        authUploadDeviceSigningKeys: async (makeRequest) => {
          await makeRequest(null);
        },
      });
      await crypto.bootstrapSecretStorage({
        createSecretStorageKey: async () => {
          const made = await crypto.createRecoveryKeyFromPassphrase();
          setKey(made.encodedPrivateKey ?? null);
          return made;
        },
        setupNewKeyBackup: true,
      });
    } catch (error) {
      toast.error(t("chat.security.setupFailed"), error instanceof Error ? error.message : String(error), { help: "chat" });
    } finally {
      setBusy(false);
    }
  };
  if (key === null) {
    return (
      <>
        <DialogBody className="flex flex-col gap-2 text-[13.5px]">
          <p>{t("chat.security.setupLead")}</p>
          <p className="text-muted">{t("chat.security.setupHow")}</p>
        </DialogBody>
        <DialogFooter>
          <Button onClick={() => void begin()} disabled={busy}>
            {busy && <Spinner size={14} />}
            {t("chat.security.setupGo")}
          </Button>
        </DialogFooter>
      </>
    );
  }
  return (
    <>
      <DialogBody className="flex flex-col gap-3 text-[13.5px]">
        <p>{t("chat.security.keyLead")}</p>
        <p className="rounded-control border border-line bg-sunk p-3 font-mono text-[14px] leading-relaxed tracking-wide break-words select-all" data-testid="recovery-key">
          {key}
        </p>
        <div className="flex gap-2">
          <Button
            size="sm"
            variant="secondary"
            onClick={() => {
              void navigator.clipboard.writeText(key).then(() => toast.done(t("chat.security.copied")));
              setSaved(true);
            }}
          >
            {t("chat.security.copy")}
          </Button>
          <Button
            size="sm"
            variant="secondary"
            onClick={() => {
              saveText("losos-chat-recovery-key.txt", `${key}\n`);
              setSaved(true);
            }}
          >
            {t("chat.security.download")}
          </Button>
        </div>
      </DialogBody>
      <DialogFooter>
        <Button onClick={onDone} disabled={!saved}>
          {t("chat.security.keySaved")}
        </Button>
      </DialogFooter>
    </>
  );
}

/** Another device of the person's: the recovery key, or emoji. */
function Unlock({ client, keys, onDone, onVerify }: { client: MatrixClient; keys: SecretKeys; onDone: () => void; onVerify: (request: VerificationRequest) => void }) {
  const t = useT();
  const [text, setText] = React.useState("");
  const [busy, setBusy] = React.useState(false);
  const [wrong, setWrong] = React.useState(false);

  const useKey = async (): Promise<void> => {
    const crypto = client.getCrypto();
    const bytes = recoveryKeyBytes(text);
    if (crypto === undefined || bytes === null) {
      setWrong(true);
      return;
    }
    setBusy(true);
    setWrong(false);
    try {
      const found = await client.secretStorage.getKey();
      if (found === null) throw new Error("no secret storage key");
      const [keyId, info] = found;
      if (!(await client.secretStorage.checkKey(bytes, info as Parameters<typeof client.secretStorage.checkKey>[1]))) {
        setWrong(true);
        return;
      }
      keys.set(keyId, bytes);
      // Takes the cross-signing keys out of secret storage and signs this device.
      await crypto.bootstrapCrossSigning({});
      await crypto.loadSessionBackupPrivateKeyFromSecretStorage().catch(() => undefined);
      const backup = await crypto.checkKeyBackupAndEnable();
      if (backup !== null) await crypto.restoreKeyBackup().catch(() => undefined);
      toast.done(t("chat.security.unlocked"));
      onDone();
    } catch (error) {
      toast.error(t("chat.security.unlockFailed"), error instanceof Error ? error.message : String(error), { help: "chat" });
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <DialogBody className="flex flex-col gap-3 text-[13.5px]">
        <p>{t("chat.security.unlockLead")}</p>
        <form
          className="flex flex-col gap-2"
          onSubmit={(event) => {
            event.preventDefault();
            void useKey();
          }}
        >
          <Input
            value={text}
            onChange={(event) => setText(event.target.value)}
            placeholder="EsT… …"
            aria-label={t("chat.security.keyField")}
            aria-invalid={wrong || undefined}
            className="font-mono"
            autoComplete="off"
            spellCheck={false}
          />
          {wrong && <p className="text-[12.5px] text-crit">{t("chat.security.wrongKey")}</p>}
          <Button type="submit" disabled={busy || text.trim() === ""}>
            {busy && <Spinner size={14} />}
            {t("chat.security.useKey")}
          </Button>
        </form>
        <div className="flex items-center gap-3 text-[12px] text-faint">
          <span className="h-px flex-1 bg-hair" />
          {t("chat.security.or")}
          <span className="h-px flex-1 bg-hair" />
        </div>
        <Button
          variant="secondary"
          disabled={busy}
          onClick={() => {
            const crypto = client.getCrypto();
            if (crypto === undefined) return;
            crypto
              .requestOwnUserVerification()
              .then(onVerify)
              .catch((error: unknown) => toast.error(t("chat.security.verifyFailed"), String(error), { help: "chat" }));
          }}
        >
          {t("chat.security.withDevice")}
        </Button>
      </DialogBody>
    </>
  );
}

interface DeviceRow {
  id: string;
  name: string;
  verified: boolean;
}

/** This person's devices, and whether each is verified. */
function Devices({ client, onVerify }: { client: MatrixClient; onVerify: (request: VerificationRequest) => void }) {
  const t = useT();
  const [rows, setRows] = React.useState<DeviceRow[] | null>(null);
  React.useEffect(() => {
    const crypto = client.getCrypto();
    if (crypto === undefined) return;
    let live = true;
    const me = client.getSafeUserId();
    (async () => {
      const map = await crypto.getUserDeviceInfo([me], true);
      const out: DeviceRow[] = [];
      for (const device of map.get(me)?.values() ?? []) {
        const status = await crypto.getDeviceVerificationStatus(me, device.deviceId);
        out.push({ id: device.deviceId, name: device.displayName ?? device.deviceId, verified: status?.crossSigningVerified === true });
      }
      return out;
    })()
      .then((found) => live && setRows(found))
      .catch(() => live && setRows([]));
    return () => {
      live = false;
    };
  }, [client]);
  if (rows === null) return <Spinner size={16} />;
  return (
    <ul className="flex flex-col divide-y divide-hair rounded-control border border-hair">
      {rows.map((row) => (
        <li key={row.id} className="flex items-center gap-2 px-3 py-2">
          <span className="min-w-0 flex-1">
            <span className="block truncate text-[13px] font-medium">
              {row.name}
              {row.id === client.getDeviceId() ? ` · ${t("chat.security.thisDevice")}` : ""}
            </span>
            <span className="block font-mono text-[11.5px] text-muted">{row.id}</span>
          </span>
          <span className={cn("text-[12px] font-medium", row.verified ? "text-ok" : "text-warn")}>
            {row.verified ? t("chat.security.verified") : t("chat.security.notVerified")}
          </span>
          {!row.verified && row.id !== client.getDeviceId() && (
            <Button
              size="xs"
              variant="secondary"
              onClick={() => {
                void client
                  .getCrypto()
                  ?.requestDeviceVerification(client.getSafeUserId(), row.id)
                  .then(onVerify);
              }}
            >
              {t("chat.security.verify")}
            </Button>
          )}
        </li>
      ))}
    </ul>
  );
}

export interface SecurityDialogProps {
  client: MatrixClient;
  keys: SecretKeys;
  trust: Trust;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onChanged: () => void;
  onVerify: (request: VerificationRequest) => void;
}

export function SecurityDialog({ client, keys, trust, open, onOpenChange, onChanged, onVerify }: SecurityDialogProps) {
  const t = useT();
  // What the dialog opened for stays on screen until it closes: setting up
  // makes this device ready halfway through, and the recovery key must not
  // give way to the device list before the person has saved it.
  const [shown, setShown] = React.useState<Trust>(trust);
  const wasOpen = React.useRef(false);
  React.useEffect(() => {
    if (open && (!wasOpen.current || shown === "loading")) setShown(trust);
    wasOpen.current = open;
  }, [open, trust, shown]);
  const view = wasOpen.current && shown !== "loading" ? shown : trust;
  const done = (): void => {
    onChanged();
    onOpenChange(false);
  };
  return (
    <Dialog open={open} onOpenChange={onOpenChange} labelledBy="chat-security" dialogClassName="w-[min(30rem,calc(100vw-2rem))]">
      <DialogHeader>
        <DialogTitle id="chat-security">{t("chat.security.title")}</DialogTitle>
        <DialogDescription>
          {view === "ready" ? t("chat.security.readyLead") : view === "new" ? t("chat.security.newLead") : t("chat.security.unverifiedLead")}
        </DialogDescription>
      </DialogHeader>
      {open && view === "new" && <SetUp client={client} onDone={done} />}
      {open && view === "unverified" && (
        <Unlock
          client={client}
          keys={keys}
          onDone={done}
          onVerify={(request) => {
            onOpenChange(false);
            onVerify(request);
          }}
        />
      )}
      {open && view === "ready" && (
        <DialogBody>
          <Devices
            client={client}
            onVerify={(request) => {
              onOpenChange(false);
              onVerify(request);
            }}
          />
        </DialogBody>
      )}
      {open && view === "loading" && (
        <DialogBody className="flex justify-center">
          <Spinner size={18} />
        </DialogBody>
      )}
    </Dialog>
  );
}

/** A verification in progress, either side: accept, compare emoji, done. */
export function VerificationDialog({ request, onClose }: { request: VerificationRequest | null; onClose: () => void }) {
  const t = useT();
  const [, setTick] = React.useState(0);
  const [sas, setSas] = React.useState<ShowSasCallbacks | null>(null);
  const hooked = React.useRef<unknown>(null);

  React.useEffect(() => {
    if (request === null) return;
    setSas(null);
    hooked.current = null;
    const changed = (): void => {
      setTick((n) => n + 1);
      const verifier = request.verifier;
      if (verifier !== undefined && hooked.current !== verifier) {
        hooked.current = verifier;
        verifier.on(VerifierEvent.ShowSas, setSas);
        const shown = verifier.getShowSasCallbacks();
        if (shown !== null) setSas(shown);
        verifier.verify().catch(() => undefined);
      }
    };
    request.on(VerificationRequestEvent.Change, changed);
    changed();
    return () => {
      request.off(VerificationRequestEvent.Change, changed);
    };
  }, [request]);

  if (request === null) return null;
  const phase = request.phase;
  const close = (): void => {
    if (phase !== VerificationPhase.Done && phase !== VerificationPhase.Cancelled) void request.cancel().catch(() => undefined);
    onClose();
  };

  let body: React.ReactNode;
  let footer: React.ReactNode = null;
  if (phase === VerificationPhase.Requested && !request.initiatedByMe) {
    body = <p>{t("chat.verify.incoming", { device: request.otherDeviceId ?? "" })}</p>;
    footer = (
      <>
        <Button variant="ghost" onClick={close}>
          {t("chat.verify.decline")}
        </Button>
        <Button onClick={() => void request.accept()}>{t("chat.verify.accept")}</Button>
      </>
    );
  } else if (phase === VerificationPhase.Requested || phase === VerificationPhase.Unsent) {
    body = (
      <p className="flex items-center gap-2">
        <Spinner size={14} />
        {t("chat.verify.waiting")}
      </p>
    );
  } else if (phase === VerificationPhase.Ready) {
    body = <p>{t("chat.verify.ready")}</p>;
    footer = (
      <Button
        onClick={() => {
          request.startVerification("m.sas.v1").catch(() => undefined);
        }}
      >
        {t("chat.verify.emoji")}
      </Button>
    );
  } else if (phase === VerificationPhase.Started) {
    const emoji = sas?.sas.emoji;
    body =
      emoji === undefined ? (
        <p className="flex items-center gap-2">
          <Spinner size={14} />
          {t("chat.verify.waiting")}
        </p>
      ) : (
        <div className="flex flex-col gap-3">
          <p>{t("chat.verify.compare")}</p>
          <ul className="grid grid-cols-4 gap-2 sm:grid-cols-7" data-testid="sas-emoji">
            {emoji.map(([symbol, name]) => (
              <li key={name} className="flex flex-col items-center gap-1 rounded-control bg-sunk p-2">
                <span className="text-2xl">{symbol}</span>
                <span className="text-[11px] text-muted">{name}</span>
              </li>
            ))}
          </ul>
        </div>
      );
    if (sas !== null)
      footer = (
        <>
          <Button variant="ghost" onClick={() => sas.mismatch()}>
            {t("chat.verify.mismatch")}
          </Button>
          <Button onClick={() => void sas.confirm()}>{t("chat.verify.match")}</Button>
        </>
      );
  } else if (phase === VerificationPhase.Done) {
    body = <p className="text-ok">{t("chat.verify.done")}</p>;
    footer = <Button onClick={onClose}>{t("chat.dialog.close")}</Button>;
  } else {
    body = <p className="text-muted">{t("chat.verify.cancelled")}</p>;
    footer = <Button onClick={onClose}>{t("chat.dialog.close")}</Button>;
  }

  return (
    <Dialog open onOpenChange={(open) => !open && close()} labelledBy="chat-verify" dialogClassName="w-[min(32rem,calc(100vw-2rem))]">
      <DialogHeader>
        <DialogTitle id="chat-verify">{t("chat.verify.title")}</DialogTitle>
      </DialogHeader>
      <DialogBody className="text-[13.5px]">{body}</DialogBody>
      {footer !== null && <DialogFooter>{footer}</DialogFooter>}
    </Dialog>
  );
}
