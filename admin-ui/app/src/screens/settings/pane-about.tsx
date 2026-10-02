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
            <RowValue className="text-ink">
              {`${saved.https ? "https" : "http"}://${saved.hostName}.local`}
            </RowValue>
          </Row>
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
