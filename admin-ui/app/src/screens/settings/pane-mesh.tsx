import * as React from "react";
import { ButtonGroup, ButtonGroupText } from "@/components/ui/button-group";
import { FieldError, Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/progress";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { HelpLink } from "@/components/ui/help-link";
import { HugeiconsIcon } from "@hugeicons/react";
import { Alert02Icon, CheckmarkBadge01Icon } from "@hugeicons/core-free-icons";
import { cn } from "@/lib/utils";
import { Rich, useT } from "@/lib/i18n-react";
import { HourStrip } from "./hour-strip";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText, RowValue, StackRow, SwitchRow } from "./rows";
import { describeWindow } from "./window";
import { useEdge, type EdgeView } from "./use-edge";
import type { SettingsForm } from "./use-settings-form";

/* Joining the mesh, and the hours this box lends while nobody is using it.
 *
 * The one thing this pane must not get wrong is whose clock the hours are on.
 * They are the OWNER's. The taint that closes the window can only be written
 * by the edge — NodeRestriction lets nobody else — so the comparison happens
 * on a machine that is not this one, and for a while it happened against the
 * edge's own clock: a 23:00→07:00 window entered in Berlin was enforced
 * 00:00→08:00 in winter and 01:00→09:00 in summer, sliding an hour at each
 * DST change, which handed strangers the first hours of the owner's working
 * day — the exact thing the feature exists to prevent. The zone now travels
 * with the two bounds and the edge evaluates each box in its own.
 *
 * So the copy says "your local time", the strip is drawn in local hours with
 * no conversion anywhere, and the caption says the hours travel with the
 * setting. That sentence is load-bearing, not reassurance.
 *
 * Above all of it sits the edge proxy. The mesh is other boxes behind an
 * edge, and lososd looks for one every few seconds (on the LAN by DNS-SD and
 * at the configured registrar) and refuses to turn joining on when none
 * answers. The first group shows that reading, and the Join switch is greyed
 * with the same reason while nothing is in reach — a switch the box is going
 * to refuse should not look like it will take. A box already joined keeps
 * its switch live, so the owner can still leave while the edge is away.
 */

/* The first group: what the box found when it last looked for an edge.
 *
 * One row per edge, because a box may have several in reach at once (its
 * own network's and the public one, say): sharing is allowed while ANY of
 * them answers, and trading only through one that proved it is LosOS's.
 * That second fact sits next to each edge's name as a sign with a tooltip:
 * a check for an official edge, a warning for any other, whose tooltip
 * lists what that edge cannot do for this box. The sign is a button so the
 * same words are its accessible name — Base UI shows no tooltip on touch,
 * and a settings row must not hide half its meaning in a hover. */

function EdgeSign({ official }: { official: boolean }) {
  const t = useT();
  const label = t(official ? "panes.mesh.edge.officialTip" : "panes.mesh.edge.companyTip");
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <button
            type="button"
            aria-label={label}
            data-edge-sign={official ? "official" : "warning"}
            className={cn(
              "inline-flex size-5 shrink-0 items-center justify-center rounded-full align-middle",
              "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand",
              official ? "text-ok" : "text-warn",
            )}
          />
        }
      >
        <HugeiconsIcon icon={official ? CheckmarkBadge01Icon : Alert02Icon} size={16} strokeWidth={2} />
      </TooltipTrigger>
      <TooltipContent side="top" align="start" className="max-w-[22rem] whitespace-pre-line">
        {label}
      </TooltipContent>
    </Tooltip>
  );
}

export function EdgeGroup({ view }: { view: EdgeView }) {
  const t = useT();
  const { state } = view;
  if (state.kind === "known" && state.edge.reachable) {
    const edges = state.edge.edges;
    return (
      <PaneSection>
        <GroupTitle>{t("panes.mesh.edge.title")}</GroupTitle>
        <TooltipProvider>
          <Group>
            {edges.map((edge, i) => (
              <Row
                key={edge.url}
                last={i === edges.length - 1}
                data-edge="found"
                data-edge-official={edge.official ? "yes" : "no"}
              >
                <RowText
                  title={
                    <span className="inline-flex items-center gap-1.5">
                      <span>{t("panes.mesh.edge.found", { name: edge.name })}</span>
                      <EdgeSign official={edge.official} />
                    </span>
                  }
                  detail={edge.url}
                />
                <RowValue className="text-ok">
                  {t(edge.source === "configured" ? "panes.mesh.edge.viaInternet" : "panes.mesh.edge.viaLan")}
                </RowValue>
              </Row>
            ))}
          </Group>
        </TooltipProvider>
        <GroupCaption>
          {t(state.edge.official ? "panes.mesh.edge.caption" : "panes.mesh.edge.captionCompany")}
        </GroupCaption>
      </PaneSection>
    );
  }
  let title: string;
  let detail: React.ReactNode;
  let value: React.ReactNode = null;
  let tone: string | undefined;
  if (state.kind === "loading") {
    title = t("panes.mesh.edge.looking");
    value = <Spinner size={16} />;
  } else if (state.kind === "failed") {
    title = t("panes.mesh.edge.unknown");
    detail = state.message;
  } else {
    title = t("panes.mesh.edge.none");
    const tried = state.edge.configuredUrl;
    const why = state.edge.lanSearched
      ? tried === null
        ? t("panes.mesh.edge.noneDetail")
        : t("panes.mesh.edge.noneTried", { url: tried })
      : t("panes.mesh.edge.lanUnsearched");
    /* The refusal the switches below are about to give has a handbook page;
     * it is linked here, where the owner reads why, rather than only from
     * the 409 toast an apply would raise. */
    detail = (
      <>
        {why} <HelpLink entry="edge-not-found" />
      </>
    );
    value = t("panes.mesh.edge.sharingOff");
    tone = "text-danger";
  }
  return (
    <PaneSection>
      <GroupTitle>{t("panes.mesh.edge.title")}</GroupTitle>
      <Group>
        <Row last data-edge={state.kind === "known" ? "none" : state.kind}>
          <RowText title={title} detail={detail} />
          <RowValue className={tone}>{value}</RowValue>
        </Row>
      </Group>
      <GroupCaption>{t("panes.mesh.edge.caption")}</GroupCaption>
    </PaneSection>
  );
}

