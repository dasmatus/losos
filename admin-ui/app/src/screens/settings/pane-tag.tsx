import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Cancel01Icon } from "@hugeicons/core-free-icons";
import {
  ApiError,
  deleteTagSkill,
  type TagAssistant,
  type TagSkill,
  type TagView,
  getTag,
  putTagArea,
  putTagAssistant,
  putTagClearance,
  putTagLevel,
  putTagSkill,
} from "@/lib/api";
import { useT } from "@/lib/i18n-react";
import { Badge } from "@/components/ui/badge";
import { toast } from "@/components/ui/toast";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/progress";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText, RowValue } from "./rows";

/* LosOS Tag (backend/src/tag.rs): the assistants people tag in LosOS
 * Chat, and box they are allowed to read — the levels, the areas that
 * grade the box's data with them, and the skills an ask can use.
 *
 * Everyone's own is on one pane: a person shapes their own assistants and
 * skills, an administrator adds the levels and grades anyone's. Editing,
 * every list: a row adds, a form saves, nothing applies at once — each
 * request is its own, so nothing here shares the one form draft.
 */

import { canUse } from "@/lib/api";

type Loading = { kind: "loading" } | { kind: "ready"; view: TagView } | { kind: "failed"; message: string };

/** The kinds an area grades, in the same words the backend carries. */
const KINDS = ["cloud", "git", "chat", "mail", "logs"] as const;

export function TagPane({ locked }: { locked: boolean }) {
  const t = useT();
  const [state, setState] = React.useState<Loading>({ kind: "loading" });
  const [reload, setReload] = React.useState(0);

  React.useEffect(() => {
    let live = true;
    setState({ kind: "loading" });
    getTag()
      .then((view) => {
        if (live) setState({ kind: "ready", view });
      })
      .catch((error: unknown) => {
        if (live) {
          setState({
            kind: "failed",
            message: error instanceof ApiError ? error.message : String(error),
          });
        }
      });
    return () => {
      live = false;
    };
  }, [reload]);

  if (state.kind === "loading" || state.kind === "failed") {
    return (
      <PaneSection>
        <Group>
          <Row last>
            <RowText title={state.kind === "loading" ? t("tag.loading") : t("tag.failed")} detail={state.kind === "failed" ? state.message : undefined} />
            {state.kind === "failed" && (
              <Button size="sm" variant="secondary" onClick={() => setReload((n) => n + 1)}>
                {t("tag.retry")}
              </Button>
            )}
            {state.kind === "loading" && <Spinner size={16} />}
          </Row>
        </Group>
      </PaneSection>
    );
  }
  return (
    <TagEditor view={state.view} onSaved={() => setReload((n) => n + 1)} locked={locked} />
  );
}

interface TagEditorProps {
  view: TagView;
  onSaved: () => void;
  locked: boolean;
}

function TagEditor({ view, onSaved, locked }: TagEditorProps) {
  const busy = React.useRef(false);
  const save = async (
    request: () => Promise<{ ok: true }>,
    then: "clear" | "reload",
    form: React.RefObject<HTMLFormElement | null>,
  ): Promise<void> => {
    if (busy.current) return;
    busy.current = true;
    try {
      await request();
      if (then === "clear") form.current?.reset();
      onSaved();
    } catch (error) {
      toast.status(error instanceof ApiError ? error.message : String(error));
    } finally {
      busy.current = false;
    }
  };
  return (
    <PaneSection>
      <ClearanceSection view={view} locked={locked} save={save} />
      <LevelsSection view={view} locked={locked} save={save} />
      <AreasSection view={view} locked={locked} save={save} />
      <AssistantsSection view={view} locked={locked} save={save} />
      <SkillsSection view={view} locked={locked} save={save} />
      <AsksSection view={view} />
    </PaneSection>
  );
}

type Request = () => Promise<{ ok: true }>;
type Save = (request: Request, then: "clear" | "reload", form: React.RefObject<HTMLFormElement | null>) => Promise<void>;

