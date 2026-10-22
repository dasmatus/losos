import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Alert02Icon, Flag02Icon, Task01Icon } from "@hugeicons/core-free-icons";
import { PersonAvatar, RoleBadge, type PersonLike } from "@/components/person";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogBody,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { HelpLink } from "@/components/ui/help-link";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Spinner } from "@/components/ui/progress";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import type { ModerationLogEntry, ModerationPerson, ModerationView, Report } from "@/lib/api";
import { intlTag, type MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText, StackRow } from "./rows";
import { useModeration, type HoldKind, type ModerationData } from "./use-moderation";

/* Moderation: what people on this box reported, who is muted or suspended,
 * and the log of what moderators did (backend/src/moderation.rs).
 *
 * A report names what it is about and never carries it, so nothing on this
 * pane opens a message or shows a picture. A report of a child being harmed
 * comes first, says not to open it, and offers only "hide without opening"
 * (or, where the app cannot hide it from here, "reported to the police").
 * Moderators see the people they may act on with buttons; everyone else is
 * listed without, since lososd would refuse. */

export function ModerationPane({ locked }: { locked: boolean }) {
  const t = useT();
  const data = useModeration(!locked);
  const { state } = data;

  if (state.kind === "loading") {
    return (
      <PaneSection>
        <Group>
          <Row last>
            <RowText title={t("moderation.loading")} />
            <Spinner size={16} />
          </Row>
        </Group>
      </PaneSection>
    );
  }
  if (state.kind === "unavailable") {
    return (
      <PaneSection data-testid="moderation-unavailable">
        <Group>
          <Row last>
            <RowText title={t("users.unavailable")} />
          </Row>
        </Group>
      </PaneSection>
    );
  }
  if (state.kind === "failed") {
    return (
      <PaneSection>
        <Alert variant="crit">
          <HugeiconsIcon icon={Alert02Icon} strokeWidth={1.5} aria-hidden="true" />
          <AlertTitle>{t("moderation.failed")}</AlertTitle>
          <AlertDescription>
            <p>{state.message}</p>
            <Button size="sm" variant="secondary" className="mt-2" onClick={data.refresh}>
              {t("moderation.retry")}
            </Button>
          </AlertDescription>
        </Alert>
      </PaneSection>
    );
  }

  const disabled = locked || data.busy;
  return (
    <>
      <ReportsSection view={state.view} data={data} disabled={disabled} />
      <PeopleSection view={state.view} data={data} disabled={disabled} />
      <LogSection view={state.view} />
    </>
  );
}

// ── Helpers ─────────────────────────────────────────────────────────────

function when(seconds: number): string {
  return new Intl.DateTimeFormat(intlTag(), { dateStyle: "medium", timeStyle: "short" }).format(
    new Date(seconds * 1000),
  );
}

/** A name as a person to draw: their profile when they are on the box. */
function personOf(view: ModerationView, name: string): PersonLike {
  return view.people.find((p) => p.name === name) ?? { name, displayName: name };
}

function nameOf(view: ModerationView, name: string): string {
  if (name === "box") return "";
  return view.people.find((p) => p.name === name)?.displayName ?? name;
}

const REASON_BADGE = {
  spam: "neutral",
  abuse: "warn",
  sexual: "warn",
  child: "crit",
  other: "neutral",
} as const;

// ── Reports ─────────────────────────────────────────────────────────────

