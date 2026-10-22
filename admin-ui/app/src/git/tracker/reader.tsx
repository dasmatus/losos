import * as React from "react";
import { Link } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  AiMagicIcon,
  ArrowLeft01Icon,
  Calendar03Icon,
  Copy01Icon,
  Delete02Icon,
  GitCommitIcon,
  GitPullRequestIcon,
  Link01Icon,
  Mail01Icon,
  Message01Icon,
  MoreHorizontalIcon,
  Notification01Icon,
  NotificationOff01Icon,
  PencilEdit02Icon,
  UserCircleIcon,
} from "@hugeicons/core-free-icons";
import { useMentions } from "@/components/mention-picker";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Spinner } from "@/components/ui/spinner";
import { Textarea } from "@/components/ui/textarea";
import { toast } from "@/components/ui/toast";
import type { MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { when, whenFull } from "../format";
import { Markdown } from "../markdown";
import { ErrorAlert, failed, Person } from "../parts";
import { useLoad } from "../store";
import { closed, tracker, type Fields, type IssueDetail, type Priority, type RelationKind } from "./api";
import {
  Assignee,
  AssigneeMenu,
  CycleMenu,
  DueDate,
  LabelChip,
  LabelMenu,
  PRIORITY_NAME,
  PriorityIcon,
  PriorityMenu,
  StateIcon,
  StateMenu,
  cycleName,
  EstimateMenu,
} from "./parts";
import { canHandTo } from "./screen";
import { useIssueKeys, useNameOf, useTracker } from "./store";

/* The open issue: its title and description, everything one can set on it
 * as a row of buttons that each open their menu, what is under it and what
 * it waits on, the commits and pull requests that name it, and one
 * timeline of what was said and what changed. */

type MenuName = "state" | "priority" | "assignee" | "labels";

export function Reader({
  issueKey,
  round,
  onBack,
  onChanged,
  onOpen,
  onCreateChild,
  menu,
  onMenu,
}: {
  issueKey: string;
  round: number;
  onBack: () => void;
  onChanged: () => void;
  onOpen: (key: string) => void;
  onCreateChild: (parent: string, project: string) => void;
  menu: MenuName | null;
  onMenu: (m: MenuName | null) => void;
}) {
  const t = useT();
  const loaded = useLoad((signal) => tracker.issue(issueKey, signal), [issueKey, round]);
  const [issue, setIssue] = React.useState<IssueDetail | null>(null);
  React.useEffect(() => {
    if (loaded.data !== null) setIssue(loaded.data);
  }, [loaded.data]);

  if (issue === null || issue.key !== issueKey) {
    return (
      <div className="flex flex-1 flex-col">
        <ReaderBar issueKey={issueKey} onBack={onBack} />
        <div className="m-auto p-6">{loaded.error !== null ? <ErrorAlert error={loaded.error} /> : <Spinner />}</div>
      </div>
    );
  }
  return (
    <Open
      issue={issue}
      onBack={onBack}
      onOpen={onOpen}
      onCreateChild={onCreateChild}
      menu={menu}
      onMenu={onMenu}
      onSaved={(next) => {
        setIssue(next);
        onChanged();
      }}
      reload={() => {
        loaded.reload();
        onChanged();
      }}
      onDeleted={() => {
        onBack();
        onChanged();
        toast.done(t("tracker.done.deleted", { key: issue.key }));
      }}
    />
  );
}

function ReaderBar({ issueKey, onBack, children }: { issueKey: string; onBack: () => void; children?: React.ReactNode }) {
  const t = useT();
  return (
    <div className="flex items-center gap-1.5 border-b border-hair px-3 py-2">
      <Button variant="ghost" size="icon-sm" onClick={onBack} aria-label={t("tracker.reader.back")} title={t("tracker.reader.back")}>
        <HugeiconsIcon icon={ArrowLeft01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      </Button>
      <span className="min-w-0 flex-1 truncate font-mono text-[12.5px] text-muted">{issueKey}</span>
      {children}
    </div>
  );
}

function Open({
  issue,
  onBack,
  onOpen,
  onCreateChild,
  menu,
  onMenu,
  onSaved,
  reload,
  onDeleted,
}: {
  issue: IssueDetail;
  onBack: () => void;
  onOpen: (key: string) => void;
  onCreateChild: (parent: string, project: string) => void;
  menu: MenuName | null;
  onMenu: (m: MenuName | null) => void;
  onSaved: (next: IssueDetail) => void;
  reload: () => void;
  onDeleted: () => void;
}) {
  const t = useT();
  const { home } = useTracker();
  const nameOf = useNameOf();
  const keys = useIssueKeys();
  const project = issue.project;
  const write = issue.permission === "write" || issue.permission === "admin";
  const today = home?.today ?? "";
  const ai = home?.ai === true;

  const save = async (fields: Fields, done?: MessageKey) => {
    try {
      onSaved(await tracker.edit(issue.key, fields));
      if (done !== undefined) toast.done(t(done, { count: 1 }));
    } catch (e) {
      failed(t("tracker.error.change"), e);
    }
  };

  // 1 to 9 pick a state while its menu is open.
  React.useEffect(() => {
    if (menu !== "state") return;
    const onKey = (e: KeyboardEvent) => {
      const n = Number(e.key);
      if (!Number.isInteger(n) || n < 1) return;
      const s = [...project.states].sort((a, b) => order(a.category) - order(b.category) || a.position - b.position)[n - 1];
      if (s !== undefined) {
        e.preventDefault();
        onMenu(null);
        void save({ state: s.id }, "tracker.done.state");
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  });

  const menuProps = (m: MenuName) => ({ open: menu === m, onOpenChange: (o: boolean) => onMenu(o ? m : null) });
  const link = `${window.location.origin}/git/issues/${issue.key}`;
  const mailHref = `/mail?${new URLSearchParams({ subject: `${issue.key} ${issue.title}`, body: `${issue.title}\n${link}\n` }).toString()}`;
  const chatHref = `/chat/?${new URLSearchParams({ text: `${issue.key} ${issue.title} ${link}` }).toString()}`;

  return (
    <div className="flex min-h-0 flex-1 flex-col" data-reader={issue.key}>
      <ReaderBar issueKey={`${project.name} · ${issue.key}`} onBack={onBack}>
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={() => {
            void navigator.clipboard?.writeText(link).then(() => toast.done(t("tracker.done.copied")));
          }}
          aria-label={t("tracker.reader.copyLink")}
          title={t("tracker.reader.copyLink")}
        >
          <HugeiconsIcon icon={Link01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </Button>
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={async () => {
            try {
              await tracker.follow(issue.key, !issue.subscribed);
              onSaved({ ...issue, subscribed: !issue.subscribed });
              toast.done(t(issue.subscribed ? "tracker.done.unfollowed" : "tracker.done.followed"));
            } catch (e) {
              failed(t("tracker.error.change"), e);
            }
          }}
          aria-pressed={issue.subscribed}
          aria-label={t(issue.subscribed ? "tracker.reader.unfollow" : "tracker.reader.follow")}
          title={t(issue.subscribed ? "tracker.reader.unfollow" : "tracker.reader.follow")}
          data-follow
        >
          <HugeiconsIcon icon={issue.subscribed ? Notification01Icon : NotificationOff01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </Button>
        <DropdownMenu>
          <DropdownMenuTrigger
            render={
              <Button variant="ghost" size="icon-sm" aria-label={t("tracker.reader.more")} title={t("tracker.reader.more")} data-more>
                <HugeiconsIcon icon={MoreHorizontalIcon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              </Button>
            }
          />
          <DropdownMenuContent align="end">
            {canHandTo("mail") && (
              <DropdownMenuItem render={<a href={mailHref} />} data-hand="mail">
                <HugeiconsIcon icon={Mail01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                {t("tracker.hand.mail")}
              </DropdownMenuItem>
            )}
            {canHandTo("chat") && (
              <DropdownMenuItem render={<a href={chatHref} />} data-hand="chat">
                <HugeiconsIcon icon={Message01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                {t("tracker.hand.chat")}
              </DropdownMenuItem>
            )}
            {issue.due !== null && (
              <DropdownMenuItem render={<a href={tracker.calendarUrl(issue.key)} download={`${issue.key}.ics`} />} data-hand="calendar">
                <HugeiconsIcon icon={Calendar03Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                {t("tracker.hand.calendar")}
              </DropdownMenuItem>
            )}
            {issue.assignee !== null && canHandTo("contacts") && (
              <DropdownMenuItem render={<a href={`/contacts?person=${encodeURIComponent(issue.assignee)}`} />} data-hand="contacts">
                <HugeiconsIcon icon={UserCircleIcon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                {t("tracker.hand.contact")}
              </DropdownMenuItem>
            )}
            <DropdownMenuItem
              onClick={() => {
                void navigator.clipboard?.writeText(`${issue.key} ${issue.title}`).then(() => toast.done(t("tracker.done.copied")));
              }}
            >
              <HugeiconsIcon icon={Copy01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              {t("tracker.reader.copyKey")}
            </DropdownMenuItem>
            {issue.permission === "admin" && (
              <>
                <DropdownMenuSeparator />
                <DropdownMenuItem
                  className="text-crit"
                  onClick={async () => {
                    if (!window.confirm(t("tracker.reader.deleteAsk", { key: issue.key }))) return;
                    try {
                      await tracker.remove(issue.key);
                      onDeleted();
                    } catch (e) {
                      failed(t("tracker.error.change"), e);
                    }
                  }}
                >
                  <HugeiconsIcon icon={Delete02Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                  {t("tracker.reader.delete")}
                </DropdownMenuItem>
              </>
            )}
          </DropdownMenuContent>
        </DropdownMenu>
      </ReaderBar>

      <div className="min-h-0 flex-1 overflow-y-auto">
        <div className="flex flex-col gap-5 px-5 py-4">
          {issue.parent !== undefined && (
            <button type="button" onClick={() => issue.parent !== undefined && onOpen(issue.parent.key)} className="flex items-center gap-1.5 self-start text-[12.5px] text-muted hover:text-ink">
              <StateIcon category={issue.parent.state.category} />
              <span className="font-mono">{issue.parent.key}</span>
              <span className="truncate">{issue.parent.title}</span>
            </button>
          )}
          <Title issue={issue} write={write} onSave={(title) => void save({ title })} />

          <dl className="grid grid-cols-[auto_1fr] items-center gap-x-4 gap-y-1.5 text-[13px]" data-properties>
            <Prop label={t("tracker.field.state")}>
              <StateMenu
                {...menuProps("state")}
                states={project.states}
                current={issue.state.id}
                onPick={(s) => void save({ state: s.id }, "tracker.done.state")}
                trigger={
                  <PropButton disabled={!write} data-prop="state">
                    <StateIcon category={issue.state.category} />
                    {issue.state.name}
                  </PropButton>
                }
              />
            </Prop>
            <Prop label={t("tracker.field.priority")}>
              <PriorityMenu
                {...menuProps("priority")}
                current={issue.priority as Priority}
                onPick={(p) => void save({ priority: p }, "tracker.done.priority")}
                trigger={
                  <PropButton disabled={!write} data-prop="priority">
                    <PriorityIcon priority={issue.priority as Priority} />
                    {t(PRIORITY_NAME[issue.priority as Priority])}
                  </PropButton>
                }
              />
            </Prop>
            <Prop label={t("tracker.field.assignee")}>
              <AssigneeMenu
                {...menuProps("assignee")}
                current={issue.assignee}
                onPick={(login) => void save({ assignee: login }, "tracker.done.assignee")}
                trigger={
                  <PropButton disabled={!write} data-prop="assignee">
                    {issue.assignee === null ? (
                      <span className="text-muted">{t("tracker.unassigned")}</span>
                    ) : (
                      <Person login={issue.assignee} name={nameOf(issue.assignee)} />
                    )}
                  </PropButton>
                }
              />
            </Prop>
            <Prop label={t("tracker.field.labels")}>
              <LabelMenu
                {...menuProps("labels")}
                labels={project.labels}
                current={issue.labels.map((l) => l.name)}
                onToggle={(name, on) => void save(on ? { addLabels: [name] } : { removeLabels: [name] })}
                trigger={
                  <PropButton disabled={!write} data-prop="labels" className="flex-wrap">
                    {issue.labels.length === 0 ? <span className="text-muted">{t("tracker.labels.add")}</span> : issue.labels.map((l) => <LabelChip key={l.id} label={l} />)}
                  </PropButton>
                }
              />
            </Prop>
            {project.cycles.length > 0 && (
              <Prop label={t("tracker.field.cycle")}>
                <CycleMenu
                  cycles={project.cycles}
                  current={issue.cycleId}
                  today={today}
                  onPick={(id) => void save({ cycle: id })}
                  trigger={
                    <PropButton disabled={!write}>
                      {(() => {
                        const c = project.cycles.find((x) => x.id === issue.cycleId);
                        return c === undefined ? <span className="text-muted">{t("tracker.cycle.none")}</span> : cycleName(c, t);
                      })()}
                    </PropButton>
                  }
                />
              </Prop>
            )}
            <Prop label={t("tracker.field.due")}>
              <span className="flex items-center gap-2">
                <Input
                  type="date"
                  value={issue.due ?? ""}
                  disabled={!write}
                  onChange={(e) => void save({ due: e.target.value === "" ? null : e.target.value })}
                  className="-mx-1.5 h-7 w-36 border-transparent bg-transparent px-1.5 text-[13px] shadow-none hover:border-line"
                  aria-label={t("tracker.field.due")}
                  data-prop="due"
                />
                {issue.due !== null && <DueDate due={issue.due} today={today} open={!closed(issue.state.category)} />}
              </span>
            </Prop>
            <Prop label={t("tracker.field.estimate")}>
              <EstimateMenu
                current={issue.estimate}
                onPick={(n) => void save({ estimate: n })}
                trigger={
                  <PropButton disabled={!write} data-prop="estimate" className="tabular-nums">
                    {issue.estimate === null ? <span className="text-muted">{t("tracker.estimate.none")}</span> : t("tracker.estimate.points", { count: issue.estimate })}
                  </PropButton>
                }
              />
            </Prop>
            {issue.repo !== null && (
              <Prop label={t("tracker.field.repo")}>
                <Link to={`/${issue.repo.owner}/${issue.repo.name}`} className="font-mono text-[12.5px] text-muted hover:text-ink">
                  {issue.repo.owner}/{issue.repo.name}#{issue.repo.number}
                </Link>
              </Prop>
            )}
          </dl>

          <Description issue={issue} write={write} keys={keys} onSave={(body) => save({ body })} />

          {ai && <AiHelp issue={issue} write={write} onMade={reload} />}

          <SubIssues issue={issue} write={write} onOpen={onOpen} onCreate={() => onCreateChild(issue.key, project.key)} onMade={reload} />

          <Relations issue={issue} write={write} onOpen={onOpen} onChanged={reload} />

          {issue.links.length > 0 && <Links issue={issue} />}

          <Timeline issue={issue} keys={keys} onChanged={reload} />
        </div>
      </div>
      <Composer issue={issue} onSent={reload} />
    </div>
  );
}

function order(c: string): number {
  return ["backlog", "unstarted", "started", "done", "canceled"].indexOf(c);
}

function Prop({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <>
      <dt className="text-muted">{label}</dt>
      <dd className="min-w-0">{children}</dd>
    </>
  );
}

function PropButton({ className, children, ...props }: React.ComponentProps<"button">) {
  return (
    <button
      type="button"
      className={cn(
        "-mx-1.5 inline-flex min-h-7 max-w-full items-center gap-1.5 rounded-control px-1.5 py-0.5 text-left text-ink",
        "transition-[background-color,scale] duration-150 hover:bg-sunk active:scale-[0.97] disabled:pointer-events-none",
        "outline-none focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent",
        "data-popup-open:bg-sunk",
        className,
      )}
      {...props}
    >
      {children}
    </button>
  );
}

function Title({ issue, write, onSave }: { issue: IssueDetail; write: boolean; onSave: (title: string) => void }) {
  const t = useT();
  const [editing, setEditing] = React.useState(false);
  const [text, setText] = React.useState(issue.title);
  React.useEffect(() => setText(issue.title), [issue.title]);
  if (editing) {
    return (
      <Input
        autoFocus
        value={text}
        onChange={(e) => setText(e.target.value)}
        onBlur={() => {
          setEditing(false);
          if (text.trim() !== "" && text.trim() !== issue.title) onSave(text.trim());
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter") e.currentTarget.blur();
          if (e.key === "Escape") {
            setText(issue.title);
            setEditing(false);
          }
        }}
        className="h-auto py-1 text-[19px] font-semibold tracking-tight"
        aria-label={t("tracker.field.title")}
      />
    );
  }
  return (
    <h2
      className={cn("text-[19px] leading-tight font-semibold tracking-tight [overflow-wrap:anywhere]", write && "cursor-text rounded-control hover:bg-sunk")}
      onClick={() => write && setEditing(true)}
      data-title
    >
      {issue.title}
    </h2>
  );
}

/** A text box that offers people and files after an @. */
function MentionBox({
  value,
  onChange,
  onSubmit,
  onCancel,
  placeholder,
  label,
  rows = 3,
  autoFocus = false,
}: {
  value: string;
  onChange: (v: string) => void;
  onSubmit: () => void;
  onCancel?: () => void;
  placeholder?: string;
  label: string;
  rows?: number;
  autoFocus?: boolean;
}) {
  const area = React.useRef<HTMLTextAreaElement>(null);
  const mentions = useMentions(area, onChange);
  return (
    <>
      <Textarea
        ref={area}
        value={value}
        rows={rows}
        autoFocus={autoFocus}
        placeholder={placeholder}
        aria-label={label}
        aria-autocomplete="list"
        onChange={(e) => {
          onChange(e.target.value);
          mentions.sync();
        }}
        onSelect={mentions.sync}
        onKeyDown={(e) => {
          if (mentions.onKeyDown(e)) {
            e.preventDefault();
            return;
          }
          if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
            e.preventDefault();
            onSubmit();
          }
          if (e.key === "Escape" && onCancel !== undefined) onCancel();
        }}
      />
      {mentions.picker}
    </>
  );
}

function Description({ issue, write, keys, onSave }: { issue: IssueDetail; write: boolean; keys: ReadonlySet<string>; onSave: (body: string) => Promise<void> }) {
  const t = useT();
  const [editing, setEditing] = React.useState(false);
  const [text, setText] = React.useState(issue.body);
  const [busy, setBusy] = React.useState(false);
  const submit = async () => {
    setBusy(true);
    await onSave(text);
    setBusy(false);
    setEditing(false);
  };
  if (editing) {
    return (
      <div className="flex flex-col gap-2">
        <MentionBox value={text} onChange={setText} onSubmit={() => void submit()} onCancel={() => setEditing(false)} rows={8} autoFocus label={t("tracker.field.description")} />
        <div className="flex justify-end gap-2">
          <Button variant="secondary" size="sm" onClick={() => setEditing(false)}>
            {t("git.cancel")}
          </Button>
          <Button size="sm" onClick={() => void submit()} disabled={busy}>
            {t("tracker.save")}
          </Button>
        </div>
      </div>
    );
  }
  return (
    <div className="group relative" data-description>
      {issue.body.trim() === "" ? (
        write ? (
          <button type="button" onClick={() => setEditing(true)} className="text-[13.5px] text-faint hover:text-muted">
            {t("tracker.description.add")}
          </button>
        ) : null
      ) : (
        <Markdown source={issue.body} issueKeys={keys} />
      )}
      {write && issue.body.trim() !== "" && (
        <Button
          variant="ghost"
          size="icon-xs"
          onClick={() => {
            setText(issue.body);
            setEditing(true);
          }}
          className="absolute -top-1 right-0 opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
          aria-label={t("tracker.description.edit")}
          title={t("tracker.description.edit")}
        >
          <HugeiconsIcon icon={PencilEdit02Icon} size={14} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </Button>
      )}
    </div>
  );
}

/** The box's code model on this issue: steps to split it into, or what the
 *  talk so far comes to. It suggests; the person picks what to make. */
function AiHelp({ issue, write, onMade }: { issue: IssueDetail; write: boolean; onMade: () => void }) {
  const t = useT();
  const [busy, setBusy] = React.useState<"breakdown" | "summary" | null>(null);
  const [steps, setSteps] = React.useState<string[] | null>(null);
  const [picked, setPicked] = React.useState<ReadonlySet<number>>(new Set());
  const [summary, setSummary] = React.useState<string | null>(null);
  const ask = async (task: "breakdown" | "summary") => {
    setBusy(task);
    try {
      const answer = await tracker.ai(issue.key, task);
      if (task === "breakdown") {
        setSteps(answer.items);
        setPicked(new Set(answer.items.map((_, i) => i)));
      } else setSummary(answer.text);
    } catch (e) {
      failed(t("tracker.ai.failed"), e);
    }
    setBusy(null);
  };
  const make = async () => {
    if (steps === null) return;
    const chosen = steps.filter((_, i) => picked.has(i));
    try {
      for (const title of chosen) await tracker.create({ project: issue.project.key, title, parent: issue.key });
      toast.done(t("tracker.ai.made", { count: chosen.length }));
      setSteps(null);
      onMade();
    } catch (e) {
      failed(t("tracker.error.change"), e);
    }
  };
  return (
    <section className="flex flex-col gap-2" aria-label={t("tracker.ai.label")} data-ai>
      <div className="flex flex-wrap gap-2">
        {write && (
          <Button variant="secondary" size="sm" onClick={() => void ask("breakdown")} disabled={busy !== null}>
            {busy === "breakdown" ? <Spinner className="size-4" /> : <HugeiconsIcon icon={AiMagicIcon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />}
            {t("tracker.ai.breakdown")}
          </Button>
        )}
        {(issue.comments.length > 0 || issue.body.length > 400) && (
          <Button variant="secondary" size="sm" onClick={() => void ask("summary")} disabled={busy !== null}>
            {busy === "summary" ? <Spinner className="size-4" /> : <HugeiconsIcon icon={AiMagicIcon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />}
            {t("tracker.ai.summary")}
          </Button>
        )}
      </div>
      {summary !== null && (
        <p className="animate-rise rounded-card border border-line bg-sunk px-3 py-2 text-[13.5px]" data-ai-summary>
          {summary}
        </p>
      )}
      {steps !== null && (
        <div className="flex animate-rise flex-col gap-1.5 rounded-card border border-line bg-sunk p-3" data-ai-steps>
          {steps.length === 0 ? (
            <p className="text-[13px] text-muted">{t("tracker.ai.nothing")}</p>
          ) : (
            steps.map((s, i) => (
              <label key={i} className="flex items-start gap-2 text-[13.5px]">
                <input
                  type="checkbox"
                  className="mt-1 accent-[var(--color-accent)]"
                  checked={picked.has(i)}
                  onChange={() => setPicked((p) => (p.has(i) ? new Set([...p].filter((x) => x !== i)) : new Set([...p, i])))}
                />
                {s}
              </label>
            ))
          )}
          <div className="mt-1 flex justify-end gap-2">
            <Button variant="ghost" size="sm" onClick={() => setSteps(null)}>
              {t("git.cancel")}
            </Button>
            {steps.length > 0 && (
              <Button size="sm" onClick={() => void make()} disabled={picked.size === 0}>
                {t("tracker.ai.make", { count: picked.size })}
              </Button>
            )}
          </div>
        </div>
      )}
    </section>
  );
}

function SubIssues({ issue, write, onOpen, onCreate, onMade }: { issue: IssueDetail; write: boolean; onOpen: (k: string) => void; onCreate: () => void; onMade: () => void }) {
  const t = useT();
  const [title, setTitle] = React.useState("");
  const done = issue.children.filter((c) => closed(c.state.category)).length;
  if (issue.children.length === 0 && !write) return null;
  const add = async () => {
    if (title.trim() === "") return;
    try {
      await tracker.create({ project: issue.project.key, title: title.trim(), parent: issue.key });
      setTitle("");
      onMade();
    } catch (e) {
      failed(t("tracker.error.change"), e);
    }
  };
  return (
    <section className="flex flex-col gap-1.5" aria-label={t("tracker.children.title")} data-children>
      <div className="flex items-center gap-2">
        <h3 className="text-[13px] font-medium">{t("tracker.children.title")}</h3>
        {issue.children.length > 0 && (
          <>
            <span className="text-[12px] text-muted tabular-nums">
              {done}/{issue.children.length}
            </span>
            <span className="h-1 w-20 overflow-hidden rounded-full bg-sunk" aria-hidden="true">
              <span className="block h-full origin-left rounded-full bg-accent transition-transform duration-200" style={{ transform: `scaleX(${done / issue.children.length})` }} />
            </span>
          </>
        )}
        <span className="flex-1" />
        {write && (
          <Button variant="ghost" size="xs" onClick={onCreate}>
            {t("tracker.children.full")}
          </Button>
        )}
      </div>
      <ul className="flex flex-col">
        {issue.children.map((c) => (
          <li key={c.key}>
            <button type="button" onClick={() => onOpen(c.key)} className="flex w-full items-center gap-2 rounded-control px-1.5 py-1 text-left text-[13.5px] hover:bg-sunk">
              <StateIcon category={c.state.category} />
              <span className="font-mono text-[12px] text-muted">{c.key}</span>
              <span className={cn("min-w-0 flex-1 truncate", closed(c.state.category) && "text-muted")}>{c.title}</span>
              <Assignee login={c.assignee} />
            </button>
          </li>
        ))}
      </ul>
      {write && (
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void add();
          }}
        >
          <Input value={title} onChange={(e) => setTitle(e.target.value)} placeholder={t("tracker.children.add")} aria-label={t("tracker.children.add")} className="h-8 text-[13px]" />
        </form>
      )}
    </section>
  );
}

const RELATION_NAME: Record<RelationKind, MessageKey> = {
  blocks: "tracker.relation.blocks",
  blocked_by: "tracker.relation.blockedBy",
  relates: "tracker.relation.relates",
  duplicates: "tracker.relation.duplicates",
  duplicated_by: "tracker.relation.duplicatedBy",
};

function Relations({ issue, write, onOpen, onChanged }: { issue: IssueDetail; write: boolean; onOpen: (k: string) => void; onChanged: () => void }) {
  const t = useT();
  const [adding, setAdding] = React.useState(false);
  const [kind, setKind] = React.useState<RelationKind>("blocks");
  const [other, setOther] = React.useState("");
  if (issue.relations.length === 0 && !write) return null;
  const add = async () => {
    try {
      await tracker.relate(issue.key, kind, other.trim().toUpperCase());
      setOther("");
      setAdding(false);
      onChanged();
    } catch (e) {
      failed(t("tracker.error.change"), e);
    }
  };
  return (
    <section className="flex flex-col gap-1.5" aria-label={t("tracker.relation.title")} data-relations>
      <div className="flex items-center gap-2">
        <h3 className="text-[13px] font-medium">{t("tracker.relation.title")}</h3>
        <span className="flex-1" />
        {write && !adding && (
          <Button variant="ghost" size="xs" onClick={() => setAdding(true)}>
            {t("tracker.relation.add")}
          </Button>
        )}
      </div>
      <ul className="flex flex-col">
        {issue.relations.map((r) => (
          <li key={r.id} className="group flex items-center gap-2 rounded-control px-1.5 py-1 text-[13.5px] hover:bg-sunk">
            <span className={cn("w-28 shrink-0 text-[12px]", r.kind === "blocked_by" ? "text-warn" : "text-muted")}>{t(RELATION_NAME[r.kind])}</span>
            <button type="button" onClick={() => onOpen(r.issue.key)} className="flex min-w-0 flex-1 items-center gap-2 text-left">
              <StateIcon category={r.issue.state.category} />
              <span className="font-mono text-[12px] text-muted">{r.issue.key}</span>
              <span className="min-w-0 flex-1 truncate">{r.issue.title}</span>
            </button>
            {write && (
              <Button
                variant="ghost"
                size="icon-xs"
                className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
                onClick={async () => {
                  try {
                    await tracker.unrelate(r.id);
                    onChanged();
                  } catch (e) {
                    failed(t("tracker.error.change"), e);
                  }
                }}
                aria-label={t("tracker.relation.remove", { key: r.issue.key })}
                title={t("tracker.relation.remove", { key: r.issue.key })}
              >
                <HugeiconsIcon icon={Delete02Icon} size={13} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              </Button>
            )}
          </li>
        ))}
      </ul>
      {adding && (
        <form
          className="flex animate-rise flex-wrap items-center gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            void add();
          }}
        >
          <NativeSelect value={kind} onChange={(e) => setKind(e.target.value as RelationKind)} className="h-8 w-auto text-[13px]" aria-label={t("tracker.relation.kind")}>
            {(["blocks", "blocked_by", "relates", "duplicates"] as const).map((k) => (
              <NativeSelectOption key={k} value={k}>
                {t(RELATION_NAME[k])}
              </NativeSelectOption>
            ))}
          </NativeSelect>
          <Input value={other} onChange={(e) => setOther(e.target.value)} placeholder="LOS-12" autoFocus className="h-8 w-28 font-mono text-[13px]" aria-label={t("tracker.relation.other")} />
          <Button type="submit" size="sm" disabled={other.trim() === ""}>
            {t("tracker.relation.add")}
          </Button>
          <Button type="button" variant="ghost" size="sm" onClick={() => setAdding(false)}>
            {t("git.cancel")}
          </Button>
        </form>
      )}
    </section>
  );
}

function Links({ issue }: { issue: IssueDetail }) {
  const t = useT();
  return (
    <section className="flex flex-col gap-1.5" aria-label={t("tracker.links.title")} data-links>
      <h3 className="text-[13px] font-medium">{t("tracker.links.title")}</h3>
      <ul className="flex flex-col">
        {issue.links.map((l) => {
          const to = l.kind === "pull" ? `/${l.owner}/${l.repo}/pulls/${l.target}` : `/${l.owner}/${l.repo}/commits?sha=${l.target}`;
          return (
            <li key={l.id}>
              <Link to={to} className="flex items-center gap-2 rounded-control px-1.5 py-1 text-[13.5px] hover:bg-sunk" data-link-kind={l.kind}>
                <HugeiconsIcon icon={l.kind === "pull" ? GitPullRequestIcon : GitCommitIcon} size={14} strokeWidth={1.5} color="currentColor" className="shrink-0 text-muted" aria-hidden="true" />
                <span className="shrink-0 font-mono text-[12px] text-muted">{l.kind === "pull" ? `#${l.target}` : l.target.slice(0, 7)}</span>
                <span className="min-w-0 flex-1 truncate">{l.title}</span>
                {l.closes && <Badge variant="ok" className="py-0 text-[11px]">{t("tracker.links.closes")}</Badge>}
                <span className="shrink-0 text-[12px] text-muted">{when(l.created)}</span>
              </Link>
            </li>
          );
        })}
      </ul>
    </section>
  );
}

/** What was said and what changed, oldest first, as one thread. */
function Timeline({ issue, keys, onChanged }: { issue: IssueDetail; keys: ReadonlySet<string>; onChanged: () => void }) {
  const t = useT();
  const { home } = useTracker();
  const nameOf = useNameOf();
  const me = home?.me ?? null;
  type Entry = { at: string; id: string } & ({ comment: IssueDetail["comments"][number] } | { activity: IssueDetail["activity"][number] });
  const entries: Entry[] = [
    ...issue.comments.map((c) => ({ at: c.created, id: `c${c.id}`, comment: c })),
    ...issue.activity.filter((a) => a.kind !== "description").map((a) => ({ at: a.created, id: `a${a.id}`, activity: a })),
  ].sort((a, b) => a.at.localeCompare(b.at) || a.id.localeCompare(b.id));
  return (
    <section className="flex flex-col gap-2 border-t border-hair pt-4" aria-label={t("tracker.activity.heading")} data-timeline>
      <h3 className="text-[13px] font-medium">{t("tracker.activity.heading")}</h3>
      <ol className="flex flex-col gap-2">
        {entries.map((e) =>
          "comment" in e ? (
            <li key={e.id} className="flex animate-rise flex-col gap-1.5 rounded-card border border-line bg-surface px-3 py-2.5" data-comment>
              <div className="flex items-center gap-2 text-[12.5px]">
                <Person login={e.comment.author} name={nameOf(e.comment.author)} />
                <span className="text-muted" title={whenFull(e.comment.created)}>
                  {when(e.comment.created)}
                </span>
                <span className="flex-1" />
                {e.comment.author === me && (
                  <Button
                    variant="ghost"
                    size="icon-xs"
                    onClick={async () => {
                      try {
                        await tracker.removeComment(e.comment.id);
                        onChanged();
                      } catch (err) {
                        failed(t("tracker.error.change"), err);
                      }
                    }}
                    aria-label={t("tracker.comment.delete")}
                    title={t("tracker.comment.delete")}
                  >
                    <HugeiconsIcon icon={Delete02Icon} size={13} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                  </Button>
                )}
              </div>
              <Markdown source={e.comment.body} issueKeys={keys} className="text-[13.5px]" />
            </li>
          ) : (
            <li key={e.id} className="flex items-center gap-2 px-1 text-[12.5px] text-muted" data-activity={e.activity.kind}>
              <Assignee login={e.activity.actor} />
              <span className="min-w-0 flex-1">
                <span className="font-medium text-ink">{nameOf(e.activity.actor)}</span>{" "}
                {describe(e.activity, t, nameOf)}
              </span>
              <span className="shrink-0" title={whenFull(e.activity.created)}>
                {when(e.activity.created)}
              </span>
            </li>
          ),
        )}
      </ol>
    </section>
  );
}

function describe(a: IssueDetail["activity"][number], t: ReturnType<typeof useT>, nameOf: (login: string) => string): string {
  const d = a.detail as Record<string, string | number | null | undefined>;
  const s = (v: unknown) => (v === null || v === undefined || v === "" ? t("tracker.activity.nothing") : String(v));
  switch (a.kind) {
    case "created":
      return t("tracker.activity.created");
    case "title":
      return t("tracker.activity.renamed", { to: s(d.to) });
    case "state":
      return t("tracker.activity.state", { to: s(d.to) });
    case "priority":
      return t("tracker.activity.priority", { to: t(PRIORITY_NAME[priorityNamed(String(d.to))]) });
    case "assignee":
      return d.to === null || d.to === undefined ? t("tracker.activity.unassigned") : t("tracker.activity.assigned", { to: nameOf(String(d.to)) });
    case "estimate":
      return t("tracker.activity.estimate", { to: s(d.to) });
    case "cycle":
      return t("tracker.activity.cycle", { to: s(d.to) });
    case "parent":
      return d.to === null || d.to === undefined ? t("tracker.activity.noParent") : t("tracker.activity.parent", { to: s(d.to) });
    case "due":
      return d.to === null || d.to === undefined ? t("tracker.activity.noDue") : t("tracker.activity.due", { to: s(d.to) });
    case "label_added":
      return t("tracker.activity.labelAdded", { label: s(d.label) });
    case "label_removed":
      return t("tracker.activity.labelRemoved", { label: s(d.label) });
    case "relation":
      return t("tracker.activity.relation", { kind: t(RELATION_NAME[(d.kind as RelationKind) ?? "relates"] ?? "tracker.relation.relates"), other: s(d.other) });
    case "link":
      return d.kind === "pull" ? t("tracker.activity.pull", { what: `${s(d.repo)}#${s(d.target)}` }) : t("tracker.activity.commit", { what: String(d.target ?? "").slice(0, 7) });
    case "moved":
      return t("tracker.activity.moved", { from: s(d.from), to: s(d.to) });
    default:
      return a.kind;
  }
}

/** The activity names a priority by word, as the server's PRIORITIES do. */
function priorityNamed(word: string): Priority {
  const i = ["none", "urgent", "high", "medium", "low"].indexOf(word);
  return (i < 0 ? 0 : i) as Priority;
}

function Composer({ issue, onSent }: { issue: IssueDetail; onSent: () => void }) {
  const t = useT();
  const [text, setText] = React.useState("");
  const [busy, setBusy] = React.useState(false);
  const send = async () => {
    if (text.trim() === "") return;
    setBusy(true);
    try {
      await tracker.comment(issue.key, text);
      setText("");
      onSent();
    } catch (e) {
      failed(t("tracker.error.change"), e);
    }
    setBusy(false);
  };
  return (
    <form
      className="flex flex-col gap-2 border-t border-line bg-surface px-4 py-3"
      onSubmit={(e) => {
        e.preventDefault();
        void send();
      }}
      data-composer
    >
      <MentionBox value={text} onChange={setText} onSubmit={() => void send()} placeholder={t("tracker.comment.placeholder")} label={t("tracker.comment.label")} rows={2} />
      <div className="flex items-center justify-between gap-2">
        <span className="text-[12px] text-faint">{t("tracker.comment.hint")}</span>
        <Button type="submit" size="sm" disabled={busy || text.trim() === ""}>
          {t("tracker.comment.send")}
        </Button>
      </div>
    </form>
  );
}