/** A person's own clearance: what their companion reads at. */
function ClearanceSection({ view, locked, save }: { view: TagView; locked: boolean; save: Save }) {
  const t = useT();
  const [level, setLevel] = React.useState(view.clearance);
  return (
    <Group>
      <GroupTitle>{t("tag.clearance.title")}</GroupTitle>
      <GroupCaption>{t("tag.clearance.summary")}</GroupCaption>
      <Row last>
        <RowText title={t("tag.clearance.row")} />
        <RowValue>
          <select
            aria-label={t("tag.clearance.row")}
            className="bg-surface text-ink rounded-lg border border-line px-2 py-1.5"
            value={level}
            disabled={locked}
            onChange={(event) => {
              const next = event.target.value;
              setLevel(next);
              void save(() => putTagClearance(next), "reload", React.createRef());
            }}
          >
            {view.levels.map((one) => (
              <option key={one.name} value={one.name}>
                {one.name}
              </option>
            ))}
          </select>
        </RowValue>
      </Row>
    </Group>
  );
}

/** The box's levels: ranked, most public first. An administrator adds
 *  them; they are the box's words, not one person's. */
function LevelsSection({ view, locked, save }: { view: TagView; locked: boolean; save: Save }) {
  const t = useT();
  const form = React.useRef<HTMLFormElement>(null);
  const [name, setName] = React.useState("");
  const [description, setDescription] = React.useState("");
  const addable = name.trim() !== "" && !locked;
  return (
    <Group>
      <GroupTitle>{t("tag.levels.title")}</GroupTitle>
      <GroupCaption>{t("tag.levels.summary")}</GroupCaption>
      <ul>
        {view.levels.map((level, index) => (
          <Row key={level.name} last={index === view.levels.length - 1}>
            <RowText title={level.name} detail={level.description ?? undefined} />
            <RowValue>
              <Badge variant="outline">
                {t("tag.levels.rank", { count: index })}
              </Badge>
            </RowValue>
          </Row>
        ))}
      </ul>
      <form
        className="flex flex-wrap items-center gap-2"
        ref={form}
        onSubmit={(event) => {
          event.preventDefault();
          void save(() => putTagLevel({ name: name.trim(), description: description.trim() }), "clear", form);
        }}
      >
        <Input aria-label={t("tag.levels.name")} value={name} onChange={(event) => setName(event.target.value)} placeholder={t("tag.levels.name")} className="w-40" disabled={locked} />
        <Input aria-label={t("tag.levels.description")} value={description} onChange={(event) => setDescription(event.target.value)} placeholder={t("tag.levels.description")} className="min-w-0 flex-1" disabled={locked} />
        <Button size="sm" type="submit" disabled={!addable}>
          {t("tag.add")}
        </Button>
      </form>
    </Group>
  );
}

/** The areas: one piece of the box's data, one level. */
function AreasSection({ view, locked, save }: { view: TagView; locked: boolean; save: Save }) {
  const t = useT();
  const form = React.useRef<HTMLFormElement>(null);
  const [kind, setKind] = React.useState<(typeof KINDS)[number]>("cloud");
  const [reference, setReference] = React.useState("");
  const [level, setLevel] = React.useState(view.levels[0]?.name ?? "");
  const [owner, setOwner] = React.useState("");
  React.useEffect(() => {
    if (level === "" && view.levels[0] !== undefined) setLevel(view.levels[0].name);
  }, [view.levels, level]);
  const addable = reference.trim() !== "" && level !== "" && !locked;
  return (
    <Group>
      <GroupTitle>{t("tag.areas.title")}</GroupTitle>
      <GroupCaption>{t("tag.areas.summary")}</GroupCaption>
      <ul>
        {view.areas.length === 0 && (
          <Row last>
            <RowText title={t("tag.areas.empty")} />
          </Row>
        )}
        {view.areas.map((area, index) => (
          <Row key={`${area.kind} ${area.ref}`} last={index === view.areas.length - 1}>
            <RowText title={`${area.kind}: ${area.ref}`} detail={`${area.owner} — ${area.level}`} />
          </Row>
        ))}
      </ul>
      <form
        className="flex flex-wrap items-center gap-2"
        ref={form}
        onSubmit={(event) => {
          event.preventDefault();
          void save(() => putTagArea({ kind, ref: reference.trim(), level, owner: owner.trim() || "everyone" }), "reload", form);
        }}
      >
        <select aria-label={t("tag.areas.kind")} className="bg-surface text-ink rounded-lg border border-line px-2 py-1.5" value={kind} onChange={(event) => setKind(event.target.value as (typeof KINDS)[number])} disabled={locked}>
          {KINDS.map((one) => (
            <option key={one} value={one}>
              {one}
            </option>
          ))}
        </select>
        <Input aria-label={t("tag.areas.ref")} value={reference} onChange={(event) => setReference(event.target.value)} placeholder={t("tag.areas.ref")} className="min-w-0 flex-1" disabled={locked} />
        <Input aria-label={t("tag.areas.owner")} value={owner} onChange={(event) => setOwner(event.target.value)} placeholder={t("tag.areas.owner")} className="w-36" disabled={locked} />
        <select aria-label={t("tag.areas.level")} className="bg-surface text-ink rounded-lg border border-line px-2 py-1.5" value={level} onChange={(event) => setLevel(event.target.value)} disabled={locked}>
          {view.levels.map((one) => (
            <option key={one.name} value={one.name}>
              {one.name}
            </option>
          ))}
        </select>
        <Button size="sm" type="submit" disabled={!addable}>
          {t("tag.add")}
        </Button>
      </form>
    </Group>
  );
}

