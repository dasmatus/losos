import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Copy01Icon, LaptopIcon, SmartPhone01Icon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/progress";
import { toast } from "@/components/ui/toast";
import { useT } from "@/lib/i18n-react";
import { cloud, isAbort, type Device } from "./api";
import { ConfirmDialog, explain, failed } from "./dialogs";
import { formatFull, formatWhen } from "./format";
import { bump, useRevision } from "./store";

function phoneLike(name: string): boolean {
  return /android|iphone|ipad|ios|mobile/i.test(name);
}

function Address({ label, value }: { label: string; value: string }) {
  const t = useT();
  return (
    <div className="flex flex-col gap-1">
      <span className="text-[12.5px] text-muted">{label}</span>
      <div className="flex items-center gap-2">
        <code className="min-w-0 flex-1 truncate rounded-control border border-line bg-sunk px-3 py-1.5 font-mono text-[13px]">{value}</code>
        <Button
          size="icon-sm"
          variant="ghost"
          aria-label={t("cloud.devices.copy", { what: label })}
          title={t("cloud.devices.copy", { what: label })}
          onClick={() => {
            void navigator.clipboard?.writeText(value).then(() => toast.done(t("cloud.devices.copied")));
          }}
        >
          <HugeiconsIcon icon={Copy01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </Button>
      </div>
    </div>
  );
}

/** The computers and phones that sync, and how to add one. */
export function DevicesView() {
  const t = useT();
  const revision = useRevision();
  const [devices, setDevices] = React.useState<Device[] | null>(null);
  const [error, setError] = React.useState<string | null>(null);
  const [revoking, setRevoking] = React.useState<Device | null>(null);
  const origin = window.location.origin;

  React.useEffect(() => {
    const ctl = new AbortController();
    cloud.devices(ctl.signal).then(
      (r) => setDevices([...r.devices].sort((a, b) => b.lastUsed - a.lastUsed || b.created - a.created)),
      (err: unknown) => {
        if (!isAbort(err)) setError(explain(err));
      },
    );
    return () => ctl.abort();
  }, [revision]);

  return (
    <div className="flex flex-col gap-6">
      <h1 className="flex min-h-9 items-center text-[19px] leading-tight font-semibold tracking-tight">{t("cloud.nav.devices")}</h1>

      <section className="flex flex-col gap-4 rounded-card border border-line bg-surface p-5" aria-labelledby="cloud-connect-title">
        <div className="flex flex-col gap-1">
          <h2 id="cloud-connect-title" className="text-sm font-semibold">
            {t("cloud.devices.addTitle")}
          </h2>
          <p className="text-[13px] text-muted">{t("cloud.devices.addBody")}</p>
        </div>
        <Address label={t("cloud.devices.server")} value={origin} />
        <Address label={t("cloud.devices.webdav")} value={`${origin}/remote.php/webdav/`} />
      </section>

      <section className="flex flex-col gap-3" aria-labelledby="cloud-devices-title">
        <h2 id="cloud-devices-title" className="text-sm font-semibold">
          {t("cloud.devices.listTitle")}
        </h2>
        {error !== null ? (
          <Alert variant="crit">
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        ) : devices === null ? (
          <Spinner />
        ) : devices.length === 0 ? (
          <p className="rounded-card border border-dashed border-line p-6 text-center text-[13px] text-muted">{t("cloud.devices.none")}</p>
        ) : (
          <ul className="flex flex-col rounded-card border border-line bg-surface" data-devices>
            {devices.map((d) => (
              <li key={d.id} className="flex items-center gap-3 border-b border-hair px-4 py-3 last:border-b-0">
                <HugeiconsIcon
                  icon={phoneLike(d.name) ? SmartPhone01Icon : LaptopIcon}
                  size={22}
                  strokeWidth={1.5}
                  color="currentColor"
                  className="text-muted"
                  aria-hidden="true"
                />
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-medium">{d.name}</p>
                  <p className="text-[12.5px] text-muted" title={formatFull(d.created)}>
                    {d.lastUsed > 0
                      ? t("cloud.devices.lastUsed", { when: formatWhen(d.lastUsed) })
                      : t("cloud.devices.added", { when: formatWhen(d.created) })}
                  </p>
                </div>
                <Button size="sm" variant="secondary" onClick={() => setRevoking(d)}>
                  {t("cloud.devices.revoke")}
                </Button>
              </li>
            ))}
          </ul>
        )}
      </section>

      <ConfirmDialog
        open={revoking !== null}
        onOpenChange={(o) => !o && setRevoking(null)}
        title={t("cloud.devices.revokeTitle", { name: revoking?.name ?? "" })}
        body={t("cloud.devices.revokeBody")}
        confirm={t("cloud.devices.revoke")}
        onConfirm={() => {
          const d = revoking;
          if (d === null) return;
          cloud.revokeDevice(d.id).then(
            () => {
              toast.done(t("cloud.devices.revoked", { name: d.name }));
              bump();
            },
            (err: unknown) => failed(t("cloud.devices.revokeFailed"), err),
          );
        }}
      />
    </div>
  );
}
