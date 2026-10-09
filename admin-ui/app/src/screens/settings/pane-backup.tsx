import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Alert02Icon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogBody,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input, MonoInput } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Spinner } from "@/components/ui/progress";
import {
  getRecoveryCode,
  isUnauthorized,
  type BackupResponse,
  type BackupTarget,
  type BackupTargetInput,
  type StorageClass,
} from "@/lib/api";
import { intlTag, type MessageKey } from "@/lib/i18n";
import { useLocale, useT } from "@/lib/i18n-react";
import { formatBytes } from "./format";
import {
  Group,
  GroupCaption,
  GroupTitle,
  PaneSection,
  Row,
  RowText,
  RowValue,
  StackRow,
} from "./rows";
import { useBackup, type BackupData } from "./use-backup";

/* Backups to a bucket the owner rents, and the way back from one.
 *
 * Any S3-compatible bucket will do: Amazon's, Backblaze's, a MinIO on the
 * owner's own NAS. The box encrypts everything before it leaves (restic,
 * modules/backup.nix) and the key is the recovery code printed in the
 * wizard, so the bucket's owner holds only ciphertext and the code is what
 * opens the backup again, on this box or on a new one.
 *
 * Nothing here is a setting of the box's configuration, so the pane sits
 * outside the Apply bar: every button is its own request to lososd. The
 * secret key goes in and never comes back out; the form shows that one is
 * stored and leaves the field empty to keep it.
 *
 * On Amazon S3 the owner can put the backups in Glacier. The two cold
 * classes make a restore wait hours while AWS thaws the data, so the pane
 * says so beside the choice and again before and during a restore. */

export function BackupPane({ locked, rebuilding }: { locked: boolean; rebuilding: boolean }) {
  const t = useT();
  const backup = useBackup(!locked);
  const { state } = backup;

  if (state.kind === "loading") {
    return (
      <PaneSection>
        <Group>
          <Row last>
            <RowText title={t("panes.backup.loading")} />
            <Spinner size={16} />
          </Row>
        </Group>
      </PaneSection>
    );
  }
  if (state.kind === "failed") {
    return (
      <PaneSection>
        <Group>
          <Row last>
            <RowText title={t("panes.backup.unreachable")} detail={state.message} />
            <Button size="sm" variant="secondary" disabled={locked} onClick={backup.refresh}>
              {t("panes.market.refresh")}
            </Button>
          </Row>
        </Group>
      </PaneSection>
    );
  }

  const { view } = state;
  const running = view.job?.state === "running";
  const disabled = locked || backup.busy || view.erase !== null;
  /* lososd refuses a backup or a restore while a change is being applied
   * (erase.rs, ensure_idle); the buttons say so before anyone clicks. */
  const waiting = rebuilding && !running;
  return (
    <>
      {waiting && <WaitingForChange />}
      <TargetSection view={view} backup={backup} disabled={disabled || running} />
      <CodeSection locked={locked} />
      <BackupsSection view={view} backup={backup} disabled={disabled || running || waiting} />
      <RestoreSection view={view} backup={backup} disabled={disabled || running || waiting} />
    </>
  );
}

// ── The bucket ──────────────────────────────────────────────────────────

/* AWS's own names for the Glacier classes are product names and stay in
 * English; only Standard is translated. */
const CLASSES: { id: StorageClass; name?: string; detail: MessageKey }[] = [
  { id: "standard", detail: "panes.backup.target.classStandardDetail" },
  {
    id: "glacierInstant",
    name: "Glacier Instant Retrieval",
    detail: "panes.backup.target.classInstantDetail",
  },
  {
    id: "glacier",
    name: "Glacier Flexible Retrieval",
    detail: "panes.backup.target.classGlacierDetail",
  },
  { id: "deepArchive", name: "Glacier Deep Archive", detail: "panes.backup.target.classDeepDetail" },
];

/* The same test lososd makes (backup.rs, `is_aws`): only AWS's own S3 has
 * the Glacier classes. */