/** The assistants: the built-in one, and anyone's own beside it. */
function AssistantsSection({ view, locked, save }: { view: TagView; locked: boolean; save: Save }) {
  const t = useT();
  const form = React.useRef<HTMLFormElement>(null);
  const [slug, setSlug] = React.useState("");
  const [display, setDisplay] = React.useState("");
  const [prompt, setPrompt] = React.useState("");
  const [clearance, setClearance] = React.useState(view.levels[0]?.name ?? "");
  React.useEffect(() => {
    if (clearance === "" && view.levels[0] !== undefined) setClearance(view.levels[0].name);
  }, [view.levels, clearance]);
  const addable = slug.trim() !== "" && display.trim() !== "" && clearance !== "" && !locked;
  return (
    <Group>
      <GroupTitle>{t("tag.assistants.title")}</GroupTitle>
      <GroupCaption>{t("tag.assistants.summary")}</GroupCaption>
      <ul>
        {view.assistants.map((assistant, index) => (
          <AssistantRow key={assistant.slug} assistant={assistant} last={index === view.assistants.length - 1} save={save} />
        ))}
      </ul>
      <form
        className="flex flex-col gap-2"
        ref={form}
        onSubmit={(event) => {
          event.preventDefault();
          void save(
            () => putTagAssistant({ slug: slug.trim(), display: display.trim(), prompt: prompt.trim(), clearance }),
            "clear",
            form,
          );
        }}
      >
        <div className="flex flex-wrap items-center gap-2">
          <Input aria-label={t("tag.assistants.slug")} value={slug} onChange={(event) => setSlug(event.target.value)} placeholder={t("tag.assistants.slug")} className="w-40" disabled={locked} />
          <Input aria-label={t("tag.assistants.display")} value={display} onChange={(event) => setDisplay(event.target.value)} placeholder={t("tag.assistants.display")} className="w-48" disabled={locked} />
          <select aria-label={t("tag.assistants.clearance")} className="bg-surface text-ink rounded-lg border border-line px-2 py-1.5" value={clearance} onChange={(event) => setClearance(event.target.value)} disabled={locked}>
            {view.levels.map((one) => (
              <option key={one.name} value={one.name}>
                {one.name}
              </option>
            ))}
          </select>
          <Button size="sm" type="submit" disabled={!addable}>
            {t("tag.add")}
          </Button>
        </div>
        <Input aria-label={t("tag.assistants.prompt")} value={prompt} onChange={(event) => setPrompt(event.target.value)} placeholder={t("tag.assistants.prompt")} className="min-w-0 flex-1" disabled={locked} />
      </form>
    </Group>
  );
}

function AssistantRow({ assistant, last, save }: { assistant: TagAssistant; last: boolean; save: Save }) {
  const t = useT();
  const editable = canUse("admin") || assistant.owner !== "everyone";
  return (
    <Row last={last}>
      <RowText title={assistant.display} detail={`@${assistant.slug} — ${assistant.clearance}${assistant.prompt !== "" ? ` — ${assistant.prompt}` : ""}`} />
      <RowValue>
        <Badge variant={assistant.enabled ? "outline" : "secondary"}>{t(assistant.enabled ? "tag.assistants.on" : "tag.assistants.off")}</Badge>
        {assistant.enabled ? (
          <Button
            size="sm"
            variant="ghost"
            disabled={assistant.owner === "everyone" && !editable}
            onClick={() => void save(() => putTagAssistant({ ...assistant, enabled: false }), "reload", React.createRef())}
          >
            <HugeiconsIcon icon={Cancel01Icon} className="size-4" aria-hidden="true" />
            {t("tag.assistants.disable")}
          </Button>
        ) : null}
      </RowValue>
    </Row>
  );
}

