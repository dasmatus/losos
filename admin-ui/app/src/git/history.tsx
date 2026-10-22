import { Link, useNavigate, useSearchParams } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { useT } from "@/lib/i18n-react";
import { git, type Commit, type Repo } from "./api";
import { when, whenFull } from "./format";
import { BranchPicker, ErrorAlert, Loading, RepoHeader, Sha, WithRepo } from "./parts";
import { useLoad } from "./store";

/* A branch's history, and the list of branches. */

export function CommitsView() {
  return <WithRepo>{(repo) => <Commits repo={repo} />}</WithRepo>;
}

function Commits({ repo }: { repo: Repo }) {
  const t = useT();
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const ref = params.get("ref") ?? repo.default_branch;
  const o = repo.owner.login;
  const branches = useLoad((signal) => git.branches(o, repo.name, signal), [o, repo.name]);
  const commits = useLoad((signal) => git.commits(o, repo.name, ref, signal), [o, repo.name, ref]);
  return (
    <div className="flex flex-col gap-4">
      <RepoHeader repo={repo} />
      <div className="flex items-center gap-3">
        <BranchPicker
          branches={branches.data ?? []}
          value={ref}
          label={t("git.code.branch")}
          onChange={(b) => navigate(`/${o}/${repo.name}/commits?ref=${encodeURIComponent(b)}`)}
        />
      </div>
      {commits.error !== null ? (
        <ErrorAlert error={commits.error} />
      ) : commits.data === null ? (
        <Loading rows={6} />
      ) : commits.data.length === 0 ? (
        <p className="text-[13px] text-muted">{t("git.commits.none")}</p>
      ) : (
        <CommitTable commits={commits.data} />
      )}
    </div>
  );
}

export function CommitTable({ commits }: { commits: readonly Commit[] }) {
  const t = useT();
  return (
    <Table data-commits>
      <TableHeader>
        <TableRow className="hover:bg-transparent">
          <TableHead>{t("git.commits.col.subject")}</TableHead>
          <TableHead className="max-md:hidden">{t("git.commits.col.author")}</TableHead>
          <TableHead className="max-sm:hidden">{t("git.commits.col.date")}</TableHead>
          <TableHead className="w-0">{t("git.commits.col.id")}</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {commits.map((c) => (
          <TableRow key={c.sha}>
            <TableCell className="max-w-96 truncate font-medium text-ink" title={c.body || c.subject}>
              {c.subject}
            </TableCell>
            <TableCell className="whitespace-nowrap text-muted max-md:hidden" title={c.author.email}>
              {c.author.name}
            </TableCell>
            <TableCell className="whitespace-nowrap text-muted max-sm:hidden" title={whenFull(c.date)}>
              {when(c.date)}
            </TableCell>
            <TableCell>
              <Sha sha={c.sha} />
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}

export function BranchesView() {
  return <WithRepo>{(repo) => <Branches repo={repo} />}</WithRepo>;
}

function Branches({ repo }: { repo: Repo }) {
  const t = useT();
  const navigate = useNavigate();
  const o = repo.owner.login;
  const base = `/${o}/${repo.name}`;
  const branches = useLoad((signal) => git.branches(o, repo.name, signal), [o, repo.name]);
  return (
    <div className="flex flex-col gap-4">
      <RepoHeader repo={repo} />
      {branches.error !== null ? (
        <ErrorAlert error={branches.error} />
      ) : branches.data === null ? (
        <Loading />
      ) : branches.data.length === 0 ? (
        <p className="text-[13px] text-muted">{t("git.branches.none")}</p>
      ) : (
        <Table data-branches>
          <TableHeader>
            <TableRow className="hover:bg-transparent">
              <TableHead>{t("git.branches.col.name")}</TableHead>
              <TableHead className="max-md:hidden">{t("git.branches.col.last")}</TableHead>
              <TableHead className="max-sm:hidden">{t("git.commits.col.date")}</TableHead>
              <TableHead className="w-0">
                <span className="sr-only">{t("git.branches.col.actions")}</span>
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {branches.data.map((b) => (
              <TableRow key={b.name}>
                <TableCell>
                  <span className="flex items-center gap-2">
                    <Link to={`${base}/tree/?ref=${encodeURIComponent(b.name)}`} className="font-mono text-[13px] text-ink hover:underline">
                      {b.name}
                    </Link>
                    {b.default && <Badge variant="local">{t("git.branches.default")}</Badge>}
                  </span>
                </TableCell>
                <TableCell className="max-w-80 truncate text-muted max-md:hidden">{b.commit.subject}</TableCell>
                <TableCell className="whitespace-nowrap text-muted max-sm:hidden" title={whenFull(b.commit.date)}>
                  {when(b.commit.date)}
                </TableCell>
                <TableCell>
                  {!b.default && repo.permissions.push && (
                    <Button
                      size="xs"
                      variant="secondary"
                      onClick={() =>
                        navigate(`${base}/compare?base=${encodeURIComponent(repo.default_branch)}&head=${encodeURIComponent(b.name)}`)
                      }
                    >
                      {t("git.branches.compare")}
                    </Button>
                  )}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
    </div>
  );
}
