import * as React from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { getUpdates, isAbort, isUnauthorized, postUpdatesCheck } from "@/lib/api";
import { useT } from "@/lib/i18n-react";
import { openTour } from "@/tour/store";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText, RowValue } from "./rows";
import { describeWindow } from "./window";
import type { SettingsForm } from "./use-settings-form";

/* What this box IS, as opposed to what it is about to become.
 *
 * Every value here is read from `saved`, never from `draft`. About is the one
 * pane that describes the running box, and showing a half-edited name under
 * "This box is called" would tell an owner their box answers to something it
 * does not yet answer to — on the screen they would go to precisely because
 * they had lost track of that.
 */

export function AboutPane({ form }: { form: SettingsForm }) {
  const t = useT();
  const saved = form.saved;

  const [updateOutput, setUpdateOutput] = React.useState("");
  const [updateError, setUpdateError] = React.useState("");
  const [updateBusy, setUpdateBusy] = React.useState(false);

  const loadUpdates = React.useCallback(async (signal?: AbortSignal) => {
    try {
      const result = await getUpdates(signal === undefined ? {} : { signal });
      setUpdateOutput(result.output);
      setUpdateError("");
    } catch (fault) {
      if (isAbort(fault) || isUnauthorized(fault)) return;
      setUpdateError(fault instanceof Error && fault.message ? fault.message : "Could not read update status");
    }
  }, []);

  React.useEffect(() => {
    const controller = new AbortController();
    void loadUpdates(controller.signal);
    return () => controller.abort();
  }, [loadUpdates]);

  const checkAndInstall = async () => {
    setUpdateBusy(true);
    try {
      await postUpdatesCheck();
      setUpdateError("");
      setUpdateOutput("Update service started. The machine may restart when activation completes.");
    } catch (fault) {
      if (!isUnauthorized(fault)) {
        setUpdateError(fault instanceof Error && fault.message ? fault.message : "Could not start the update service");
      }
    } finally {
      setUpdateBusy(false);
    }
  };

  /* Two addresses, and the order matters. `here` is the one this browser is
   * using right now, read off the address bar, so it is true by construction
   * — if it did not work, this page would not be showing. `byName` is the
   * mDNS name the box announces, which every link the apps hand out is built
   * on, and which a computer that does not resolve .local (the host of a
   * libvirt VM, say) cannot open at all. Leading with the name told an owner
   * who had just reached the box by its IP address to go somewhere that did
   * not load. */
  const here = browserOrigin();
  const byName = saved === null ? "" : `${window.location.protocol}//${saved.hostName}.local`;

  if (saved === null) {
    return (
      <PaneSection>
        <Group>
          {[0, 1, 2, 3].map((row) => (
            <Row key={row} last={row === 3}>
              <Skeleton className="h-4 w-40" />
              <Skeleton className="h-4 w-24" />
            </Row>
          ))}
        </Group>
      </PaneSection>
    );
  }

  return (
    <>
      <PaneSection>
        <GroupTitle>{t("panes.about.thisBox")}</GroupTitle>
        <Group>
          <Row>
            <RowText title={t("panes.about.called")} />
            <RowValue className="text-ink">{saved.hostName}</RowValue>
          </Row>
          <Row>
            <RowText title={t("panes.about.reachedAt")} />
            <RowValue className="text-ink">{here}</RowValue>
          </Row>
          {byName !== here && (
            <Row>
              <RowText title={t("panes.about.byName")} />
              <RowValue className="text-ink">{byName}</RowValue>
            </Row>
          )}
          <Row>
            <RowText title={t("panes.about.storage")} />
            {/* Filled for "kept to itself", hatched for "shared with the
                mesh" — the same pairing the capacity meter uses, and the only
                place in the app where two things differ by texture instead of
                by colour. Sharing switched on in a box that has not joined a
                mesh lends nothing, so it gets neither texture and says so in
                words; otherwise this row read "shared" above an "On the
                mesh: not joined" row. */}
            <Badge variant={storageBadge(saved)}>{t(storageLabel(saved))}</Badge>
          </Row>
          <Row last>
            <RowText title={t("panes.about.reachableOutside")} />
            <RowValue>{saved.proxyEnable ? t("panes.about.yes") : t("panes.about.no")}</RowValue>
          </Row>
        </Group>
        <GroupCaption>{t("panes.about.addressCaption", { name: `${saved.hostName}.local` })}</GroupCaption>
      </PaneSection>

      <PaneSection>
        <GroupTitle>{t("panes.about.handbook")}</GroupTitle>
        <Group>
          <Row last>
            <RowText title={t("panes.about.handbook")} />
            {/* A relative path on purpose: the box serves its own copy of
                the handbook at /handbook/ (modules/containers.nix), on
                whichever address this page was opened on, so the link works
                from the LAN with no internet and under the IP address as
                well as the name. It opens in a new tab so the admin key this
                tab holds is not lost to a navigation. */}
            <RowValue className="text-ink">
              <a
                href="/handbook/"
                target="_blank"
                rel="noopener"
                className="text-accent underline-offset-2 hover:underline"
              >
                {t("panes.about.handbookLink")}
              </a>
            </RowValue>
          </Row>
        </Group>
        <GroupCaption>{t("panes.about.handbookCaption")}</GroupCaption>
      </PaneSection>

      <PaneSection>
        <GroupTitle>{t("panes.about.tour")}</GroupTitle>
        <Group>
          <Row last>
            <RowText title={t("panes.about.tour")} />
            <RowValue className="text-ink">
              <button
                type="button"
                onClick={openTour}
                className="text-accent underline-offset-2 hover:underline"
              >
                {t("panes.about.tourLink")}
              </button>
            </RowValue>
          </Row>
        </Group>
        <GroupCaption>{t("panes.about.tourCaption")}</GroupCaption>
      </PaneSection>

      <PaneSection>
        <GroupTitle>{t("panes.about.spareTime")}</GroupTitle>
        <Group>
          <Row>
            <RowText title={t("panes.about.onMesh")} />
            <RowValue>{saved.clusterEnable ? t("panes.about.joined") : t("panes.about.notJoined")}</RowValue>
          </Row>
          <Row last>
            <RowText title={t("panes.about.lentOut")} />
            <RowValue>
              {saved.clusterEnable && saved.shareCompute
                ? `${saved.computeWindowStart}–${saved.computeWindowEnd}`
                : t("panes.about.never")}
            </RowValue>
          </Row>
        </Group>
        <GroupCaption>
          {saved.clusterEnable && saved.shareCompute
            ? describeWindow(saved.computeWindowStart, saved.computeWindowEnd)
            : t("panes.about.idle")}
        </GroupCaption>
      </PaneSection>

      <PaneSection>
        <GroupTitle>{t("panes.about.current")}</GroupTitle>
        <Group>
          <Row>
            <RowText title={t("panes.about.updates")} />
            <RowValue>{t("panes.about.everyNightAt", { time: "03:00" })}</RowValue>
          </Row>
          <Row last>
            <RowText title={t("panes.about.restarts")} />
            <RowValue>{t("panes.about.everyNightAt", { time: "00:07" })}</RowValue>
          </Row>
        </Group>
        <GroupCaption>{t("panes.about.caption")}</GroupCaption>
      </PaneSection>

      <PaneSection>
        <GroupTitle>System updates</GroupTitle>
        <Group>
          {updateError !== "" && (
            <p role="alert" className="text-sm text-destructive">
              {updateError}
            </p>
          )}
          {updateOutput !== "" && (
            <pre className="max-h-48 overflow-auto whitespace-pre-wrap rounded-md border border-border p-2 text-xs text-muted-foreground">
              {updateOutput}
            </pre>
          )}
          <div className="flex flex-wrap items-center gap-2">
            <Button variant="secondary" size="sm" disabled={updateBusy} onClick={() => void checkAndInstall()}>
              {updateBusy ? "Starting update…" : "Check and install now"}
            </Button>
            <Button variant="ghost" size="sm" disabled={updateBusy} onClick={() => void loadUpdates()}>
              Refresh status
            </Button>
          </div>
        </Group>
        <GroupCaption>
          Updates are signature-checked before download, then extracted on encrypted persistent storage under /persist.
        </GroupCaption>
      </PaneSection>
    </>
  );
}

type StorageState = "local" | "waiting" | "shared";

/* Storage is lent only when the switch is on AND the box is on a mesh. */
function storageState(saved: { sharingMyStorage: boolean; clusterEnable: boolean }): StorageState {
  if (!saved.sharingMyStorage) return "local";
  return saved.clusterEnable ? "shared" : "waiting";
}

function storageBadge(saved: { sharingMyStorage: boolean; clusterEnable: boolean }) {
  const state = storageState(saved);
  return state === "shared" ? "mesh" : state === "waiting" ? "outline" : "local";
}

function storageLabel(saved: { sharingMyStorage: boolean; clusterEnable: boolean }) {
  const state = storageState(saved);
  return state === "shared"
    ? "panes.about.storageShared"
    : state === "waiting"
      ? "panes.about.storageWaiting"
      : "panes.about.storageLocal";
}

/* The origin in the address bar. Empty if there is none to read (a file: URL,
 * a test harness without a window), in which case the row shows the name. */
function browserOrigin(): string {
  try {
    const origin = window.location.origin;
    return origin === "null" ? "" : origin;
  } catch {
    return "";
  }
}
