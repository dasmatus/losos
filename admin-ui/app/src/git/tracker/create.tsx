import * as React from "react";
import { useMentions } from "@/components/mention-picker";
import { Button } from "@/components/ui/button";
import { Dialog, DialogBody, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Textarea } from "@/components/ui/textarea";
import { toast } from "@/components/ui/toast";
import { useT } from "@/lib/i18n-react";
import { failed } from "../parts";
import { tracker, type IssueDetail, type Priority } from "./api";
import { Assignee, AssigneeMenu, LabelChip, LabelMenu, PRIORITY_NAME, PriorityIcon, PriorityMenu, StateIcon, StateMenu } from "./parts";
import { useTracker } from "./store";

/* A new issue: a title, a description if there is more to say, and the
 * few things one usually knows at once, each a button with its menu.
 * Ctrl+Enter (Cmd+Enter) makes it; "Make another" keeps the dialog open
 * for the next one, as when writing down a list. */

export interface Draft {
  project: string;
  state?: number;
  parent?: string;
}

export function CreateIssue({ draft, onClose, onCreated }: { draft: Draft; onClose: () => void; onCreated: (issue: IssueDetail, more: boolean) => void }) {
  const t = useT();
  const id = React.useId();
  const { home } = useTracker();
  const projects = (home?.projects ?? []).filter((p) => !p.archived && (p.permission === "write" || p.permission === "admin"));
  const [projectKey, setProjectKey] = React.useState(draft.project !== "" ? draft.project : (projects[0]?.key ?? ""));
  const project = projects.find((p) => p.key === projectKey) ?? projects[0];
  const [title, setTitle] = React.useState("");
  const [body, setBody] = React.useState("");
  const [state, setState] = React.useState<number | undefined>(draft.state);
  const [priority, setPriority] = React.useState<Priority>(0);
  const [assignee, setAssignee] = React.useState<string | null>(null);
  const [labels, setLabels] = React.useState<string[]>([]);
  const [due, setDue] = React.useState("");
  const [more, setMore] = React.useState(false);
  const [busy, setBusy] = React.useState(false);
  const area = React.useRef<HTMLTextAreaElement>(null);
  const mentions = useMentions(area, setBody);
  const titleRef = React.useRef<HTMLInputElement>(null);
  // showModal() moves focus to the first control (the project) after this
  // mounts, so the title takes it back a frame later.
  React.useEffect(() => {
    const frame = requestAnimationFrame(() => titleRef.current?.focus());
    return () => cancelAnimationFrame(frame);
  }, []);

  const chosenState = project?.states.find((s) => s.id === state) ?? project?.states.find((s) => s.category === "unstarted") ?? project?.states[0];

  const submit = async () => {
    if (project === undefined || title.trim() === "") return;
    setBusy(true);
    try {
      const issue = await tracker.create({
        project: project.key,
        title: title.trim(),
        body,
        state: chosenState?.id,
        priority,
        assignee,
        labels,
        due: due === "" ? null : due,
        parent: draft.parent ?? null,
      });
      toast.done(t("tracker.done.created", { key: issue.key }));
      onCreated(issue, more);
      if (more) {
        setTitle("");
        setBody("");
        titleRef.current?.focus();
      }
    } catch (e) {
      failed(t("tracker.error.create"), e);
    }
    setBusy(false);
  };

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()} labelledBy={`${id}-title`} dialogClassName="w-[min(40rem,calc(100vw-2rem))]">
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
            e.preventDefault();
            void submit();
          }
        }}
        data-create-issue
      >
        <DialogHeader className="flex-row items-center gap-2">
          <NativeSelect value={project?.key ?? ""} onChange={(e) => setProjectKey(e.target.value)} className="h-7 w-auto text-[12.5px]" aria-label={t("tracker.field.project")} disabled={draft.parent !== undefined}>
            {projects.map((p) => (
              <NativeSelectOption key={p.key} value={p.key}>
                {p.key} · {p.name}
              </NativeSelectOption>
            ))}
          </NativeSelect>
          <DialogTitle id={`${id}-title`} className="text-[15px]">
            {draft.parent !== undefined ? t("tracker.create.child", { key: draft.parent }) : t("tracker.create.title")}
          </DialogTitle>
        </DialogHeader>
        <DialogBody className="flex flex-col gap-3">
          <Input
            ref={titleRef}
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder={t("tracker.create.titlePlaceholder")}
            aria-label={t("tracker.field.title")}
            autoFocus
            maxLength={255}
            className="h-10 border-0 px-0 text-[17px] font-medium shadow-none focus-visible:ring-0"
            data-create-title
          />
          <Textarea
            ref={area}
            value={body}
            onChange={(e) => {
              setBody(e.target.value);
              mentions.sync();
            }}
            onSelect={mentions.sync}
            onKeyDown={(e) => {
              if (mentions.onKeyDown(e)) e.preventDefault();
            }}
            rows={5}
            placeholder={t("tracker.create.bodyPlaceholder")}
            aria-label={t("tracker.field.description")}
            aria-autocomplete="list"
            className="border-0 px-0 shadow-none focus-visible:ring-0"
          />
          {mentions.picker}
          {project !== undefined && (
            <div className="flex flex-wrap items-center gap-1.5">
              <StateMenu
                states={project.states}
                current={chosenState?.id}
                onPick={(s) => setState(s.id)}
                trigger={
                  <Button type="button" variant="secondary" size="xs">
                    {chosenState !== undefined && <StateIcon category={chosenState.category} />}
                    {chosenState?.name}
                  </Button>
                }
              />
              <PriorityMenu
                current={priority}
                onPick={setPriority}
                trigger={
                  <Button type="button" variant="secondary" size="xs">
                    <PriorityIcon priority={priority} />
                    {t(PRIORITY_NAME[priority])}
                  </Button>
                }
              />
              <AssigneeMenu
                current={assignee}
                onPick={setAssignee}
                trigger={
                  <Button type="button" variant="secondary" size="xs">
                    <Assignee login={assignee} className="size-4 text-[8px]" />
                    {assignee === null ? t("tracker.unassigned") : home?.people.find((p) => p.login === assignee)?.fullName || assignee}
                  </Button>
                }
              />
              <LabelMenu
                labels={project.labels}
                current={labels}
                onToggle={(name, on) => setLabels((l) => (on ? [...l, name] : l.filter((x) => x.toLowerCase() !== name.toLowerCase())))}
                trigger={
                  <Button type="button" variant="secondary" size="xs">
                    {labels.length === 0 ? t("tracker.field.labels") : labels.map((l) => <LabelChip key={l} label={{ id: 0, name: l, color: project.labels.find((x) => x.name === l)?.color ?? "", description: "" }} />)}
                  </Button>
                }
              />
              <Input type="date" value={due} onChange={(e) => setDue(e.target.value)} className="h-6 w-auto px-2 text-[12px]" aria-label={t("tracker.field.due")} />
            </div>
          )}
        </DialogBody>
        <DialogFooter className="items-center">
          <label className="mr-auto flex items-center gap-2 text-[13px] text-muted">
            <input type="checkbox" checked={more} onChange={(e) => setMore(e.target.checked)} className="accent-[var(--color-accent)]" />
            {t("tracker.create.more")}
          </label>
          <Button type="button" variant="secondary" onClick={onClose}>
            {t("git.cancel")}
          </Button>
          <Button type="submit" disabled={busy || title.trim() === "" || project === undefined}>
            {t("tracker.create.submit")}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}
