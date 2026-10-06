import * as React from "react";
import { ButtonGroup, ButtonGroupText } from "@/components/ui/button-group";
import { FieldError, Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { Rich, useT } from "@/lib/i18n-react";
import { HourStrip } from "./hour-strip";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText, StackRow } from "./rows";
import { describeWindow } from "./window";
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
 */

export function MeshPane({ form }: { form: SettingsForm }) {
  const t = useT();
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
  /* The window only means anything once this box has joined. Greyed rather
   * than hidden, because the shape of what joining gets you is half the
   * decision — and both halves go out in the same apply, so turning join on
   * and setting the hours is still one trip. */
  const windowDisabled = disabled || !joined;

  return (
    <>
      <PaneSection>
        <GroupTitle>{t("panes.mesh.otherBoxes")}</GroupTitle>
        <Group>
          <Row last>
            <RowText htmlFor={joinId} title={t("panes.mesh.join")} />
            <Switch
              id={joinId}
              checked={joined}
              disabled={disabled}
              onCheckedChange={(next) => form.set("clusterEnable", next)}
            />
          </Row>
        </Group>
        <GroupCaption>{t("panes.mesh.joinCaption")}</GroupCaption>
      </PaneSection>

      <PaneSection>
        <GroupTitle>{t("panes.mesh.spareTime")}</GroupTitle>
        <Group>
          <Row>
            <RowText
              htmlFor={shareId}
              title={t("panes.mesh.lend")}
              detail={windowDisabled && !disabled ? t("panes.mesh.joinFirst") : undefined}
            />
            <Switch
              id={shareId}
              checked={sharing}
              disabled={windowDisabled}
              onCheckedChange={(next) => form.set("shareCompute", next)}
            />
          </Row>

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
