import * as React from "react";
import { Link, useNavigate } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import { Add01Icon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { Dialog, DialogBody, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import { Field, FieldError, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Textarea } from "@/components/ui/textarea";
import { useT } from "@/lib/i18n-react";
import { git, type Visibility } from "./api";
import { when, whenFull } from "./format";
import { ErrorAlert, explain, Loading, PageTitle, VisibilityBadge, VisibilityField } from "./parts";
import { useLoad, useMe, useVisitor } from "./store";

/* The person's repositories: their own, the ones shared with them and the
 * ones the box shares with everyone on it. A visitor sees the public ones. */

export function RepoList() {
  const t = useT();
  const me = useMe();
  const visitor = useVisitor();
  const repos = useLoad(
    (signal) => (visitor ? git.publicRepos(signal) : me === null ? new Promise<never>(() => {}) : git.repos(signal)),
    [visitor, me],
  );
  const [creating, setCreating] = React.useState(false);
  const list = [...(repos.data ?? [])].sort((a, b) => b.updated.localeCompare(a.updated));

  return (
    <div className="flex flex-col gap-4">
      <div className="flex min-h-9 flex-wrap items-center gap-3">
        <PageTitle>{t(visitor ? "git.visitor.title" : "git.repos.title")}</PageTitle>
        {!visitor && (
          <Button size="sm" className="ml-auto" onClick={() => setCreating(true)}>
            <HugeiconsIcon icon={Add01Icon} size={15} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            {t("git.repos.new")}
          </Button>
        )}
      </div>
      <p className="text-[13px] text-muted">{t(visitor ? "git.visitor.summary" : "git.repos.summary")}</p>
      {repos.error !== null ? (
        <ErrorAlert error={repos.error} />
      ) : repos.data === null ? (
        <Loading />
      ) : list.length === 0 ? (
        <Empty className="min-h-64">
          <EmptyHeader>
            <EmptyTitle>{t(visitor ? "git.visitor.emptyTitle" : "git.repos.emptyTitle")}</EmptyTitle>
            <EmptyDescription>{t(visitor ? "git.visitor.emptyBody" : "git.repos.emptyBody")}</EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : (
        <Table data-repos>
          <TableHeader>
            <TableRow className="hover:bg-transparent">
              <TableHead>{t("git.repos.col.name")}</TableHead>
              <TableHead className="max-md:hidden">{t("git.repos.col.description")}</TableHead>
              <TableHead className="max-sm:hidden">{t("git.repos.col.updated")}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {list.map((r) => (
              <TableRow key={r.id}>
                <TableCell>
                  <span className="flex min-w-0 items-center gap-2">
                    <Link to={`/${r.owner.login}/${r.name}`} className="truncate font-medium text-ink hover:underline">
                      {me !== null && r.owner.login === me.login ? r.name : r.full_name}
                    </Link>
                    <VisibilityBadge visibility={r.visibility} compact />
                  </span>
                </TableCell>
                <TableCell className="max-w-80 truncate text-muted max-md:hidden">{r.description}</TableCell>
                <TableCell className="whitespace-nowrap text-muted max-sm:hidden" title={whenFull(r.updated)}>
                  {when(r.updated)}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
      <NewRepoDialog open={creating} onOpenChange={setCreating} />
    </div>
  );
}

function NewRepoDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const t = useT();
  const id = React.useId();
  const navigate = useNavigate();
  const [name, setName] = React.useState("");
  const [description, setDescription] = React.useState("");
  const [visibility, setVisibility] = React.useState<Visibility>("private");
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  React.useEffect(() => {
    if (open) {
      setName("");
      setDescription("");
      setVisibility("private");
      setBusy(false);
      setError(null);
    }
  }, [open]);

  const trimmed = name.trim();
  return (
    <Dialog open={open} onOpenChange={onOpenChange} labelledBy={`${id}-title`} dialogClassName="w-[min(30rem,calc(100vw-2rem))]">
      <form
        onSubmit={(event) => {
          event.preventDefault();
          if (trimmed === "" || busy) return;
          setBusy(true);
          git.createRepo(trimmed, description.trim(), visibility).then(
            (repo) => {
              onOpenChange(false);
              navigate(`/${repo.owner.login}/${repo.name}`);
            },
            (err: unknown) => {
              setBusy(false);
              setError(explain(err));
            },
          );
        }}
      >
        <DialogHeader>
          <DialogTitle id={`${id}-title`}>{t("git.repos.new")}</DialogTitle>
        </DialogHeader>
        <DialogBody className="flex flex-col gap-4">
          <Field invalid={error !== null}>
            <FieldLabel htmlFor={`${id}-name`}>{t("git.repos.name")}</FieldLabel>
            <Input
              id={`${id}-name`}
              value={name}
              autoComplete="off"
              spellCheck={false}
              onChange={(e) => {
                setName(e.target.value);
                setError(null);
              }}
            />
            {error !== null && <FieldError>{error}</FieldError>}
          </Field>
          <Field>
            <FieldLabel htmlFor={`${id}-description`}>{t("git.repos.description")}</FieldLabel>
            <Textarea id={`${id}-description`} rows={2} value={description} onChange={(e) => setDescription(e.target.value)} />
          </Field>
          <VisibilityField value={visibility} onChange={setVisibility} />
        </DialogBody>
        <DialogFooter>
          <Button type="button" variant="secondary" onClick={() => onOpenChange(false)}>
            {t("git.cancel")}
          </Button>
          <Button type="submit" disabled={trimmed === "" || busy}>
            {t("git.repos.create")}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}
