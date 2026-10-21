import * as React from "react";
import { useNavigate } from "react-router-dom";
import { Button } from "@/components/ui/button";
import { Field, FieldError, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { toast } from "@/components/ui/toast";
import { useT } from "@/lib/i18n-react";
import { git, type Repo } from "./api";
import { explain, NotFound, RepoHeader, SwitchField, WithRepo } from "./parts";

/* A repository's own settings, for the people who administer it: its
 * name, its description and who may see it. */

export function SettingsView() {
  return <WithRepo>{(repo) => (repo.permissions.admin ? <Settings repo={repo} /> : <NotFound />)}</WithRepo>;
}

function Settings({ repo }: { repo: Repo }) {
  const t = useT();
  const id = React.useId();
  const navigate = useNavigate();
  const [name, setName] = React.useState(repo.name);
  const [description, setDescription] = React.useState(repo.description);
  const [isPrivate, setPrivate] = React.useState(repo.private);
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const changed = name.trim() !== repo.name || description !== repo.description || isPrivate !== repo.private;

  return (
    <div className="flex flex-col gap-4">
      <RepoHeader repo={repo} />
      <form
        className="flex max-w-xl flex-col gap-4 rounded-card border border-line bg-surface p-4"
        data-repo-settings
        onSubmit={(e) => {
          e.preventDefault();
          if (!changed || busy) return;
          setBusy(true);
          git.editRepo(repo.owner.login, repo.name, { name: name.trim(), description, private: isPrivate }).then(
            (next) => {
              setBusy(false);
              toast.success(t("git.settings.saved"));
              navigate(`/${next.owner.login}/${next.name}/settings`, { replace: true });
            },
            (err: unknown) => {
              setBusy(false);
              setError(explain(err));
            },
          );
        }}
      >
        <h2 className="text-[14px] font-semibold">{t("git.settings.title")}</h2>
        <Field invalid={error !== null}>
          <FieldLabel htmlFor={`${id}-name`}>{t("git.repos.name")}</FieldLabel>
          <Input id={`${id}-name`} value={name} autoComplete="off" spellCheck={false} onChange={(e) => setName(e.target.value)} />
          {error !== null && <FieldError>{error}</FieldError>}
        </Field>
        <Field>
          <FieldLabel htmlFor={`${id}-description`}>{t("git.repos.description")}</FieldLabel>
          <Textarea id={`${id}-description`} rows={2} value={description} onChange={(e) => setDescription(e.target.value)} />
        </Field>
        <SwitchField label={t("git.repos.private")} description={t("git.repos.privateHint")} checked={isPrivate} onChange={setPrivate} />
        <div className="flex justify-end">
          <Button type="submit" disabled={!changed || busy}>
            {t("git.settings.save")}
          </Button>
        </div>
      </form>
    </div>
  );
}