function ReportsSection({ view, data, disabled }: { view: ModerationView; data: ModerationData; disabled: boolean }) {
  const t = useT();
  const open = view.reports.filter((r) => r.state === "open");
  const closed = view.reports.filter((r) => r.state !== "open").slice(0, 10);
  return (
    <PaneSection data-testid="moderation-reports">
      <GroupTitle>{t("moderation.reports.title")}</GroupTitle>
      {open.length === 0 && (
        <Empty className="bg-surface" data-testid="moderation-reports-empty">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <HugeiconsIcon icon={Flag02Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            </EmptyMedia>
            <EmptyTitle>{t("moderation.reports.emptyTitle")}</EmptyTitle>
            <EmptyDescription>{t("moderation.reports.emptyBody")}</EmptyDescription>
          </EmptyHeader>
        </Empty>
      )}
      {open.length > 0 && (
        <Group>
          {open.map((report, i) => (
            <ReportRow
              key={report.id}
              report={report}
              view={view}
              data={data}
              disabled={disabled}
              last={i === open.length - 1}
            />
          ))}
        </Group>
      )}
      {closed.length > 0 && (
        <>
          <GroupTitle className="pt-4">{t("moderation.reports.closed")}</GroupTitle>
          <Group>
            {closed.map((report, i) => (
              <ReportRow
                key={report.id}
                report={report}
                view={view}
                data={data}
                disabled={disabled}
                last={i === closed.length - 1}
              />
            ))}
          </Group>
        </>
      )}
      <GroupCaption>{t("moderation.reports.caption")}</GroupCaption>
    </PaneSection>
  );
}

function thingOf(report: Report): MessageKey {
  if (report.app === "profile") {
    return report.ref === "banner" ? "moderation.report.thing.banner" : "moderation.report.thing.picture";
  }
  if (report.app === "mail" || report.app === "chat") return "moderation.report.thing.message";
  return "moderation.report.thing.other";
}

function ReportRow({
  report,
  view,
  data,
  disabled,
  last,
}: {
  report: Report;
  view: ModerationView;
  data: ModerationData;
  disabled: boolean;
  last: boolean;
}) {
  const t = useT();
  const child = report.reason === "child";
  const open = report.state === "open";
  const canHide = view.hides.includes(report.app);
  const about = report.about !== null ? personOf(view, report.about) : null;
  return (
    <StackRow last={last} className="gap-2" data-testid="moderation-report" data-reason={report.reason} data-state={report.state}>
      <div className="flex flex-wrap items-center gap-x-2 gap-y-1.5">
        <Badge variant={REASON_BADGE[report.reason]}>{t(`moderation.reason.${report.reason}`)}</Badge>
        <span className="text-[13px] text-muted">
          {t(`moderation.app.${report.app}`)} · {t(thingOf(report))}
        </span>
        <span className="numeric ml-auto text-[12.5px] text-faint">{when(report.at)}</span>
      </div>
      <div className="flex flex-wrap items-center gap-x-4 gap-y-1.5 text-[13px] text-muted">
        <span className="inline-flex items-center gap-1.5">
          <PersonAvatar person={personOf(view, report.by)} size="xs" />
          {t("moderation.report.by", { name: nameOf(view, report.by) })}
        </span>
        {about !== null && (
          <span className="inline-flex items-center gap-1.5">
            <PersonAvatar person={about} size="xs" />
            {t("moderation.report.about", { name: about.displayName })}
          </span>
        )}
      </div>
      {report.note.length > 0 && (
        <p className="border-l-2 border-line pl-3 text-[13.5px] leading-snug text-ink">{report.note}</p>
      )}
      {child && open && (
        <Alert variant="crit" data-testid="moderation-child">
          <HugeiconsIcon icon={Alert02Icon} strokeWidth={1.5} aria-hidden="true" />
          <AlertTitle>{t("moderation.child.title")}</AlertTitle>
          <AlertDescription>
            <p>{t("moderation.child.body")}</p>
            <HelpLink entry="moderation" label={t("moderation.child.where")} className="mt-1" />
          </AlertDescription>
        </Alert>
      )}
      {open ? (
        <div className="flex flex-wrap justify-end gap-2 pt-0.5">
          {child ? (
            canHide ? (
              <Button size="sm" disabled={disabled} data-testid="moderation-hide" onClick={() => void data.act(report.id, "hide")}>
                {t("moderation.child.hide")}
              </Button>
            ) : (
              <Button size="sm" disabled={disabled} data-testid="moderation-resolve" onClick={() => void data.act(report.id, "resolve")}>
                {t("moderation.child.reported")}
              </Button>
            )
          ) : (
            <>
              <Button size="sm" variant="ghost" disabled={disabled} data-testid="moderation-dismiss" onClick={() => void data.act(report.id, "dismiss")}>
                {t("moderation.action.dismiss")}
              </Button>
              <Button size="sm" variant="secondary" disabled={disabled} data-testid="moderation-resolve" onClick={() => void data.act(report.id, "resolve")}>
                {t("moderation.action.resolve")}
              </Button>
              {canHide && (
                <Button size="sm" disabled={disabled} data-testid="moderation-hide" onClick={() => void data.act(report.id, "hide")}>
                  {report.app === "mail"
                    ? t("moderation.action.holdMail")
                    : report.app === "chat"
                      ? t("moderation.action.hideMessage")
                      : t("moderation.action.takeDown")}
                </Button>
              )}
            </>
          )}
        </div>
      ) : (
        <div className="flex flex-wrap items-center justify-end gap-2 pt-0.5">
          <span className="text-[12.5px] text-muted">
            {t("moderation.state.by", {
              state: t(`moderation.state.${report.state === "hidden" ? "hidden" : report.state === "dismissed" ? "dismissed" : "resolved"}`),
              name: nameOf(view, report.handledBy ?? ""),
            })}
          </span>
          {report.state === "hidden" && report.app === "mail" && !child && (
            <Button size="sm" variant="secondary" disabled={disabled} data-testid="moderation-release" onClick={() => void data.act(report.id, "release")}>
              {t("moderation.action.release")}
            </Button>
          )}
        </div>
      )}
    </StackRow>
  );
}

// ── People ──────────────────────────────────────────────────────────────

function PeopleSection({ view, data, disabled }: { view: ModerationView; data: ModerationData; disabled: boolean }) {
  const t = useT();
  const [holding, setHolding] = React.useState<{ kind: "mute" | "suspend"; person: ModerationPerson } | null>(null);
  return (
    <PaneSection data-testid="moderation-people">
      <GroupTitle>{t("moderation.people.title")}</GroupTitle>
      <Group>
        {view.people.map((person, i) => (
          <StackRow key={person.name} last={i === view.people.length - 1} className="gap-1.5" data-testid="moderation-person" data-person={person.name}>
            <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
              <PersonAvatar person={person} size="md" showRole />
              <div className="min-w-0 flex-1">
                <p className="truncate text-sm leading-snug text-ink">{person.displayName}</p>
                <p className="truncate text-[12.5px] leading-snug text-muted">@{person.name}</p>
              </div>
              <div className="flex shrink-0 flex-wrap items-center gap-1.5">
                {person.suspended !== null && (
                  <Badge variant="crit" data-testid="moderation-suspended">
                    {person.suspended.until === null
                      ? t("moderation.suspended")
                      : t("moderation.suspendedUntil", { time: when(person.suspended.until) })}
                  </Badge>
                )}
                {person.muted !== null && (
                  <Badge variant="warn" data-testid="moderation-muted">
                    {person.muted.until === null
                      ? t("moderation.muted")
                      : t("moderation.mutedUntil", { time: when(person.muted.until) })}
                  </Badge>
                )}
                <RoleBadge person={person} />
              </div>
            </div>
            {person.actionable && (
              <div className="flex flex-wrap justify-end gap-2 pt-0.5">
                {person.muted !== null ? (
                  <Button size="sm" variant="secondary" disabled={disabled} data-testid="moderation-unmute" onClick={() => void data.hold("unmute", person.name)}>
                    {t("moderation.action.unmute")}
                  </Button>
                ) : (
                  <Button size="sm" variant="secondary" disabled={disabled} data-testid="moderation-mute" onClick={() => setHolding({ kind: "mute", person })}>
                    {t("moderation.action.mute")}
                  </Button>
                )}
                {person.suspended !== null ? (
                  <Button size="sm" variant="secondary" disabled={disabled} data-testid="moderation-unsuspend" onClick={() => void data.hold("unsuspend", person.name)}>
                    {t("moderation.action.unsuspend")}
                  </Button>
                ) : (
                  <Button size="sm" variant="secondary" disabled={disabled} data-testid="moderation-suspend" onClick={() => setHolding({ kind: "suspend", person })}>
                    {t("moderation.action.suspend")}
                  </Button>
                )}
              </div>
            )}
          </StackRow>
        ))}
      </Group>
      <GroupCaption>{t("moderation.people.caption")}</GroupCaption>
      <HoldDialog holding={holding} data={data} onClose={() => setHolding(null)} />
    </PaneSection>
  );
}

/** How long a hold runs, in minutes; null until lifted. */
const DURATIONS: { id: string; label: MessageKey; minutes: number | null }[] = [
  { id: "hour", label: "moderation.for.hour", minutes: 60 },
  { id: "day", label: "moderation.for.day", minutes: 24 * 60 },
  { id: "week", label: "moderation.for.week", minutes: 7 * 24 * 60 },
  { id: "month", label: "moderation.for.month", minutes: 30 * 24 * 60 },
  { id: "ever", label: "moderation.for.ever", minutes: null },
];

function HoldDialog({
  holding,
  data,
  onClose,
}: {
  holding: { kind: "mute" | "suspend"; person: ModerationPerson } | null;
  data: ModerationData;
  onClose: () => void;
}) {
  const t = useT();
  const titleId = React.useId();
  const bodyId = React.useId();
  const forId = React.useId();
  const reasonId = React.useId();
  const [duration, setDuration] = React.useState("day");
  const [reason, setReason] = React.useState("");

  React.useEffect(() => {
    if (holding === null) return;
    setDuration("day");
    setReason("");
  }, [holding]);

  const suspend = holding?.kind === "suspend";
  const name = holding?.person.displayName ?? "";
  const submit = async () => {
    if (holding === null) return;
    const minutes = DURATIONS.find((d) => d.id === duration)?.minutes ?? null;
    const kind: HoldKind = holding.kind;
    if (await data.hold(kind, holding.person.name, minutes, reason)) onClose();
  };

  return (
    <Dialog
      open={holding !== null}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      labelledBy={titleId}
      describedBy={bodyId}
      dialogClassName="w-[min(30rem,calc(100vw-2rem))]"
    >
      <form
        onSubmit={(event) => {
          event.preventDefault();
          void submit();
        }}
      >
        <DialogHeader>
          <DialogTitle id={titleId}>
            {suspend ? t("moderation.suspendTitle", { name }) : t("moderation.muteTitle", { name })}
          </DialogTitle>
          <DialogDescription id={bodyId}>
            {suspend ? t("moderation.suspendBody") : t("moderation.muteBody")}
          </DialogDescription>
        </DialogHeader>
        <DialogBody>
          <Group>
            <Row>
              <RowText htmlFor={forId} title={t("moderation.for")} />
              <div className="w-[12rem] max-sm:w-[10rem]">
                <NativeSelect
                  id={forId}
                  className="w-full"
                  value={duration}
                  data-testid="moderation-duration"
                  onChange={(event) => setDuration(event.target.value)}
                >
                  {DURATIONS.map((d) => (
                    <NativeSelectOption key={d.id} value={d.id}>
                      {t(d.label)}
                    </NativeSelectOption>
                  ))}
                </NativeSelect>
              </div>
            </Row>
            <StackRow last className="gap-1.5">
              <label htmlFor={reasonId} className="text-sm text-ink">
                {t("moderation.reason")}
              </label>
              <Input
                id={reasonId}
                data-testid="moderation-reason"
                value={reason}
                maxLength={300}
                placeholder={t("moderation.reasonPlaceholder")}
                onChange={(event) => setReason(event.target.value)}
              />
              <p className="text-[12.5px] leading-snug text-muted">{t("moderation.reasonDetail")}</p>
            </StackRow>
          </Group>
        </DialogBody>
        <DialogFooter>
          <Button type="button" variant="ghost" onClick={onClose}>
            {t("moderation.cancel")}
          </Button>
          <Button type="submit" disabled={data.busy} data-testid="moderation-hold-confirm">
            {suspend ? t("moderation.action.suspend") : t("moderation.action.mute")}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}

// ── Log ─────────────────────────────────────────────────────────────────

const LOG_KEYS: Record<string, MessageKey> = {
  report: "moderation.log.report",
  hide: "moderation.log.hide",
  release: "moderation.log.release",
  dismiss: "moderation.log.dismiss",
  resolve: "moderation.log.resolve",
  mute: "moderation.log.mute",
  unmute: "moderation.log.unmute",
  suspend: "moderation.log.suspend",
  unsuspend: "moderation.log.unsuspend",
  "mute ended": "moderation.log.muteEnded",
  "suspension ended": "moderation.log.suspensionEnded",
  role: "moderation.log.role",
  "role saved": "moderation.log.roleSaved",
  "role removed": "moderation.log.roleRemoved",
  remove: "moderation.log.remove",
  "take down": "moderation.log.takeDown",
};

/** A role as the log names it, in the reader's language when it is one of
 *  the built-in ones. */
function roleWord(t: ReturnType<typeof useT>, detail: string): string {
  return ["owner", "admin", "moderator", "member", "guest"].includes(detail)
    ? t(`roles.${detail as "owner" | "admin" | "moderator" | "member" | "guest"}`)
    : detail;
}

function LogLine({ entry, view }: { entry: ModerationLogEntry; view: ModerationView }) {
  const t = useT();
  const about = entry.about !== null ? nameOf(view, entry.about) : "";
  if (entry.action === "report" && entry.about === null) return <>{t("moderation.log.reportNobody")}</>;
  const key = LOG_KEYS[entry.action] ?? "moderation.log.other";
  const detail =
    entry.action === "role"
      ? roleWord(t, entry.detail)
      : entry.action === "take down"
        ? entry.detail === "banner"
          ? t("moderation.report.thing.banner")
          : t("moderation.report.thing.picture")
        : entry.detail;
  return <>{t(key, { about, detail })}</>;
}

function LogSection({ view }: { view: ModerationView }) {
  const t = useT();
  const who = (by: string) =>
    by === "box" ? t("moderation.log.box") : by === "spare-key" ? t("moderation.log.spareKey") : nameOf(view, by);
  return (
    <PaneSection data-testid="moderation-log">
      <GroupTitle>{t("moderation.log.title")}</GroupTitle>
      {view.log.length === 0 ? (
        <Empty className="bg-surface">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <HugeiconsIcon icon={Task01Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            </EmptyMedia>
            <EmptyTitle>{t("moderation.log.emptyTitle")}</EmptyTitle>
            <EmptyDescription>{t("moderation.log.emptyBody")}</EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : (
        <Group className="overflow-x-auto">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>{t("moderation.log.when")}</TableHead>
                <TableHead>{t("moderation.log.who")}</TableHead>
                <TableHead>{t("moderation.log.what")}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {view.log.map((entry, i) => (
                <TableRow key={`${entry.at}-${i}`} data-testid="moderation-log-line" data-action={entry.action}>
                  <TableCell className="numeric whitespace-nowrap text-muted">{when(entry.at)}</TableCell>
                  <TableCell className="whitespace-nowrap text-ink">{who(entry.by)}</TableCell>
                  <TableCell className="text-muted">
                    <LogLine entry={entry} view={view} />
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </Group>
      )}
    </PaneSection>
  );
}
