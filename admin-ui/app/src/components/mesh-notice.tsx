import * as React from "react";
import { Link, useLocation } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import { Cancel01Icon, Share08Icon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { getSettings, hasToken, subscribeAuth, type SettingsResponse } from "@/lib/api";
import { paneHref } from "@/lib/routes";
import { paneOpen, type SettingsPaneId } from "@/screens/settings/panes";
import { useT } from "@/lib/i18n-react";

/* The banner over every page while this box is not on a mesh.
 *
 * Some of what LosOS does runs only on a box that shares its storage AND has
 * joined a mesh, or that shares with its local gateways (the same rule the
 * About pane and the Overview's storage strip read, and
 * `lososInternal.sideLive` on the Nix side). Those features
 * do not appear at all on any other box, so the banner names none of them:
 * it says that some may be missing and where the switch is.
 *
 * The reading is the box's saved settings (GET /api/settings), asked again
 * on every page change, when the tab comes back, and every 30 s, so the
 * banner goes by itself once a join is applied. Dismissing it is this
 * browser's choice and lasts until the box joins; a box that later leaves
 * shows it again. Nothing is shown until the box has answered. */

const PERIOD_MS = 30_000;
const DISMISSED_KEY = "losos-mesh-notice";

export type MeshReading = "unknown" | "joined" | "local" | "notJoined" | "notSharing";

/** Where the box stands, from its saved settings. A box that shares with
 *  its local gateways runs the same features as `kindashared`
 *  (`lososInternal.sideUser`), so it is not missing any and sees no banner. */
export function meshReading(
  settings: Pick<SettingsResponse, "sharingMyStorage" | "sharingLocally" | "clusterEnable"> | null,
): MeshReading {
  if (settings === null) return "unknown";
  if (settings.sharingMyStorage && settings.clusterEnable) return "joined";
  if (settings.sharingLocally) return "local";
  if (!settings.clusterEnable) return "notJoined";
  if (!settings.sharingMyStorage) return "notSharing";
  return "joined";
}

function readDismissed(): boolean {
  try {
    return window.localStorage.getItem(DISMISSED_KEY) === "dismissed";
  } catch {
    return false;
  }
}

function writeDismissed(dismissed: boolean): void {
  try {
    if (dismissed) window.localStorage.setItem(DISMISSED_KEY, "dismissed");
    else window.localStorage.removeItem(DISMISSED_KEY);
  } catch {
    // Storage refused (a private window): the dismissal lasts this page.
  }
}

function useMeshReading(): MeshReading {
  const signedIn = React.useSyncExternalStore(subscribeAuth, hasToken, () => false);
  const { pathname } = useLocation();
  const [reading, setReading] = React.useState<MeshReading>("unknown");

  React.useEffect(() => {
    if (!signedIn) {
      setReading("unknown");
      return;
    }
    const controller = new AbortController();
    const ask = (): void => {
      getSettings({ signal: controller.signal })
        .then((settings) => setReading(meshReading(settings)))
        .catch(() => {
          // No answer keeps the last reading; a 401 has already sent the
          // shell back to sign-in.
        });
    };
    ask();
    const timer = window.setInterval(ask, PERIOD_MS);
    const onVisible = (): void => {
      if (document.visibilityState === "visible") ask();
    };
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      controller.abort();
      window.clearInterval(timer);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, [signedIn, pathname]);

  return reading;
}

export function MeshNotice() {
  const t = useT();
  const { pathname } = useLocation();
  const reading = useMeshReading();
  const [dismissed, setDismissed] = React.useState(readDismissed);

  React.useEffect(() => {
    if ((reading === "joined" || reading === "local") && dismissed) {
      writeDismissed(false);
      setDismissed(false);
    }
  }, [reading, dismissed]);

  if (reading === "unknown" || reading === "joined" || reading === "local" || dismissed) return null;

  const pane: SettingsPaneId = reading === "notJoined" ? "mesh" : "market";
  const href = paneHref(pane);
  // No link to the page the owner is already on, nor to one this person
  // cannot open; they get the sentence for someone who cannot change it.
  const canOpen = paneOpen(pane);
  const showLink = canOpen && pathname !== href;

  return (
    <Alert
      variant="warn"
      role="status"
      data-notice="mesh"
      data-mesh={reading}
      className="mb-4 pr-11"
    >
      <HugeiconsIcon icon={Share08Icon} size={19} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      <AlertTitle>{t("ui.mesh.title")}</AlertTitle>
      <AlertDescription>
        <p>
          {t(reading === "notJoined" ? "ui.mesh.notJoined" : "ui.mesh.notSharing")}
          {!canOpen && ` ${t("ui.mesh.askAdmin")}`}
        </p>
        {showLink && (
          <Link
            to={href}
            data-mesh-link={pane}
            className="inline-flex w-fit items-center gap-1 text-[12.5px] font-medium text-accent underline-offset-2 hover:underline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
          >
            {t(reading === "notJoined" ? "ui.mesh.join" : "ui.mesh.share")}
          </Link>
        )}
      </AlertDescription>
      <Button
        variant="ghost"
        size="icon-sm"
        aria-label={t("ui.dismiss")}
        data-mesh-dismiss=""
        className="absolute top-2 right-2"
        onClick={() => {
          writeDismissed(true);
          setDismissed(true);
        }}
      >
        <HugeiconsIcon icon={Cancel01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      </Button>
    </Alert>
  );
}