const AWS_ENDPOINT = /^https:\/\/s3[^/:]*\.amazonaws\.com(:\d+)?\/*$/;

function classOf(target: BackupTarget | null): StorageClass {
  return target?.storageClass ?? "standard";
}

function needsThaw(target: BackupTarget | null): boolean {
  const c = classOf(target);
  return c === "glacier" || c === "deepArchive";
}

function useClassName(): (id: StorageClass) => string {
  const t = useT();
  return (id) => CLASSES.find((c) => c.id === id)?.name ?? t("panes.backup.target.classStandard");
}

const EMPTY: BackupTargetInput = {
  endpoint: "",
  bucket: "",
  prefix: "",
  region: "",
  accessKeyId: "",
  secretAccessKey: "",
  storageClass: "standard",
};

function draftOf(target: BackupTarget | null): BackupTargetInput {
  if (target === null) return EMPTY;
  return { ...target, secretAccessKey: "", storageClass: classOf(target) };
}


/* A change is being applied: backups, restores and the erase wait for it. */
function WaitingForChange() {
  const t = useT();
  return (
    <PaneSection data-testid="backup-waiting">
      <Group>
        <Row last>
          <RowText title={t("panes.backup.waiting.title")} detail={t("panes.backup.waiting.detail")} />
          <Spinner size={16} />
        </Row>
      </Group>
    </PaneSection>
  );
}