export function MeshPane({ form }: { form: SettingsForm }) {
  const t = useT();
  const edge = useEdge();
  const joinId = React.useId();
  const shareId = React.useId();
  const startId = React.useId();
  const endId = React.useId();
  const windowErrorId = React.useId();

  const draft = form.draft;
  const joined = draft?.clusterEnable ?? false;
  const sharing = draft?.shareCompute ?? false;
  const start = draft?.computeWindowStart ?? "";
  const end = draft?.computeWindowEnd ?? "";

  const disabled = form.locked || !form.ready;
  /* No edge in reach and not joined yet: the daemon would refuse the join,
   * so the switch is greyed and says why. Already joined (as saved) stays
   * live, so the owner can leave the mesh while the edge is away. */
  const joinRefused = edge.blocked && !(form.saved?.clusterEnable ?? false);
  const joinDisabled = disabled || joinRefused;
  /* The window only means anything once this box has joined. Greyed rather
   * than hidden, because the shape of what joining gets you is half the
   * decision — and both halves go out in the same apply, so turning join on
   * and setting the hours is still one trip. */
  const windowDisabled = disabled || !joined;

  return (
    <>
      <EdgeGroup view={edge} />

      <PaneSection>
        <GroupTitle>{t("panes.mesh.otherBoxes")}</GroupTitle>
        <Group>
          <SwitchRow
            id={joinId}
            title={t("panes.mesh.join")}
            description={joinRefused && !disabled ? t("panes.mesh.edge.needed") : undefined}
            checked={joined}
            disabled={joinDisabled}
            onCheckedChange={(next) => form.set("clusterEnable", next)}
            last
          />
        </Group>
        <GroupCaption>{t("panes.mesh.joinCaption")}</GroupCaption>
      </PaneSection>

      <PaneSection>
        <GroupTitle>{t("panes.mesh.spareTime")}</GroupTitle>
        <Group>
          <SwitchRow
            id={shareId}
            title={t("panes.mesh.lend")}
            description={windowDisabled && !disabled ? t("panes.mesh.joinFirst") : undefined}
            checked={sharing}
            disabled={windowDisabled}
            onCheckedChange={(next) => form.set("shareCompute", next)}
          />

          <StackRow>
            <HourStrip start={start} end={end} muted={!sharing || windowDisabled} />
            <p className="text-[12.5px] leading-snug text-muted" aria-hidden="true">
              {describeWindow(start, end)}
            </p>
          </StackRow>

          <Row last>
            {/* No `htmlFor` here: the row heads a PAIR of fields, and each
                carries its own name below. Pointing this one at the start
                field would name it "Hours Lend from" to a screen reader. */}
            <RowText title={t("panes.mesh.hours")} />
            {/* A shadcn Button Group: from, the word between, until — one
                control with shared edges, not three widgets in a row. */}
            <ButtonGroup aria-label={t("panes.mesh.hours")}>
              <label htmlFor={startId} className="sr-only">
                {t("panes.mesh.lendFrom")}
              </label>
              <Input
                id={startId}
                type="time"
                className="numeric w-[7.5rem]"
                value={start}
                disabled={windowDisabled}
                aria-invalid={form.problems.computeWindow !== null}
                aria-describedby={
                  form.problems.computeWindow === null ? undefined : windowErrorId
                }
                onChange={(event) => form.set("computeWindowStart", event.target.value)}
              />
              <ButtonGroupText>{t("panes.mesh.until")}</ButtonGroupText>
              <label htmlFor={endId} className="sr-only">
                {t("panes.mesh.lendUntil")}
              </label>
              <Input
                id={endId}
                type="time"
                className="numeric w-[7.5rem]"
                value={end}
                disabled={windowDisabled}
                aria-invalid={form.problems.computeWindow !== null}
                aria-describedby={
                  form.problems.computeWindow === null ? undefined : windowErrorId
                }
                onChange={(event) => form.set("computeWindowEnd", event.target.value)}
              />
            </ButtonGroup>
          </Row>
        </Group>

        <FieldError id={windowErrorId} className="px-1.5 pt-2">
          {form.problems.computeWindow}
        </FieldError>

        <GroupCaption>
          <Rich k="panes.mesh.windowCaption" vars={{ your: <em>{t("panes.mesh.your")}</em> }} />
        </GroupCaption>
      </PaneSection>
    </>
  );
}
