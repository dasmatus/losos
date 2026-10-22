import * as React from "react";
import { Link } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { useT } from "@/lib/i18n-react";
import { git, type Repo, type Worktree } from "./api";
import { when, whenFull } from "./format";
import { ErrorAlert, failed, Loading, Person, RepoHeader, Sha, WithRepo } from "./parts";
import { useLoad, useMe } from "./store";

/* The checkouts that push to a repository. A repository on the box has no
 * worktrees of its own; each `git worktree add` on a computer can name
 * itself in its pushes (push options `worktree=` and `computer=`), and the
 * box keeps, per person, the branch and commit each one last pushed
 * (backend/src/forge/worktrees.rs). */

export function WorktreesView() {
  return <WithRepo>{(repo) => <Worktrees repo={repo} />}</WithRepo>;
}

function Worktrees({ repo }: { repo: Repo }) {
  const t = useT();
  const me = useMe();
  const o = repo.owner.login;
  const list = useLoad((signal) => git.worktrees(o, repo.name, signal), [o, repo.name]);
  const pulls = useLoad((signal) => git.pulls(o, repo.name, "open", signal).catch(() => []), [o, repo.name]);
  const pullOf = (branch: string | undefined) => (pulls.data ?? []).find((p) => p.pull_request?.head === branch);

  return (
    <div className="flex flex-col gap-4">
      <RepoHeader repo={repo} />
      <p className="text-[13px] text-muted">{t("git.worktrees.summary")}</p>
      {list.error !== null ? (
        <ErrorAlert error={list.error} />
      ) : list.data === null ? (
        <Loading />
      ) : list.data.length === 0 ? (
        <Empty className="min-h-48">
          <EmptyHeader>
            <EmptyTitle>{t("git.worktrees.emptyTitle")}</EmptyTitle>
            <EmptyDescription>{t("git.worktrees.emptyBody")}</EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : (
        <Table data-worktrees>
          <TableHeader>
            <TableRow className="hover:bg-transparent">
              <TableHead>{t("git.worktrees.col.name")}</TableHead>
              <TableHead>{t("git.worktrees.col.branch")}</TableHead>
              <TableHead className="max-md:hidden">{t("git.worktrees.col.who")}</TableHead>
              <TableHead className="max-sm:hidden">{t("git.worktrees.col.pushed")}</TableHead>
              <TableHead className="w-0">
                <span className="sr-only">{t("git.branches.col.actions")}</span>
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {list.data.map((w) => (
              <Row
                key={w.id}
                repo={repo}
                worktree={w}
                pull={pullOf(w.branch)?.number ?? null}
                canForget={(me !== null && me.login === w.owner.login) || repo.permissions.admin}
                onForgotten={list.reload}
              />
            ))}
          </TableBody>
        </Table>
      )}
      <HowTo repo={repo} />
    </div>
  );
}

function Row({
  repo,
  worktree: w,
  pull,
  canForget,
  onForgotten,
}: {
  repo: Repo;
  worktree: Worktree;
  pull: number | null;
  canForget: boolean;
  onForgotten: () => void;
}) {
  const t = useT();
  const base = `/${repo.owner.login}/${repo.name}`;
  const branch = w.branch ?? "";
  return (
    <TableRow data-worktree={w.name}>
      <TableCell>
        <div className="flex flex-col gap-0.5">
          <span className="font-medium text-ink">{w.name}</span>
          {w.computer !== "" && <span className="text-[12px] text-muted">{t("git.worktrees.computer", { computer: w.computer })}</span>}
        </div>
      </TableCell>
      <TableCell>
        <div className="flex flex-wrap items-center gap-2">
          {w.branch_exists === false ? (
            <span className="font-mono text-[12.5px] text-muted line-through">{branch}</span>
          ) : (
            <Link to={`${base}/tree/?ref=${encodeURIComponent(branch)}`} className="font-mono text-[12.5px] text-ink hover:underline">
              {branch}
            </Link>
          )}
          <Sha sha={w.head} />
          {w.branch_exists === false ? (
            <Badge variant="neutral">{t("git.worktrees.gone")}</Badge>
          ) : w.behind_branch === true ? (
            <Badge variant="warn">{t("git.worktrees.behind")}</Badge>
          ) : null}
          {pull !== null && (
            <Link to={`${base}/pulls/${pull}`} className="text-[12.5px] text-accent hover:underline">
              #{pull}
            </Link>
          )}
        </div>
      </TableCell>
      <TableCell className="max-md:hidden">
        <Person login={w.owner.login} />
      </TableCell>
      <TableCell className="whitespace-nowrap text-muted max-sm:hidden" title={whenFull(w.updated_at)}>
        {when(w.updated_at)}
      </TableCell>
      <TableCell>
        {canForget && (
          <Button
            size="xs"
            variant="ghost"
            onClick={() => {
              git.forgetWorktree(repo.owner.login, repo.name, w.id).then(onForgotten, (err: unknown) => failed(t("git.worktrees.forgetFailed"), err));
            }}
          >
            {t("git.worktrees.forget")}
          </Button>
        )}
      </TableCell>
    </TableRow>
  );
}

function HowTo({ repo }: { repo: Repo }) {
  const t = useT();
  const commands = React.useMemo(
    () =>
      [
        "git config extensions.worktreeConfig true",
        "git worktree add ../fix-backup -b fix-backup",
        "cd ../fix-backup",
        'git config --worktree push.pushOption "worktree=fix-backup"',
        'git config --worktree --add push.pushOption "computer=laptop"',
        "git push -u origin fix-backup",
      ].join("\n"),
    [],
  );
  return (
    <section className="flex flex-col gap-2 rounded-card border border-line bg-surface p-4" data-worktree-howto>
      <h2 className="text-[14px] font-semibold">{t("git.worktrees.howTitle")}</h2>
      <p className="text-[13px] text-muted">{t("git.worktrees.howBody", { repo: repo.name })}</p>
      <pre className="overflow-x-auto rounded-control border border-line bg-sunk px-3 py-2 font-mono text-[12.5px]">{commands}</pre>
    </section>
  );
}