function TargetSection({
  view,
  backup,
  disabled,
}: {
  view: BackupResponse;
  backup: BackupData;
  disabled: boolean;
}) {
  const t = useT();
  const [editing, setEditing] = React.useState(view.target === null);
  const [draft, setDraft] = React.useState<BackupTargetInput>(() => draftOf(view.target));
  const ids = {
    endpoint: React.useId(),
    bucket: React.useId(),
    prefix: React.useId(),
    region: React.useId(),
    key: React.useId(),
    secret: React.useId(),
    class: React.useId(),
  };
  const className = useClassName();

  const set = (field: keyof BackupTargetInput) => (event: React.ChangeEvent<HTMLInputElement>) =>
    setDraft((d) => ({ ...d, [field]: event.target.value }));

  const target = view.target;
  if (!editing && target !== null) {
    return (
      <PaneSection data-testid="backup-target">
        <GroupTitle>{t("panes.backup.target.title")}</GroupTitle>
        <Group>
          <Row>
            <RowText title={t("panes.backup.target.endpoint")} />
            <RowValue className="truncate select-all">{target.endpoint}</RowValue>
          </Row>
          <Row>
            <RowText title={t("panes.backup.target.bucket")} />
            <RowValue className="truncate select-all">
              {target.prefix.length > 0 ? `${target.bucket}/${target.prefix}` : target.bucket}
            </RowValue>
          </Row>
          <Row>
            <RowText title={t("panes.backup.target.class")} />
            <RowValue data-testid="backup-class">{className(classOf(target))}</RowValue>
          </Row>
          <Row last>
            <RowText title={t("panes.backup.target.key")} />
            <div className="flex items-center gap-2">
              <RowValue className="truncate">{target.accessKeyId}</RowValue>
              <Button
                size="sm"
                variant="secondary"
                disabled={disabled}
                onClick={() => {
                  setDraft(draftOf(target));
                  setEditing(true);
                }}
              >
                {t("panes.backup.target.change")}
              </Button>
            </div>
          </Row>
        </Group>
        <GroupCaption>{t("panes.backup.target.caption")}</GroupCaption>
      </PaneSection>
    );
  }

  const keepsSecret = target?.hasSecret === true;
  const secretMissing = !keepsSecret && (draft.secretAccessKey ?? "").length === 0;
  const incomplete =
    draft.endpoint.trim().length === 0 ||
    draft.bucket.trim().length === 0 ||
    draft.accessKeyId.trim().length === 0 ||
    secretMissing;
  const aws = AWS_ENDPOINT.test(draft.endpoint.trim());
  // Off AWS the choice is Standard whatever was picked before the address
  // changed, so the box is never sent a class it would refuse.
  const chosen: StorageClass = aws ? (draft.storageClass ?? "standard") : "standard";
  const classDetail = aws
    ? t(CLASSES.find((c) => c.id === chosen)?.detail ?? "panes.backup.target.classStandardDetail")
    : t("panes.backup.target.classAwsOnly");

  const submit = async () => {
    const secret = (draft.secretAccessKey ?? "").trim();
    const input: BackupTargetInput = {
      endpoint: draft.endpoint.trim().replace(/\/+$/, ""),
      bucket: draft.bucket.trim(),
      prefix: draft.prefix.trim().replace(/^\/+|\/+$/g, ""),
      region: draft.region.trim(),
      accessKeyId: draft.accessKeyId.trim(),
      ...(secret.length > 0 ? { secretAccessKey: secret } : {}),
      storageClass: chosen,
    };
    if (await backup.saveTarget(input)) setEditing(false);
  };

  return (
    <PaneSection data-testid="backup-target-form">
      <GroupTitle>{t("panes.backup.target.title")}</GroupTitle>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          void submit();
        }}
      >
        <Group>
          <FieldRow id={ids.endpoint} title={t("panes.backup.target.endpoint")}>
            <MonoInput
              id={ids.endpoint}
              value={draft.endpoint}
              placeholder="https://s3.eu-central-1.amazonaws.com"
              inputMode="url"
              maxLength={255}
              autoComplete="off"
              disabled={disabled}
              onChange={set("endpoint")}
            />
          </FieldRow>
          <FieldRow id={ids.bucket} title={t("panes.backup.target.bucket")}>
            <MonoInput
              id={ids.bucket}
              value={draft.bucket}
              placeholder="my-box-backups"
              maxLength={63}
              autoComplete="off"
              disabled={disabled}
              onChange={set("bucket")}
            />
          </FieldRow>
          <FieldRow
            id={ids.prefix}
            title={t("panes.backup.target.prefix")}
            detail={t("panes.backup.target.optional")}
          >
            <MonoInput
              id={ids.prefix}
              value={draft.prefix}
              placeholder="losos"
              maxLength={200}
              autoComplete="off"
              disabled={disabled}
              onChange={set("prefix")}
            />
          </FieldRow>
          <FieldRow
            id={ids.region}
            title={t("panes.backup.target.region")}
            detail={t("panes.backup.target.optional")}
          >
            <MonoInput
              id={ids.region}
              value={draft.region}
              placeholder="eu-central-1"
              maxLength={40}
              autoComplete="off"
              disabled={disabled}
              onChange={set("region")}
            />
          </FieldRow>
          <FieldRow id={ids.key} title={t("panes.backup.target.key")}>
            <MonoInput
              id={ids.key}
              value={draft.accessKeyId}
              maxLength={128}
              autoComplete="off"
              disabled={disabled}
              onChange={set("accessKeyId")}
            />
          </FieldRow>
          <FieldRow
            id={ids.secret}
            title={t("panes.backup.target.secret")}
            detail={keepsSecret ? t("panes.backup.target.secretKept") : undefined}
          >
            <Input
              id={ids.secret}
              type="password"
              value={draft.secretAccessKey ?? ""}
              maxLength={128}
              autoComplete="new-password"
              placeholder={keepsSecret ? "••••••••" : ""}
              disabled={disabled}
              onChange={set("secretAccessKey")}
            />
          </FieldRow>
          <FieldRow id={ids.class} title={t("panes.backup.target.class")} detail={classDetail}>
            <NativeSelect
              id={ids.class}
              className="w-full"
              value={chosen}
              disabled={disabled || !aws}
              data-testid="backup-class-select"
              onChange={(event) =>
                setDraft((d) => ({ ...d, storageClass: event.target.value as StorageClass }))
              }
            >
              {CLASSES.map((c) => (
                <NativeSelectOption key={c.id} value={c.id}>
                  {c.name ?? t("panes.backup.target.classStandard")}
                </NativeSelectOption>
              ))}
            </NativeSelect>
          </FieldRow>
          <Row last className="justify-end">
            {target !== null && (
              <>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  disabled={disabled}
                  onClick={() => void backup.forgetTarget()}
                >
                  {t("panes.backup.target.forget")}
                </Button>
                <Button
                  type="button"
                  variant="secondary"
                  size="sm"
                  disabled={disabled}
                  onClick={() => setEditing(false)}
                >
                  {t("panes.backup.target.cancel")}
                </Button>
              </>
            )}
            <Button type="submit" size="sm" disabled={disabled || incomplete}>
              {t("panes.backup.target.save")}
            </Button>
          </Row>
        </Group>
      </form>
      <GroupCaption>{t("panes.backup.target.caption")}</GroupCaption>
    </PaneSection>
  );
}

function FieldRow({
  id,
  title,
  detail,
  children,
}: {
  id: string;
  title: string;
  detail?: string | undefined;
  children: React.ReactNode;
}) {
  return (
    <Row>
      <RowText htmlFor={id} title={title} detail={detail} />
      <div className="w-[16rem] max-sm:w-[11rem]">{children}</div>
    </Row>
  );
}

