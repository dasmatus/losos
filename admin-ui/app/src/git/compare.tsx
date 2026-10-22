import * as React from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import { ArrowLeft01Icon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Field, FieldError, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { useT } from "@/lib/i18n-react";
import { git, type Repo } from "./api";
import { DiffFiles } from "./diff";
import { CommitTable } from "./history";
import { BranchPicker, ErrorAlert, explain, Loading, RepoHeader, WithRepo } from "./parts";
import { useLoad } from "./store";

/* What a branch would bring into another, and the form that opens a pull
 * request for it. */

export function CompareView() {
  return <WithRepo>{(repo) => <Compare repo={repo} />}</WithRepo>;
}

function Compare({ repo }: { repo: Repo }) {
  const t = useT();
  const id = React.useId();
  const navigate = useNavigate();
  const [params, setParams] = useSearchParams();
  const o = repo.owner.login;
  const base = params.get("base") ?? repo.default_branch;
  const branches = useLoad((signal) => git.branches(o, repo.name, signal), [o, repo.name]);
  const firstOther = (branches.data ?? []).find((b) => b.name !== base)?.name ?? "";
  const head = params.get("head") ?? firstOther;
  const compare = useLoad(
    (signal) => (head === "" || head === base ? Promise.resolve(null) : git.compare(o, repo.name, base, head, signal)),
    [o, repo.name, base, head],
  );
  const [title, setTitle] = React.useState("");
  const [body, setBody] = React.useState("");
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  // The title starts as the only commit's subject, or the branch's name.
  React.useEffect(() => {
    const commits = compare.data?.commits ?? [];
    setTitle(commits.length === 1 ? (commits[0]?.subject ?? head) : head.replace(/[-_/]+/gu, " "));
  }, [compare.data, head]);

  const set = (next: { base?: string; head?: string }) =>
    setParams({ base: next.base ?? base, head: next.head ?? head }, { replace: true });

  return (
    <div className="flex flex-col gap-4">
      <RepoHeader repo={repo} />
      <div className="flex items-center gap-2">
        <Button variant="ghost" size="sm" onClick={() => navigate(`/${o}/${repo.name}/pulls`)}>
          <HugeiconsIcon icon={ArrowLeft01Icon} size={15} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          {t("git.nav.pulls")}
        </Button>
      </div>
      <h2 className="text-[15px] font-semibold tracking-tight">{t("git.compare.title")}</h2>
      <div className="flex flex-wrap items-center gap-2 text-[13px] text-muted">
        <span>{t("git.compare.into")}</span>
        <BranchPicker branches={branches.data ?? []} value={base} label={t("git.compare.base")} onChange={(b) => set({ base: b })} />
        <HugeiconsIcon icon={ArrowLeft01Icon} size={15} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        <span>{t("git.compare.from")}</span>
        <BranchPicker branches={branches.data ?? []} value={head} label={t("git.compare.head")} onChange={(b) => set({ head: b })} />
      </div>
      {head === "" || head === base ? (
        <p className="text-[13px] text-muted">{t("git.compare.pick")}</p>
      ) : compare.error !== null ? (
        <ErrorAlert error={compare.error} />
      ) : compare.data === null ? (
        <Loading rows={5} />
      ) : compare.data.ahead_by === 0 ? (
        <Alert>
          <AlertDescription>{t("git.compare.nothing", { head, base })}</AlertDescription>
        </Alert>
      ) : (
        <>
          <Alert variant={compare.data.mergeable ? "ok" : "warn"}>
            <AlertDescription>
              {t(compare.data.mergeable ? "git.compare.clean" : "git.compare.conflicts")}{" "}
              {t("git.compare.ahead", { count: compare.data.ahead_by })}
            </AlertDescription>
          </Alert>
          {repo.permissions.push && (
            <form
              className="flex flex-col gap-3 rounded-card border border-line bg-surface p-4"
              data-new-pull
              onSubmit={(e) => {
                e.preventDefault();
                if (title.trim() === "" || busy) return;
                setBusy(true);
                git.createPull(o, repo.name, { title: title.trim(), body, head, base }).then(
                  (pr) => navigate(`/${o}/${repo.name}/pulls/${pr.number}`),
                  (err: unknown) => {
                    setBusy(false);
                    setError(explain(err));
                  },
                );
              }}
            >
              <Field invalid={error !== null}>
                <FieldLabel htmlFor={`${id}-title`}>{t("git.compare.prTitle")}</FieldLabel>
                <Input id={`${id}-title`} value={title} onChange={(e) => setTitle(e.target.value)} />
                {error !== null && <FieldError>{error}</FieldError>}
              </Field>
              <Field>
                <FieldLabel htmlFor={`${id}-body`}>{t("git.compare.prBody")}</FieldLabel>
                <Textarea id={`${id}-body`} rows={5} value={body} onChange={(e) => setBody(e.target.value)} placeholder={t("git.compare.prBodyHint")} />
              </Field>
              <div className="flex justify-end">
                <Button type="submit" disabled={title.trim() === "" || busy}>
                  {t("git.compare.open")}
                </Button>
              </div>
            </form>
          )}
          <h3 className="text-[14px] font-semibold">{t("git.compare.commits", { count: compare.data.commits.length })}</h3>
          <CommitTable commits={compare.data.commits} />
          <DiffFiles files={compare.data.files} />
        </>
      )}
    </div>
  );
}