/** The skills: the seeded LosOS ones, then anyone's own. */
function SkillsSection({ view, locked, save }: { view: TagView; locked: boolean; save: Save }) {
  const t = useT();
  const form = React.useRef<HTMLFormElement>(null);
  const [slug, setSlug] = React.useState("");
  const [display, setDisplay] = React.useState("");
  const [prompt, setPrompt] = React.useState("");
  const addable = slug.trim() !== "" && display.trim() !== "" && prompt.trim() !== "" && !locked;
  return (
    <Group>
      <GroupTitle>{t("tag.skills.title")}</GroupTitle>
      <GroupCaption>{t("tag.skills.summary")}</GroupCaption>
      <ul>
        {view.skills.map((skill, index) => (
          <SkillRow key={`${skill.owner}/${skill.slug}`} skill={skill} last={index === view.skills.length - 1} save={save} />
        ))}
      </ul>
      <form
        className="flex flex-col gap-2"
        ref={form}
        onSubmit={(event) => {
          event.preventDefault();
          void save(() => putTagSkill({ slug: slug.trim(), display: display.trim(), prompt: prompt.trim() }), "clear", form);
        }}
      >
        <div className="flex flex-wrap items-center gap-2">
          <Input aria-label={t("tag.skills.slug")} value={slug} onChange={(event) => setSlug(event.target.value)} placeholder={t("tag.skills.slug")} className="w-40" disabled={locked} />
          <Input aria-label={t("tag.skills.display")} value={display} onChange={(event) => setDisplay(event.target.value)} placeholder={t("tag.skills.display")} className="w-56" disabled={locked} />
          <Button size="sm" type="submit" disabled={!addable}>
            {t("tag.add")}
          </Button>
        </div>
        <Input aria-label={t("tag.skills.prompt")} value={prompt} onChange={(event) => setPrompt(event.target.value)} placeholder={t("tag.skills.prompt")} className="min-w-0 flex-1" disabled={locked} />
      </form>
    </Group>
  );
}

function SkillRow({ skill, last, save }: { skill: TagSkill; last: boolean; save: Save }) {
  const t = useT();
  return (
    <Row last={last}>
      <RowText title={skill.display} detail={skill.prompt} />
      <RowValue>
        <Badge variant={skill.builtin ? "outline" : "secondary"}>{t(skill.builtin ? "tag.skills.builtin" : "tag.skills.own")}</Badge>
        <Button size="sm" variant="ghost" onClick={() => void save(() => deleteTagSkill(skill.slug), "reload", React.createRef())}>
          <HugeiconsIcon icon={Cancel01Icon} className="size-4" aria-hidden="true" />
          {t("tag.delete")}
        </Button>
      </RowValue>
    </Row>
  );
}

/** The asks and the conversations that came out of them. */
function AsksSection({ view }: { view: TagView }) {
  const t = useT();
  const rows = [
    ...view.jobs.map((job) => ({ id: job.id, kind: t("tag.jobs.row"), person: job.person, assistant: job.assistant, question: job.question, answer: job.answer, state: job.state })),
    ...view.conversations.map((one) => ({ id: one.id, kind: t("tag.asks.done"), person: one.person, assistant: one.assistant, question: one.question, answer: one.answer, state: "done" })),
  ];
  return (
    <Group>
      <GroupTitle>{t("tag.asks.title")}</GroupTitle>
      <GroupCaption>{t("tag.asks.summary")}</GroupCaption>
      <ul>
        {rows.length === 0 && (
          <Row last>
            <RowText title={t("tag.asks.empty")} />
          </Row>
        )}
        {rows.map((row, index) => (
          <Row key={row.id} last={index === rows.length - 1}>
            <RowText title={row.question} detail={row.answer !== "" ? row.answer : undefined} />
            <RowValue>
              <span className="text-muted text-[12px]">
                {row.person} → {row.assistant.replace("tag-", "")}
              </span>
              <Badge variant="outline">{row.state}</Badge>
            </RowValue>
          </Row>
        ))}
      </ul>
    </Group>
  );
}