// ── The key ─────────────────────────────────────────────────────────────

/* The recovery code is the backups' password, so the owner has to have it
 * somewhere that is not this box. It is fetched only when asked for, so it
 * is not on the screen for whoever looks over a shoulder. */
function CodeSection({ locked }: { locked: boolean }) {
  const t = useT();
  const [code, setCode] = React.useState<string | null>(null);
  const [failed, setFailed] = React.useState(false);

  const show = async () => {
    try {
      setCode((await getRecoveryCode()).code);
      setFailed(false);
    } catch (error) {
      if (!isUnauthorized(error)) setFailed(true);
    }
  };

  return (
    <PaneSection data-testid="backup-code">
      <GroupTitle>{t("panes.backup.code.title")}</GroupTitle>
      <Group>
        <Row last>
          <RowText
            title={t("panes.backup.code.row")}
            detail={failed ? t("panes.backup.code.failed") : t("panes.backup.code.detail")}
          />
          {code === null ? (
            <Button size="sm" variant="secondary" disabled={locked} onClick={() => void show()}>
              {t("panes.backup.code.show")}
            </Button>
          ) : (
            <div className="flex items-center gap-2">
              <RowValue className="text-ink select-all" data-testid="recovery-code">
                {code}
              </RowValue>
              <Button size="sm" variant="ghost" onClick={() => setCode(null)}>
                {t("panes.backup.code.hide")}
              </Button>
            </div>
          )}
        </Row>
      </Group>
      <GroupCaption>{t("panes.backup.code.caption")}</GroupCaption>
    </PaneSection>
  );
}

// ── Backing up ──────────────────────────────────────────────────────────

/* Reads the language on screen, so a switch of language re-renders it. */
function useWhen(): (seconds: number) => string {
  useLocale();
  return (seconds) =>
    new Intl.DateTimeFormat(intlTag(), { dateStyle: "medium", timeStyle: "short" }).format(
      new Date(seconds * 1000),
    );
}

function BackupsSection({
  view,
  backup,
  disabled,
}: {
  view: BackupResponse;
  backup: BackupData;
  disabled: boolean;
}) {
  const t = useT();
  const when = useWhen();
  const job = view.job?.kind === "backup" ? view.job : null;
  const last = view.last;

  return (
    <PaneSection data-testid="backup-runs">
      <GroupTitle>{t("panes.backup.runs.title")}</GroupTitle>
      <Group>
        <Row>
          <RowText
            title={t("panes.backup.runs.last")}
            detail={
              last === null
                ? t("panes.backup.runs.never")
                : t("panes.backup.runs.lastDetail", {
                    size: formatBytes(last.bytes),
                    count: last.files,
                  })
            }
          />
          {last !== null && <RowValue>{when(last.time)}</RowValue>}
        </Row>
        {job?.state === "running" && (
          <Row data-testid="backup-running">
            <RowText title={t("panes.backup.runs.running")} detail={t("panes.backup.runs.runningDetail")} />
            <Spinner size={16} />
          </Row>
        )}
        {job?.state === "failed" && (
          <StackRow data-testid="backup-failed">
            <p className="text-sm text-crit">{t("panes.backup.runs.failed")}</p>
            {job.message.length > 0 && (
              <p className="text-[12.5px] leading-snug text-muted">{job.message}</p>
            )}
          </StackRow>
        )}
        <Row last>
          <RowText
            title={t("panes.backup.runs.now")}
            detail={view.target === null ? t("panes.backup.runs.needsTarget") : undefined}
          />
          <Button
            size="sm"
            disabled={disabled || view.target === null}
            onClick={() => void backup.backUp()}
          >
            {t("panes.backup.runs.button")}
          </Button>
        </Row>
      </Group>
      <GroupCaption>{t("panes.backup.runs.caption")}</GroupCaption>
    </PaneSection>
  );
}

// ── Restoring ───────────────────────────────────────────────────────────

const CODE_SHAPE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;

