import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import { useT } from "@/lib/i18n-react";
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

  /* Two addresses, and the order matters. `here` is the one this browser is
   * using right now, read off the address bar, so it is true by construction
   * — if it did not work, this page would not be showing. `byName` is the
   * mDNS name the box announces, which every link the apps hand out is built
   * on, and which a computer that does not resolve .local (the host of a
   * libvirt VM, say) cannot open at all. Leading with the name told an owner
   * who had just reached the box by its IP address to go somewhere that did
   * not load. */
  const here = browserOrigin();
  const byName = saved === null ? "" : `${saved.https ? "https" : "http"}://${saved.hostName}.local`;

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
                by colour. */}
            <Badge variant={saved.sharingMyStorage ? "mesh" : "local"}>
              {saved.sharingMyStorage
                ? t("panes.about.storageShared")
                : t("panes.about.storageLocal")}
            </Badge>
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
    </>
  );
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
