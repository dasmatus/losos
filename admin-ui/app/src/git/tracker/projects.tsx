import * as React from "react";
import { Link, useNavigate } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import { RepeatIcon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Field, FieldDescription, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Textarea } from "@/components/ui/textarea";
import { toast } from "@/components/ui/toast";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { git } from "../api";
import { ErrorAlert, failed, Loading, PageTitle } from "../parts";
import { useLoad } from "../store";
import { closed, tracker, type Project } from "./api";
import { addDays, cycleName, shortDay } from "./parts";
import { useTracker } from "./store";

/* Making a project, and a project's cycles: stretches of time with the
 * issues meant for each and how far along they are. */

/** The key the box would pick for a name, shown before it is made
 *  (backend/src/forge/tracker/mod.rs `key_for`). */
export function keyFor(name: string): string {
  const words = name.split(/[^A-Za-z0-9]+|(?<=[a-z])(?=[A-Z])/u).filter((w) => /[A-Za-z]/u.test(w));
  const letters = (s: string) => s.replace(/[^A-Za-z]/gu, "").toUpperCase();
  let key = words.length >= 2 ? words.map((w) => letters(w)[0] ?? "").join("").slice(0, 6) : letters(words[0] ?? "").slice(0, 3);
  if (key.length === 1) key = key + key;
  return key.length >= 2 ? key : "ISS";
}

export function NewProject() {
  const t = useT();
  const navigate = useNavigate();
  const { reloadHome } = useTracker();
  const repos = useLoad((signal) => git.repos(signal), []);
  const [name, setName] = React.useState("");
  const [key, setKey] = React.useState("");
  const [description, setDescription] = React.useState("");
  const [visibility, setVisibility] = React.useState<"private" | "internal">("private");
  const [repo, setRepo] = React.useState("");
  const [busy, setBusy] = React.useState(false);
  const id = React.useId();
  const shownKey = key !== "" ? key.toUpperCase() : name.trim() !== "" ? keyFor(name) : "";

  const submit = async () => {
    setBusy(true);
    try {
      const p = await tracker.createProject({ name: name.trim(), key: key.trim() || undefined, description, visibility, repo: repo || undefined });
      toast.done(t("tracker.project.made", { name: p.name, key: p.key }));
      reloadHome();
      navigate(`/issues/p/${p.key}`);
    } catch (e) {
      failed(t("tracker.error.project"), e);
      setBusy(false);
    }
  };

  return (
    <div className="flex max-w-xl flex-col gap-4">
      <PageTitle>{t("tracker.project.new")}</PageTitle>
      <Card className="p-5">
        <form
          className="flex flex-col gap-4"
          onSubmit={(e) => {
            e.preventDefault();
            void submit();
          }}
          data-new-project
        >
          <Field>
            <FieldLabel htmlFor={`${id}-name`}>{t("tracker.project.name")}</FieldLabel>
            <Input id={`${id}-name`} value={name} onChange={(e) => setName(e.target.value)} autoFocus maxLength={80} />
          </Field>
          <Field>
            <FieldLabel htmlFor={`${id}-key`}>{t("tracker.project.key")}</FieldLabel>
            <Input id={`${id}-key`} value={key} onChange={(e) => setKey(e.target.value.replace(/[^A-Za-z0-9]/gu, "").slice(0, 6))} placeholder={shownKey} className="w-32 font-mono uppercase" />
            <FieldDescription>{t("tracker.project.keyHint", { example: `${shownKey || "LOS"}-12` })}</FieldDescription>
          </Field>
          <Field>
            <FieldLabel htmlFor={`${id}-description`}>{t("tracker.project.description")}</FieldLabel>
            <Textarea id={`${id}-description`} value={description} onChange={(e) => setDescription(e.target.value)} rows={3} />
          </Field>
          <Field>
            <FieldLabel id={`${id}-vis`}>{t("tracker.project.visibility")}</FieldLabel>
            <ToggleGroup variant="outline" value={visibility} onValueChange={(v) => (v === "private" || v === "internal") && setVisibility(v)} aria-labelledby={`${id}-vis`}>
              <ToggleGroupItem value="private">{t("tracker.project.private")}</ToggleGroupItem>
              <ToggleGroupItem value="internal">{t("tracker.project.internal")}</ToggleGroupItem>
            </ToggleGroup>
            <FieldDescription>{t(visibility === "private" ? "tracker.project.privateHint" : "tracker.project.internalHint")}</FieldDescription>
          </Field>
          <Field>
            <FieldLabel htmlFor={`${id}-repo`}>{t("tracker.project.repo")}</FieldLabel>
            <NativeSelect id={`${id}-repo`} value={repo} onChange={(e) => setRepo(e.target.value)}>
              <NativeSelectOption value="">{t("tracker.project.noRepo")}</NativeSelectOption>
              {(repos.data ?? [])
                .filter((r) => r.permissions.admin)
                .map((r) => (
                  <NativeSelectOption key={r.full_name} value={r.full_name}>
                    {r.full_name}
                  </NativeSelectOption>
                ))}
            </NativeSelect>
            <FieldDescription>{t("tracker.project.repoHint")}</FieldDescription>
          </Field>
          <div className="flex justify-end gap-2">
            <Button type="button" variant="secondary" onClick={() => navigate(-1)}>
              {t("git.cancel")}
            </Button>
            <Button type="submit" disabled={busy || name.trim() === ""}>
              {t("tracker.project.make")}
            </Button>
          </div>
        </form>
      </Card>
    </div>
  );
}

