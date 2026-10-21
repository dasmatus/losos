import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Cancel01Icon } from "@hugeicons/core-free-icons";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogBody, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Field, FieldDescription, FieldError, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Spinner } from "@/components/ui/spinner";
import { toast } from "@/components/ui/toast";
import type { MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { CloudError, isAbort } from "../api";
import { mesh, type Outgoing, type Perm } from "../collab/api";
import { initials, accentFor, STRONG, WASH } from "../collab/people";

/* Share a file or folder with someone on another box: their address
 * (name@box), what they may do, and the people it is shared with so far,
 * each with where the offer stands. The cloud's own share dialog carries
 * this as its "Someone on another box" part. */

const ADDRESS = /^[A-Za-z0-9._-]{1,64}@[a-z0-9.-]{1,253}$/i;

const STATE: Record<Outgoing["state"], { key: MessageKey; variant: "neutral" | "ok" | "warn" | "crit" }> = {
  unsent: { key: "mesh.state.unsent", variant: "warn" },
  pending: { key: "mesh.state.pending", variant: "neutral" },
  accepted: { key: "mesh.state.accepted", variant: "ok" },
  declined: { key: "mesh.state.declined", variant: "crit" },
};

export function meshError(err: unknown): MessageKey | null {
  if (!(err instanceof CloudError)) return null;
  switch (err.code) {
    case "badAddress":
      return "mesh.error.badAddress";
    case "forbidden":
      return "mesh.error.forbidden";
    case "unreachable":
    case "timeout":
    case "closed":
      return "mesh.error.unreachable";
    case "keyChanged":
      return "mesh.error.keyChanged";
    default:
      return null;
  }
}

export function MeshShareDialog({
  open,
  onOpenChange,
  fileId,
  name,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  fileId: number;
  name: string;
}) {
  const t = useT();
  const id = React.useId();
  const [to, setTo] = React.useState("");
  const [perm, setPerm] = React.useState<Perm>("edit");
  const [shares, setShares] = React.useState<Outgoing[] | null>(null);
  const [box, setBox] = React.useState<{ name: string; me: string } | null>(null);
  const [off, setOff] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [busy, setBusy] = React.useState(false);

  const reload = React.useCallback(
    (signal?: AbortSignal) =>
      mesh.shares(fileId, signal).then(setShares, (err: unknown) => {
        if (isAbort(err)) return;
        if (err instanceof CloudError && err.status === 404) setOff(true);
        setShares([]);
      }),
    [fileId],
  );

  React.useEffect(() => {
    if (!open) return;
    setTo("");
    setError(null);
    setBusy(false);
    const controller = new AbortController();
    void reload(controller.signal);
    mesh.box(controller.signal).then(setBox, (err: unknown) => {
      if (err instanceof CloudError && err.status === 404) setOff(true);
    });
    // Offers go out in the background; follow them while the dialog is up.
    const timer = window.setInterval(() => void reload(), 3000);
    return () => {
      controller.abort();
      window.clearInterval(timer);
    };
  }, [open, reload]);

  const address = to.trim().toLowerCase();
  const shape = address === "" || ADDRESS.test(address);
  const self = box !== null && address.endsWith(`@${box.name}`);
  const invalid = !shape || self;

  const submit = () => {
    if (address === "" || invalid || busy) return;
    setBusy(true);
    mesh.share(fileId, address, perm).then(
      (s) => {
        setBusy(false);
        setTo("");
        toast.done(t("mesh.share.done", { who: s.recipient }));
        void reload();
      },
      (err: unknown) => {
        setBusy(false);
        const key = meshError(err);
        setError(key !== null ? t(key) : err instanceof Error ? err.message : String(err));
      },
    );
  };

  const change = (s: Outgoing, next: Perm) => {
    setShares((all) => all?.map((x) => (x.id === s.id ? { ...x, perm: next } : x)) ?? null);
    mesh.setPerm(s.id, next).then(
      () => void reload(),
      () => void reload(),
    );
  };

  const remove = (s: Outgoing) => {
    setShares((all) => all?.filter((x) => x.id !== s.id) ?? null);
    mesh.unshare(s.id).then(
      () => void reload(),
      () => void reload(),
    );
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange} labelledBy={`${id}-title`} describedBy={`${id}-lead`} dialogClassName="max-w-lg">
      <DialogHeader>
        <DialogTitle id={`${id}-title`}>{t("mesh.share.title", { name })}</DialogTitle>
        <DialogDescription id={`${id}-lead`}>{t(off ? "mesh.share.off" : "mesh.share.lead")}</DialogDescription>
      </DialogHeader>
      {!off && (
        <DialogBody className="flex flex-col gap-5">
          <form
            className="flex flex-col gap-2"
            onSubmit={(event) => {
              event.preventDefault();
              submit();
            }}
            data-mesh-share-form
          >
            <Field invalid={invalid || error !== null}>
              <FieldLabel htmlFor={`${id}-to`}>{t("mesh.share.to.label")}</FieldLabel>
              <div className="flex flex-wrap items-center gap-2">
                <Input
                  id={`${id}-to`}
                  value={to}
                  placeholder={t("mesh.share.to.placeholder")}
                  autoComplete="off"
                  autoCapitalize="off"
                  spellCheck={false}
                  inputMode="email"
                  aria-invalid={invalid || error !== null}
                  aria-describedby={`${id}-hint`}
                  className="min-w-0 flex-1 basis-56"
                  onChange={(event) => {
                    setTo(event.target.value);
                    setError(null);
                  }}
                />
                <NativeSelect
                  aria-label={t("mesh.perm.label")}
                  value={perm}
                  onChange={(event) => setPerm(event.target.value as Perm)}
                >
                  <NativeSelectOption value="edit">{t("mesh.perm.edit")}</NativeSelectOption>
                  <NativeSelectOption value="read">{t("mesh.perm.read")}</NativeSelectOption>
                </NativeSelect>
                <Button type="submit" disabled={address === "" || invalid || busy}>
                  {busy && <Spinner size={14} />}
                  {t("mesh.share.submit")}
                </Button>
              </div>
              {invalid || error !== null ? (
                <FieldError>{error ?? t(self ? "mesh.share.to.self" : "mesh.share.to.bad")}</FieldError>
              ) : (
                <FieldDescription id={`${id}-hint`}>{t("mesh.share.to.hint")}</FieldDescription>
              )}
            </Field>
          </form>

          <section aria-labelledby={`${id}-with`} className="flex flex-col gap-2">
            <h3 id={`${id}-with`} className="text-[13px] font-medium text-muted">
              {t("mesh.share.with")}
            </h3>
            {shares === null ? (
              <Spinner size={16} />
            ) : shares.length === 0 ? (
              <p className="text-[13px] text-faint">{t("mesh.share.none")}</p>
            ) : (
              <ul className="flex flex-col divide-y divide-hair rounded-control border border-line" data-mesh-shares>
                {shares.map((s) => {
                  const accent = accentFor(s.recipient);
                  const state = STATE[s.state];
                  return (
                    <li key={s.id} className="flex flex-wrap items-center gap-x-3 gap-y-2 px-3 py-2.5" data-share={s.recipient}>
                      <span
                        aria-hidden="true"
                        className={cn("grid size-8 shrink-0 place-items-center rounded-full text-[12px] font-semibold", WASH[accent], STRONG[accent])}
                      >
                        {initials(s.recipient.split("@")[0] ?? "?")}
                      </span>
                      <div className="flex min-w-0 flex-1 flex-col">
                        <span className="truncate text-[13.5px] font-medium">{s.recipient}</span>
                        <span className="flex text-[12px]">
                          <Badge variant={state.variant} className="max-w-full px-2 py-0 text-[11.5px] whitespace-normal" data-state={s.state}>
                            {t(state.key)}
                          </Badge>
                        </span>
                      </div>
                      <NativeSelect
                        aria-label={t("mesh.perm.labelFor", { who: s.recipient })}
                        value={s.perm}
                        onChange={(event) => change(s, event.target.value as Perm)}
                      >
                        <NativeSelectOption value="edit">{t("mesh.perm.edit")}</NativeSelectOption>
                        <NativeSelectOption value="read">{t("mesh.perm.read")}</NativeSelectOption>
                      </NativeSelect>
                      <Button
                        size="icon-sm"
                        variant="ghost"
                        className="hover:text-crit"
                        aria-label={t("mesh.share.remove", { who: s.recipient })}
                        title={t("mesh.share.remove", { who: s.recipient })}
                        onClick={() => remove(s)}
                      >
                        <HugeiconsIcon icon={Cancel01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                      </Button>
                    </li>
                  );
                })}
              </ul>
            )}
          </section>
        </DialogBody>
      )}
      <DialogFooter className="justify-between">
        <span className="text-[12px] text-muted">{box !== null && t("mesh.share.mine", { address: box.me })}</span>
        <Button variant="ghost" onClick={() => onOpenChange(false)}>
          {t("mesh.share.close")}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}