function RestoreSection({
  view,
  backup,
  disabled,
}: {
  view: BackupResponse;
  backup: BackupData;
  disabled: boolean;
}) {
  const t = useT();
  const when = useWhen();
  const [code, setCode] = React.useState("");
  const [confirming, setConfirming] = React.useState(false);
  const inputId = React.useId();
  const titleId = React.useId();
  const bodyId = React.useId();
  const job = view.job?.kind === "restore" ? view.job : null;
  const thaw = needsThaw(view.target);
  const shaped = CODE_SHAPE.test(code.trim().toLowerCase());

  return (
    <PaneSection data-testid="backup-restore">
      <GroupTitle>{t("panes.backup.restore.title")}</GroupTitle>
      <Group>
        {job?.state === "running" && (
          <Row data-testid="restore-running">
            <RowText
              title={t("panes.backup.restore.running")}
              detail={t(thaw ? "panes.backup.restore.runningThaw" : "panes.backup.restore.runningDetail")}
            />
            <Spinner size={16} />
          </Row>
        )}
        {job?.state === "done" && job.finishedAt !== undefined && (
          <Row data-testid="restore-done">
            <RowText
              title={t("panes.backup.restore.done")}
              detail={t("panes.backup.restore.doneDetail")}
            />
            <RowValue>{when(job.finishedAt)}</RowValue>
          </Row>
        )}
        {job?.state === "failed" && (
          <StackRow data-testid="restore-failed">
            <p className="text-sm text-crit">{t("panes.backup.restore.failed")}</p>
            {job.message.length > 0 && (
              <p className="text-[12.5px] leading-snug text-muted">{job.message}</p>
            )}
          </StackRow>
        )}
        <Row last>
          <RowText htmlFor={inputId} title={t("panes.backup.restore.code")} />
          <form
            className="flex items-center gap-2"
            onSubmit={(event) => {
              event.preventDefault();
              if (shaped) setConfirming(true);
            }}
          >
            <MonoInput
              id={inputId}
              value={code}
              placeholder="0f8fad5b-d9cb-469f-a165-70867728950e"
              maxLength={40}
              autoComplete="off"
              disabled={disabled || view.target === null}
              className="w-[23rem] max-sm:w-[11rem]"
              onChange={(event) => setCode(event.target.value)}
            />
            <Button
              type="submit"
              size="sm"
              variant="secondary"
              disabled={disabled || view.target === null || !shaped}
            >
              {t("panes.backup.restore.button")}
            </Button>
          </form>
        </Row>
      </Group>
      <GroupCaption>{t("panes.backup.restore.caption")}</GroupCaption>

      <Dialog open={confirming} onOpenChange={setConfirming} labelledBy={titleId} describedBy={bodyId}>
        <DialogHeader>
          <DialogTitle id={titleId} className="flex items-center gap-2">
            <HugeiconsIcon
              icon={Alert02Icon}
              size={19}
              strokeWidth={1.5}
              color="currentColor"
              className="text-warn"
              aria-hidden="true"
            />
            {t("panes.backup.restore.dialogTitle")}
          </DialogTitle>
          <DialogDescription id={bodyId}>{t("panes.backup.restore.dialogBody")}</DialogDescription>
        </DialogHeader>
        <DialogBody>
          <ul className="flex flex-col gap-1.5 text-[13px] leading-snug text-muted">
            <li className="flex gap-2">
              <span aria-hidden="true" className="mt-2 size-1.5 shrink-0 rounded-full bg-warn" />
              {t("panes.backup.restore.replaces")}
            </li>
            {thaw && (
              <li className="flex gap-2" data-testid="restore-thaw">
                <span aria-hidden="true" className="mt-2 size-1.5 shrink-0 rounded-full bg-warn" />
                {t("panes.backup.restore.thaw")}
              </li>
            )}
            <li className="flex gap-2">
              <span aria-hidden="true" className="mt-2 size-1.5 shrink-0 rounded-full bg-faint" />
              {t("panes.backup.restore.codeStays")}
            </li>
            <li className="flex gap-2">
              <span aria-hidden="true" className="mt-2 size-1.5 shrink-0 rounded-full bg-faint" />
              {t("panes.backup.restore.password")}
            </li>
          </ul>
        </DialogBody>
        <DialogFooter>
          <Button variant="ghost" onClick={() => setConfirming(false)}>
            {t("panes.backup.restore.keep")}
          </Button>
          <Button
            onClick={() => {
              setConfirming(false);
              void backup.restore(code.trim()).then((ok) => {
                if (ok) setCode("");
              });
            }}
          >
            {t("panes.backup.restore.confirm")}
          </Button>
        </DialogFooter>
      </Dialog>
    </PaneSection>
  );
}
