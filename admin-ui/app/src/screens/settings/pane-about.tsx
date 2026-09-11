import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
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
        <GroupTitle>This box</GroupTitle>
        <Group>
          <Row>
            <RowText title="Called" />
            <RowValue className="text-ink">{saved.hostName}</RowValue>
          </Row>
          <Row>
            <RowText title="Reached at" />
            <RowValue className="text-ink">
              {`${saved.https ? "https" : "http"}://${saved.hostName}.local`}
            </RowValue>
          </Row>
          <Row>
            <RowText title="Storage" />
            {/* Filled for "kept to itself", hatched for "shared with the
                mesh" — the same pairing the capacity meter uses, and the only
                place in the app where two things differ by texture instead of
                by colour. */}
            <Badge variant={saved.sharingMyStorage ? "mesh" : "local"}>
              {saved.sharingMyStorage ? "shared with the mesh" : "kept to itself"}
            </Badge>
          </Row>
          <Row last>
            <RowText title="Reachable from outside" />
            <RowValue>{saved.proxyEnable ? "yes" : "no"}</RowValue>
          </Row>
        </Group>
      </PaneSection>

      <PaneSection>
        <GroupTitle>Spare time</GroupTitle>
        <Group>
          <Row>
            <RowText title="On the mesh" />
            <RowValue>{saved.clusterEnable ? "joined" : "not joined"}</RowValue>
          </Row>
          <Row last>
            <RowText title="Lent out" />
            <RowValue>
              {saved.clusterEnable && saved.shareCompute
                ? `${saved.computeWindowStart}–${saved.computeWindowEnd}`
                : "never"}
            </RowValue>
          </Row>
        </Group>
        <GroupCaption>
          {saved.clusterEnable && saved.shareCompute
            ? describeWindow(saved.computeWindowStart, saved.computeWindowEnd)
            : "Nothing on this box is doing work for anyone else."}
        </GroupCaption>
      </PaneSection>

      <PaneSection>
        <GroupTitle>Keeping itself current</GroupTitle>
        <Group>
          <Row>
            <RowText title="Looks for updates" />
            <RowValue>every night at 03:00</RowValue>
          </Row>
          <Row last>
            <RowText title="Restarts" />
            <RowValue>every night at 00:07</RowValue>
          </Row>
        </Group>
        <GroupCaption>
          This box keeps itself up to date on its own and restarts once a night so nothing is left
          half-applied. If it is switched off at the time, it catches up the next time it is on.
          Your admin key is remembered only until you close this tab.
        </GroupCaption>
      </PaneSection>
    </>
  );
}
