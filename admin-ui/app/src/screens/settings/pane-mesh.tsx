import * as React from "react";
import { FieldError, Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
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
        <GroupTitle>Other boxes</GroupTitle>
        <Group>
          <Row last>
            <RowText htmlFor={joinId} title="Join the mesh" />
            <Switch
              id={joinId}
              checked={joined}
              disabled={disabled}
              onCheckedChange={(next) => form.set("clusterEnable", next)}
            />
          </Row>
        </Group>
        <GroupCaption>
          The mesh is other people&apos;s boxes, running the same system as this one. Joining lets
          them keep copies of your files and lets you keep copies of theirs, and it is what makes
          the hours below worth anything.
        </GroupCaption>
      </PaneSection>

      <PaneSection>
        <GroupTitle>Spare time</GroupTitle>
        <Group>
          <Row>
            <RowText
              htmlFor={shareId}
              title="Lend this box while I sleep"
              detail={windowDisabled && !disabled ? "Join the mesh first." : undefined}
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
            <RowText title="Hours" />
            <div className="flex items-center gap-2">
              <label htmlFor={startId} className="sr-only">
                Lend from
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
              <span className="text-[12.5px] text-muted">until</span>
              <label htmlFor={endId} className="sr-only">
                Lend until
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
            </div>
          </Row>
        </Group>

        <FieldError id={windowErrorId} className="px-1.5 pt-2">
          {form.problems.computeWindow}
        </FieldError>

        <GroupCaption>
          These are the hours on <em>your</em> clock, and they travel with the setting. The box that
          hands out the work is told which time zone you meant, so the window does not slide by an
          hour when the clocks change. The end is the moment it stops. Set it to 07:00 and seven
          o&apos;clock is yours again. An end earlier than the start simply runs through midnight,
          which is what &ldquo;while I sleep&rdquo; usually means. Work already running is left to
          finish; nothing new starts once the window shuts.
        </GroupCaption>
      </PaneSection>
    </>
  );
}
