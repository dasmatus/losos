import * as React from "react";
import { Link, useNavigate, useParams, useSearchParams } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import { ArrowRight01Icon, File01Icon, Folder01Icon, GitPullRequestIcon, Link01Icon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import { Table, TableBody, TableCell, TableRow } from "@/components/ui/table";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { formatBytes } from "@/lib/apps";
import { git, type Dir, type File, type Repo, type TreeEntry } from "./api";
import { Markdown } from "./markdown";
import { BranchPicker, ErrorAlert, Loading, NotFound, RepoHeader } from "./parts";
import { notFound, useLoad, useRepo } from "./store";

/* A repository's code at a branch: a folder's entries and its README, or
 * one file with its line numbers. The branch rides in `?ref=`, so a branch
 * name with slashes in it never mixes with the path. */

export function CodeView({ file = false }: { file?: boolean }) {
  const { owner = "", repo: name = "", "*": rest = "" } = useParams();
  const repo = useRepo(owner, name);
  if (notFound(repo.error)) return <NotFound />;
  if (repo.error !== null) return <ErrorAlert error={repo.error} />;
  if (repo.data === null) return <Loading />;
  return <Code repo={repo.data} path={rest.replace(/^\/+|\/+$/gu, "")} file={file} />;
}

function Code({ repo, path, file }: { repo: Repo; path: string; file: boolean }) {
  const t = useT();
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const ref = params.get("ref") ?? repo.default_branch;
  const o = repo.owner.login;
  const base = `/${o}/${repo.name}`;
  const branches = useLoad((signal) => git.branches(o, repo.name, signal), [o, repo.name]);
  const entry = useLoad(
    (signal) => (repo.empty ? Promise.resolve(null) : git.contents(o, repo.name, path, ref, signal)),
    [o, repo.name, path, ref, repo.empty],
  );
  const isRoot = path === "";
  const readme = useLoad(
    (signal) => (repo.empty || !isRoot ? Promise.resolve(null) : git.readme(o, repo.name, ref, signal).catch(() => null)),
    [o, repo.name, ref, isRoot, repo.empty],
  );
  const refQuery = ref === repo.default_branch ? "" : `?ref=${encodeURIComponent(ref)}`;
  const link = (p: string, dir: boolean) => `${base}/${dir ? "tree" : "blob"}/${p}${refQuery}`;
  const parts = path === "" ? [] : path.split("/");

  return (
    <div className="flex flex-col gap-4">
      <RepoHeader repo={repo} />
      {repo.empty ? (
        <EmptyRepo repo={repo} />
      ) : (
        <>
          <div className="flex flex-wrap items-center gap-3">
            <BranchPicker
              branches={branches.data ?? []}
              value={ref}
              label={t("git.code.branch")}
              onChange={(b) => navigate(`${base}/${file ? "blob" : "tree"}/${path}${b === repo.default_branch ? "" : `?ref=${encodeURIComponent(b)}`}`)}
            />
            <nav aria-label={t("git.code.path")} className="flex min-w-0 flex-wrap items-center gap-1 text-[13.5px]">
              <Link to={`${base}${refQuery}`} className={cn("hover:underline", parts.length === 0 ? "font-medium text-ink" : "text-muted")}>
                {repo.name}
              </Link>
              {parts.map((part, i) => (
                <React.Fragment key={i}>
                  <HugeiconsIcon icon={ArrowRight01Icon} size={12} strokeWidth={1.5} color="currentColor" className="text-faint" aria-hidden="true" />
                  {i === parts.length - 1 ? (
                    <span className="font-medium text-ink">{part}</span>
                  ) : (
                    <Link to={link(parts.slice(0, i + 1).join("/"), true)} className="text-muted hover:underline">
                      {part}
                    </Link>
                  )}
                </React.Fragment>
              ))}
            </nav>
            {ref !== repo.default_branch && repo.permissions.push && (
              <Button
                size="sm"
                variant="secondary"
                className="ml-auto"
                onClick={() => navigate(`${base}/compare?base=${encodeURIComponent(repo.default_branch)}&head=${encodeURIComponent(ref)}`)}
              >
                <HugeiconsIcon icon={GitPullRequestIcon} size={15} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                {t("git.code.openPull")}
              </Button>
            )}
          </div>
          {notFound(entry.error) ? (
            <NotFound />
          ) : entry.error !== null ? (
            <ErrorAlert error={entry.error} />
          ) : entry.data === null ? (
            <Loading rows={6} />
          ) : entry.data.type === "dir" ? (
            <>
              <Entries dir={entry.data} link={link} />
              {readme.data !== null && readme.data.text !== null && (
                <section className="rounded-card border border-line bg-surface" data-readme>
                  <header className="border-b border-line px-4 py-2.5 text-[13px] font-medium">{readme.data.name}</header>
                  <Markdown source={readme.data.text} className="px-5 py-4" />
                </section>
              )}
            </>
          ) : (
            <FileView file={entry.data} />
          )}
        </>
      )}
    </div>
  );
}

function Entries({ dir, link }: { dir: Dir; link: (path: string, dir: boolean) => string }) {
  const t = useT();
  const sorted = [...dir.entries].sort((a, b) => {
    const rank = (e: TreeEntry) => (e.type === "dir" ? 0 : 1);
    return rank(a) - rank(b) || a.name.localeCompare(b.name);
  });
  if (sorted.length === 0) {
    return <p className="text-[13px] text-muted">{t("git.code.emptyFolder")}</p>;
  }
  return (
    <Table data-tree>
      <TableBody>
        {sorted.map((e) => {
          const isDir = e.type === "dir";
          const icon = isDir ? Folder01Icon : e.type === "symlink" ? Link01Icon : File01Icon;
          return (
            <TableRow key={e.path}>
              <TableCell>
                <span className="flex min-w-0 items-center gap-2.5">
                  <HugeiconsIcon icon={icon} size={16} strokeWidth={1.5} color="currentColor" className={isDir ? "text-accent" : "text-muted"} aria-hidden="true" />
                  {e.type === "submodule" ? (
                    <span className="truncate text-muted">{e.name}</span>
                  ) : (
                    <Link to={link(e.path, isDir)} className="truncate text-ink hover:underline">
                      {e.name}
                    </Link>
                  )}
                </span>
              </TableCell>
              <TableCell className="w-0 text-right whitespace-nowrap text-muted max-sm:hidden">
                {isDir ? "" : formatBytes(e.size)}
              </TableCell>
            </TableRow>
          );
        })}
      </TableBody>
    </Table>
  );
}

function FileView({ file }: { file: File }) {
  const t = useT();
  const markdown = /\.(md|markdown)$/iu.test(file.name);
  const lines = React.useMemo(() => {
    const text = file.text ?? "";
    const split = text.split("\n");
    if (split.length > 1 && split[split.length - 1] === "") split.pop();
    return split;
  }, [file.text]);
  return (
    <section className="overflow-hidden rounded-card border border-line bg-surface" data-file>
      <header className="flex flex-wrap items-center gap-3 border-b border-line px-4 py-2.5 text-[13px]">
        <span className="font-medium">{file.name}</span>
        <span className="text-muted">
          {file.binary ? formatBytes(file.size) : t("git.code.lines", { count: lines.length, size: formatBytes(file.size) })}
        </span>
      </header>
      {file.binary ? (
        <p className="px-4 py-6 text-center text-[13px] text-muted">{t("git.code.binary")}</p>
      ) : markdown ? (
        <Markdown source={file.text ?? ""} className="px-5 py-4" />
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full border-collapse font-mono text-[12.5px] leading-[1.6]">
            <tbody>
              {lines.map((l, i) => (
                <tr key={i} id={`L${i + 1}`}>
                  <td className="w-0 px-3 text-right align-top text-faint select-none">{i + 1}</td>
                  <td className="pr-4 whitespace-pre text-ink">{l}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {file.truncated && <p className="border-t border-line px-4 py-2 text-[12px] text-muted">{t("git.code.truncated")}</p>}
    </section>
  );
}

function EmptyRepo({ repo }: { repo: Repo }) {
  const t = useT();
  const commands = [
    `git remote add losos ${repo.clone_url}`,
    `git push -u losos ${repo.default_branch}`,
  ].join("\n");
  return (
    <Empty className="min-h-64">
      <EmptyHeader>
        <EmptyTitle>{t("git.code.emptyTitle")}</EmptyTitle>
        <EmptyDescription>{t("git.code.emptyBody")}</EmptyDescription>
      </EmptyHeader>
      <pre className="w-full max-w-xl overflow-x-auto rounded-control border border-line bg-sunk px-3 py-2 text-left font-mono text-[12.5px]">
        {commands}
      </pre>
    </Empty>
  );
}