export function CyclesView({ project }: { project: Project }) {
  const t = useT();
  const { home, reloadHome } = useTracker();
  const today = home?.today ?? "";
  const issues = useLoad((signal) => tracker.issues({ project: [project.key] }, signal), [project.key]);
  const write = project.permission === "write" || project.permission === "admin";
  const last = [...project.cycles].sort((a, b) => b.ends.localeCompare(a.ends))[0];
  const nextStart = last !== undefined && last.ends >= today ? addDays(last.ends, 1) : today;
  const [starts, setStarts] = React.useState(nextStart);
  const [ends, setEnds] = React.useState(addDays(nextStart, 13));
  const [name, setName] = React.useState("");
  const [adding, setAdding] = React.useState(false);
  const cycles = [...project.cycles].sort((a, b) => b.starts.localeCompare(a.starts));

  const add = async () => {
    try {
      await tracker.addCycle(project.key, { name: name.trim(), starts, ends });
      setAdding(false);
      setName("");
      reloadHome();
    } catch (e) {
      failed(t("tracker.error.change"), e);
    }
  };

  return (
    <div className="flex flex-col gap-3" data-cycles>
      <div className="flex flex-wrap items-center gap-3">
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <PageTitle className="flex items-center gap-2">
            <span className="rounded-control bg-sunk px-1.5 py-0.5 font-mono text-[12px] font-medium text-muted">{project.key}</span>
            {t("tracker.project.cyclesOf", { name: project.name })}
          </PageTitle>
          <ProjectTabs project={project} tab="cycles" cycle={undefined} />
        </div>
        {write && !adding && <Button onClick={() => setAdding(true)}>{t("tracker.cycle.new")}</Button>}
      </div>
      {adding && (
        <Card className="animate-rise p-4">
          <form
            className="flex flex-wrap items-end gap-3"
            onSubmit={(e) => {
              e.preventDefault();
              void add();
            }}
          >
            <Field className="w-48">
              <FieldLabel>{t("tracker.cycle.name")}</FieldLabel>
              <Input value={name} onChange={(e) => setName(e.target.value)} placeholder={t("tracker.cycle.numbered", { n: project.cycles.length + 1 })} />
            </Field>
            <Field className="w-40">
              <FieldLabel>{t("tracker.cycle.starts")}</FieldLabel>
              <Input type="date" value={starts} onChange={(e) => setStarts(e.target.value)} />
            </Field>
            <Field className="w-40">
              <FieldLabel>{t("tracker.cycle.ends")}</FieldLabel>
              <Input type="date" value={ends} min={starts} onChange={(e) => setEnds(e.target.value)} />
            </Field>
            <Button type="submit" disabled={ends < starts}>
              {t("tracker.cycle.make")}
            </Button>
            <Button type="button" variant="ghost" onClick={() => setAdding(false)}>
              {t("git.cancel")}
            </Button>
          </form>
        </Card>
      )}
      {issues.error !== null && <ErrorAlert error={issues.error} />}
      {issues.data === null && issues.error === null && <Loading rows={3} />}
      {issues.data !== null && cycles.length === 0 && (
        <Empty>
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <HugeiconsIcon icon={RepeatIcon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            </EmptyMedia>
            <EmptyTitle>{t("tracker.cycle.emptyTitle")}</EmptyTitle>
            <EmptyDescription>{t("tracker.cycle.emptyBody")}</EmptyDescription>
          </EmptyHeader>
        </Empty>
      )}
      {issues.data !== null && (
        <ul className="flex flex-col gap-2">
          {cycles.map((c) => {
            const mine = (issues.data ?? []).filter((i) => i.cycleId === c.id);
            const done = mine.filter((i) => closed(i.state.category)).length;
            const points = mine.reduce((n, i) => n + (i.estimate ?? 0), 0);
            const now = c.starts <= today && today <= c.ends;
            const share = mine.length === 0 ? 0 : done / mine.length;
            return (
              <li key={c.id}>
                <Link
                  to={`/issues/p/${project.key}?cycle=${c.id}`}
                  className={cn("flex flex-wrap items-center gap-3 rounded-card border bg-surface px-4 py-3 shadow-card transition-[border-color] duration-150 hover:border-faint", now ? "border-accent/50" : "border-line")}
                  data-cycle={c.number}
                >
                  <span className="flex min-w-0 flex-1 flex-col">
                    <span className="flex items-center gap-2 text-[14px] font-medium">
                      {cycleName(c, t)}
                      {now && <span className="rounded-full bg-accent-wash px-2 text-[11.5px] text-accent">{t("tracker.cycle.now")}</span>}
                    </span>
                    <span className="text-[12.5px] text-muted tabular-nums">
                      {shortDay(c.starts)} – {shortDay(c.ends)}
                      {points > 0 && ` · ${t("tracker.estimate.points", { count: points })}`}
                    </span>
                  </span>
                  <span className="flex items-center gap-2 text-[12.5px] text-muted tabular-nums">
                    {t("tracker.cycle.progress", { done, all: mine.length })}
                    <span className="h-1.5 w-28 overflow-hidden rounded-full bg-sunk" aria-hidden="true">
                      <span className="block h-full origin-left rounded-full bg-accent" style={{ transform: `scaleX(${share})` }} />
                    </span>
                  </span>
                </Link>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}

export function ProjectTabs({ project, tab, cycle }: { project: Project; tab: "issues" | "backlog" | "cycles"; cycle: number | undefined }) {
  const t = useT();
  const navigate = useNavigate();
  return (
    <div className="flex flex-wrap items-center gap-1 text-[13px]">
      {(
        [
          ["issues", `/issues/p/${project.key}`, "tracker.project.issues"],
          ["backlog", `/issues/p/${project.key}/backlog`, "tracker.project.backlog"],
          ["cycles", `/issues/p/${project.key}/cycles`, "tracker.project.cycles"],
        ] as const
      ).map(([id, to, label]) => (
        <Button key={id} variant={tab === id ? "secondary" : "ghost"} size="xs" onClick={() => navigate(to)} aria-current={tab === id ? "page" : undefined}>
          {t(label)}
        </Button>
      ))}
      {cycle !== undefined && (
        <Button variant="ghost" size="xs" onClick={() => navigate(`/issues/p/${project.key}?cycle=${cycle}`)}>
          {t("tracker.cycle.current")}
        </Button>
      )}
      {project.repos.map((r) => (
        <Link key={`${r.owner}/${r.name}`} to={`/${r.owner}/${r.name}`} className="rounded-full">
          <Badge variant="outline" className="py-0 text-[11.5px] hover:border-faint">
            {r.owner}/{r.name}
          </Badge>
        </Link>
      ))}
    </div>
  );
}
