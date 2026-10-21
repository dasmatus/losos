import * as React from "react";
import { useParams } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import { Alert01Icon, CheckmarkCircle02Icon, LaptopIcon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/progress";
import { useT } from "@/lib/i18n-react";
import { CloudError, cloud, isAbort, type Flow } from "./api";
import { explain } from "./dialogs";
import { useMe } from "./store";

/** The app's name from its user agent: "Nextcloud-android/3.30.0" and kin. */
function phoneName(agent: string): string {
  const m = /(Nextcloud-[A-Za-z]+\/[0-9.]+|Nextcloud-iOS\/[0-9.]+)/.exec(agent);
  return m?.[1] ?? "Nextcloud";
}

type Step = { kind: "loading" } | { kind: "ask"; flow: Flow } | { kind: "granted" } | { kind: "declined" } | { kind: "gone" } | { kind: "error"; message: string };

/* Login Flow v2's page: the Nextcloud desktop or phone app opened the
 * browser here (by way of /index.php/login/v2/flow/<token>), the person is
 * signed in with LosOS Login, and one button hands the app an app password
 * of its own. The app is polling meanwhile and picks it up by itself. */
export function ConnectPage() {
  const t = useT();
  const { token = "" } = useParams();
  const me = useMe();
  const [step, setStep] = React.useState<Step>({ kind: "loading" });
  const [busy, setBusy] = React.useState(false);

  // Login Flow v1: the phone app's web view, which names itself only in
  // its user agent and takes the password from an nc:// address.
  const v1 = token === "v1";

  React.useEffect(() => {
    if (v1) {
      setStep({ kind: "ask", flow: { client: phoneName(navigator.userAgent), created: 0, granted: false } });
      return;
    }
    const ctl = new AbortController();
    cloud.flow(token, ctl.signal).then(
      (flow) => setStep(flow.granted ? { kind: "granted" } : { kind: "ask", flow }),
      (err: unknown) => {
        if (isAbort(err)) return;
        if (err instanceof CloudError && err.status === 404) setStep({ kind: "gone" });
        else setStep({ kind: "error", message: explain(err) });
      },
    );
    return () => ctl.abort();
  }, [token, v1]);

  const grant = async () => {
    setBusy(true);
    try {
      if (v1) {
        const got = await cloud.grantV1(step.kind === "ask" ? step.flow.client : "");
        const enc = encodeURIComponent;
        setStep({ kind: "granted" });
        window.location.href = `nc://login/server:${got.server}&user:${enc(got.login)}&password:${enc(got.appPassword)}`;
        return;
      }
      await cloud.grant(token);
      setStep({ kind: "granted" });
    } catch (err) {
      if (err instanceof CloudError && err.status === 404) setStep({ kind: "gone" });
      else setStep({ kind: "error", message: explain(err) });
    }
    setBusy(false);
  };

  const card = (icon: typeof LaptopIcon, tone: string, title: string, body: React.ReactNode, actions?: React.ReactNode) => (
    <div className="flex w-full max-w-md flex-col items-center gap-4 rounded-card border border-line bg-surface p-8 text-center shadow-pop" data-connect>
      <span className={`grid size-12 place-items-center rounded-full bg-sunk ${tone}`}>
        <HugeiconsIcon icon={icon} size={24} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      </span>
      <h1 className="text-[19px] leading-tight font-semibold tracking-tight">{title}</h1>
      <div className="text-[13.5px] leading-relaxed text-muted">{body}</div>
      {actions !== undefined && <div className="flex w-full flex-col gap-2 pt-2">{actions}</div>}
    </div>
  );

  return (
    <main className="grid min-h-[70dvh] place-items-center px-4 py-10">
      {step.kind === "loading" ? (
        <Spinner />
      ) : step.kind === "ask" ? (
        card(
          LaptopIcon,
          "text-accent",
          t("cloud.connect.title"),
          <>
            <p>{t("cloud.connect.body", { client: step.flow.client, name: me?.name || me?.login || "" })}</p>
            <p className="mt-2">{t("cloud.connect.note")}</p>
          </>,
          <>
            <Button size="lg" onClick={() => void grant()} disabled={busy}>
              {busy && <Spinner size={16} />}
              {t("cloud.connect.allow")}
            </Button>
            <Button variant="ghost" onClick={() => setStep({ kind: "declined" })} disabled={busy}>
              {t("cloud.connect.deny")}
            </Button>
          </>,
        )
      ) : step.kind === "granted" ? (
        card(CheckmarkCircle02Icon, "text-ok", t("cloud.connect.doneTitle"), <p>{t("cloud.connect.doneBody")}</p>)
      ) : step.kind === "declined" ? (
        card(Alert01Icon, "text-muted", t("cloud.connect.declinedTitle"), <p>{t("cloud.connect.declinedBody")}</p>)
      ) : step.kind === "gone" ? (
        card(Alert01Icon, "text-warn", t("cloud.connect.goneTitle"), <p>{t("cloud.connect.goneBody")}</p>)
      ) : (
        card(Alert01Icon, "text-crit", t("cloud.connect.errorTitle"), <p>{step.message}</p>)
      )}
    </main>
  );
}
