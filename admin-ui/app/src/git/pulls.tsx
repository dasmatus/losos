import { Link, useNavigate, useSearchParams } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import { Add01Icon } from "@hugeicons/core-free-icons";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import { Table, TableBody, TableCell, TableRow } from "@/components/ui/table";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { useT } from "@/lib/i18n-react";
import { git, type Issue, type Repo } from "./api";
import { when, whenFull } from "./format";
import { ErrorAlert, Loading, Person, RepoHeader, WithRepo } from "./parts";
import { useLoad } from "./store";

/* A repository's pull requests, open or closed. */

export function PullsView() {
  return <WithRepo>{(repo) => <Pulls repo={repo} />}</WithRepo>;
}

export function PullState({ pull }: { pull: Pick<Issue, "state" | "pull_request"> & { merged?: boolean } }) {
  const t = useT();
  const merged = pull.merged ?? pull.pull_request?.merged ?? false;
  if (merged) return <Badge variant="local">{t("git.pull.merged")}</Badge>;
  if (pull.state === "closed") return <Badge variant="neutral">{t("git.pull.closed")}</Badge>;
  return <Badge variant="ok">{t("git.pull.open")}</Badge>;
}

function Pulls({ repo }: { repo: Repo }) {
  const t = useT();
  const navigate = useNavigate();
  const [params, setParams] = useSearchParams();
  const state = params.get("state") === "closed" ? "closed" : "open";
  const o = repo.owner.login;
  const base = `/${o}/${repo.name}`;
  const pulls = useLoad((signal) => git.pulls(o, repo.name, state, signal), [o, repo.name, state]);

  return (
    <div className="flex flex-col gap-4">
      <RepoHeader repo={repo} />
      <div className="flex flex-wrap items-center gap-3">
        <ToggleGroup
          value={state}
          onValueChange={(v) => setParams(v === "closed" ? { state: "closed" } : {})}
          aria-label={t("git.pulls.which")}
        >
          <ToggleGroupItem value="open">{t("git.pulls.open")}</ToggleGroupItem>
          <ToggleGroupItem value="closed">{t("git.pulls.closed")}</ToggleGroupItem>
        </ToggleGroup>
        {repo.permissions.push && !repo.empty && (
          <Button size="sm" className="ml-auto" onClick={() => navigate(`${base}/compare`)}>
            <HugeiconsIcon icon={Add01Icon} size={15} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            {t("git.pulls.new")}
          </Button>
        )}
      </div>
      {pulls.error !== null ? (
        <ErrorAlert error={pulls.error} />
      ) : pulls.data === null ? (
        <Loading />
      ) : pulls.data.length === 0 ? (
        <Empty className="min-h-56">
          <EmptyHeader>
            <EmptyTitle>{t(state === "open" ? "git.pulls.noneOpen" : "git.pulls.noneClosed")}</EmptyTitle>
            <EmptyDescription>{t("git.pulls.noneBody")}</EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : (
        <Table data-pulls>
          <TableBody>
            {pulls.data.map((p) => (
              <TableRow key={p.number}>
                <TableCell>
                  <div className="flex min-w-0 flex-col gap-1 py-0.5">
                    <span className="flex min-w-0 flex-wrap items-center gap-2">
                      <Link to={`${base}/pulls/${p.number}`} className="truncate font-medium text-ink hover:underline">
                        {p.title}
                      </Link>
                      <PullState pull={p} />
                    </span>
                    <span className="flex flex-wrap items-center gap-x-2 gap-y-1 text-[12.5px] text-muted">
                      <span>#{p.number}</span>
                      <Person login={p.user.login} />
                      <span title={whenFull(p.created_at)}>{t("git.pulls.opened", { when: when(p.created_at) })}</span>
                      {p.pull_request?.head !== null && p.pull_request?.head !== undefined && (
                        <span className="font-mono text-[12px]">
                          {p.pull_request.head} → {p.pull_request.base}
                        </span>
                      )}
                    </span>
                  </div>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
    </div>
  );
}
