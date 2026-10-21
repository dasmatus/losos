import * as React from "react";
import { Link, useParams } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import { Copy01Icon, LockIcon } from "@hugeicons/core-free-icons";
import { PersonAvatar, type AvatarSize } from "@/components/person";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import { Field, FieldContent, FieldDescription, FieldLabel } from "@/components/ui/field";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { toast } from "@/components/ui/toast";
import { t as translate } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { GitError, type Branch, type Repo } from "./api";
import { notFound, useFaces, useRepo } from "./store";

/* Pieces every page of LosOS Git draws the same way. */

/** What went wrong, in the person's words. */
export function explain(err: unknown): string {
  if (err instanceof GitError) {
    if (err.status === 0) return translate("git.error.network");
    if (err.status === 404) return translate("git.error.notFound");
    return err.message;
  }
  return err instanceof Error ? err.message : String(err);
}

export function failed(title: string, err: unknown): void {
  toast.error(title, explain(err), { help: "git" });
}

export function PageTitle({ children, className }: { children: React.ReactNode; className?: string }) {
  return <h1 className={cn("text-[19px] leading-tight font-semibold tracking-tight", className)}>{children}</h1>;
}

export function ErrorAlert({ error }: { error: unknown }) {
  return (
    <Alert variant="crit">
      <AlertDescription>{explain(error)}</AlertDescription>
    </Alert>
  );
}

export function Loading({ rows = 4 }: { rows?: number }) {
  return (
    <div className="flex flex-col gap-2" data-skeleton>
      {Array.from({ length: rows }, (_, i) => (
        <Skeleton key={i} className="h-10 w-full" />
      ))}
    </div>
  );
}

export function BotBadge() {
  const t = useT();
  return (
    <Badge variant="local" className="px-1.5 py-0 text-[11px]">
      {t("git.bot")}
    </Badge>
  );
}

/** A person's face and name; a bot carries its badge. */
export function Person({
  login,
  name,
  bot = false,
  size = "xs",
  className,
}: {
  login: string;
  /** The name to show when the box has no profile for this account. */
  name?: string;
  bot?: boolean;
  size?: AvatarSize;
  className?: string;
}) {
  const faces = useFaces();
  const known = faces(login);
  const face = name !== undefined && known.displayName === login ? { ...known, displayName: name } : known;
  return (
    <span className={cn("inline-flex min-w-0 items-center gap-2", className)}>
      <PersonAvatar person={face} size={size} />
      <span className="truncate font-medium text-ink" title={login}>
        {face.displayName || login}
      </span>
      {bot && <BotBadge />}
    </span>
  );
}

/** owner / repo, the lock of a private one, and the clone address. */
export function RepoHeader({ repo, children }: { repo: Repo; children?: React.ReactNode }) {
  const t = useT();
  return (
    <div className="flex flex-col gap-2">
      <div className="flex min-h-9 flex-wrap items-center gap-x-3 gap-y-2">
        <PageTitle className="flex min-w-0 items-center gap-1.5">
          <Link to="/" className="truncate font-normal text-muted hover:text-ink">
            {repo.owner.login}
          </Link>
          <span className="text-faint">/</span>
          <Link to={`/${repo.owner.login}/${repo.name}`} className="truncate hover:underline">
            {repo.name}
          </Link>
        </PageTitle>
        {repo.private && (
          <Badge variant="outline">
            <HugeiconsIcon icon={LockIcon} size={12} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            {t("git.repo.private")}
          </Badge>
        )}
        <div className="ml-auto flex items-center gap-2">
          {children}
          <CloneButton url={repo.clone_url} />
        </div>
      </div>
      {repo.description !== "" && <p className="text-[13px] text-muted">{repo.description}</p>}
    </div>
  );
}

export function CloneButton({ url }: { url: string }) {
  const t = useT();
  return (
    <div className="flex items-center rounded-control border border-line bg-surface max-sm:hidden">
      <code className="max-w-72 truncate px-3 font-mono text-[12px] text-muted" title={url}>
        {url}
      </code>
      <Button
        variant="ghost"
        size="icon-sm"
        aria-label={t("git.repo.copyClone")}
        onClick={() => {
          void navigator.clipboard.writeText(url).then(
            () => toast.success(t("git.repo.copied")),
            () => toast.status(url),
          );
        }}
      >
        <HugeiconsIcon icon={Copy01Icon} size={15} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      </Button>
    </div>
  );
}

export function BranchPicker({
  branches,
  value,
  onChange,
  label,
  className,
}: {
  branches: readonly Branch[];
  value: string;
  onChange: (branch: string) => void;
  label: string;
  className?: string;
}) {
  const names = branches.map((b) => b.name);
  if (!names.includes(value) && value !== "") names.unshift(value);
  return (
    <NativeSelect aria-label={label} value={value} onChange={(e) => onChange(e.target.value)} className={className}>
      {names.map((n) => (
        <NativeSelectOption key={n} value={n}>
          {n}
        </NativeSelectOption>
      ))}
    </NativeSelect>
  );
}

export function Sha({ sha, className }: { sha: string; className?: string }) {
  return (
    <code className={cn("rounded-sm bg-sunk px-1.5 py-0.5 font-mono text-[12px] text-muted", className)} title={sha}>
      {sha.slice(0, 7)}
    </code>
  );
}

export function NotFound() {
  const t = useT();
  const { owner } = useParams();
  return (
    <Empty className="min-h-72">
      <EmptyHeader>
        <EmptyTitle>{t("git.notFound.title")}</EmptyTitle>
        <EmptyDescription>
          {owner === undefined ? t("git.notFound.body") : t("git.notFound.repo")}{" "}
          <Link to="/" className="underline">
            {t("git.nav.repos")}
          </Link>
        </EmptyDescription>
      </EmptyHeader>
    </Empty>
  );
}

/** The repository the route names, once it is loaded. */
export function WithRepo({ children }: { children: (repo: Repo) => React.ReactNode }) {
  const { owner = "", repo: name = "" } = useParams();
  const repo = useRepo(owner, name);
  if (notFound(repo.error)) return <NotFound />;
  if (repo.error !== null) return <ErrorAlert error={repo.error} />;
  if (repo.data === null) return <Loading />;
  return <>{children(repo.data)}</>;
}

/** A switch with its label and, maybe, a line under it: shadcn's "Switch
 *  with a description" Field, as the settings rows lay it out. */
export function SwitchField({
  label,
  description,
  checked,
  onChange,
}: {
  label: string;
  description?: string;
  checked: boolean;
  onChange: (on: boolean) => void;
}) {
  const id = React.useId();
  return (
    <Field orientation="horizontal" className="items-center gap-4">
      <FieldContent className="min-w-0">
        <FieldLabel htmlFor={id} className="w-auto font-normal">
          {label}
        </FieldLabel>
        {description !== undefined && <FieldDescription id={`${id}-d`}>{description}</FieldDescription>}
      </FieldContent>
      <Switch id={id} aria-describedby={description !== undefined ? `${id}-d` : undefined} isSelected={checked} onChange={onChange} />
    </Field>
  );
}
