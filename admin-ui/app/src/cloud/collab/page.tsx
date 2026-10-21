import * as React from "react";
import { Link, useParams, useSearchParams } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import { ArrowLeft01Icon, Download01Icon, Share08Icon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge, StatusDot, type DotState } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "@/components/ui/empty";
import { Spinner } from "@/components/ui/spinner";
import { getProfile, type Profile } from "@/lib/api";
import { getLocale, type MessageKey } from "@/lib/i18n";
import { useLocale, useT } from "@/lib/i18n-react";
import { CloudError, isAbort } from "../api";
import { MeshShareDialog } from "../mesh/share-dialog";
import { collab, mesh, type DocMeta } from "./api";
import { CollabEditor, type Connection, type Edited, type Presence } from "./editor";
import { Faces } from "./faces";
import { accentFor, boxOf, nameOf } from "./people";

/* /cloud/edit/<file>[?share=<id>]: a text file edited together.
 *
 * Laid out like a note rather than an IDE: the name, where the file lives,
 * who is in it and whether the line to them is up, then the text. A file
 * from another box says which box holds it; when that box is out of reach
 * the page says so and keeps the typing, which merges when it is back. */

function errorKey(err: unknown): MessageKey {
  if (err instanceof CloudError) {
    switch (err.code) {
      case "notText":
      case "folder":
        return "collab.error.notText";
      case "tooLarge":
        return "collab.error.tooLarge";
      case "unreachable":
      case "timeout":
      case "closed":
        return "collab.error.unreachable";
      default:
        return "collab.error.notFound";
    }
  }
  return "collab.error.notFound";
}

/** "just now", "3 minutes ago", in the reader's language. */
function when(at: number, now: number): string {
  const seconds = Math.round((at - now) / 1000);
  if (seconds > -45) return "";
  const rtf = new Intl.RelativeTimeFormat(getLocale(), { numeric: "auto" });
  const minutes = Math.round(seconds / 60);
  if (minutes > -60) return rtf.format(minutes, "minute");
  const hours = Math.round(minutes / 60);
  if (hours > -24) return rtf.format(hours, "hour");
  return rtf.format(Math.round(hours / 24), "day");
}

export function EditPage() {
  const t = useT();
  useLocale();
  const params = useParams();
  const [search] = useSearchParams();
  const file = Number(params["file"]);
  const share = search.get("share") ?? undefined;
  const [meta, setMeta] = React.useState<DocMeta | null>(null);
  const [error, setError] = React.useState<unknown>(null);
  const [connection, setConnection] = React.useState<Connection>("connecting");
  const [people, setPeople] = React.useState<Presence[]>([]);
  const [edited, setEdited] = React.useState<Edited | null>(null);
  const [sharing, setSharing] = React.useState(false);
  const [now, setNow] = React.useState(() => Date.now());

  React.useEffect(() => {
    const controller = new AbortController();
    setMeta(null);
    setError(null);
    collab.doc(file, share, controller.signal).then(setMeta, (err: unknown) => {
      if (!isAbort(err)) setError(err);
    });
    return () => controller.abort();
  }, [file, share]);

  // Whether the owner's box is reachable from here changes on its own.
  React.useEffect(() => {
    if (meta === null || !meta.remote) return;
    const timer = window.setInterval(() => {
      collab.doc(file, share).then(setMeta, () => {});
    }, 8000);
    return () => window.clearInterval(timer);
  }, [meta?.remote, file, share]);

  React.useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 20_000);
    return () => window.clearInterval(timer);
  }, []);

  React.useEffect(() => {
    if (meta !== null) document.title = `${meta.name} · LosOS Cloud`;
  }, [meta]);

  // This person's colour and picture from their profile on this box; until
  // it answers, the colour their name gives.
  const login = meta === null ? null : nameOf(meta.me.address);
  const [profile, setProfile] = React.useState<Profile | null>(null);
  React.useEffect(() => {
    if (login === null) return;
    const ac = new AbortController();
    getProfile(login, { signal: ac.signal }).then(setProfile, () => {});
    return () => ac.abort();
  }, [login]);

  const me = React.useMemo(
    () =>
      meta === null
        ? null
        : {
            ...meta.me,
            accent: profile?.accent ?? accentFor(nameOf(meta.me.address)),
            picture: profile?.picture ?? null,
          },
    [meta?.me.name, meta?.me.address, profile?.accent, profile?.picture],
  );

  if (!Number.isSafeInteger(file) || error !== null) {
    const box = meta?.box ?? "";
    return (
      <Empty className="mx-auto mt-6 min-h-72 max-w-xl">
        <EmptyHeader>
          <EmptyTitle>{t("collab.error.title")}</EmptyTitle>
          <EmptyDescription>{t(errorKey(error), { box: box || t("collab.status.offline") })}</EmptyDescription>
        </EmptyHeader>
        <Button variant="outline" onClick={() => history.back()}>
          <HugeiconsIcon icon={ArrowLeft01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          {share === undefined ? t("collab.back.files") : t("collab.back.shared")}
        </Button>
      </Empty>
    );
  }

  if (meta === null || me === null) {
    return (
      <div className="flex min-h-72 items-center justify-center gap-2 text-[13px] text-muted" role="status">
        <Spinner size={16} />
        {t("collab.loading")}
      </div>
    );
  }

  const readOnly = meta.perm !== "edit";
  const here = boxOf(meta.me.address);
  const apart = meta.remote && meta.linked === false;
  const state: { dot: DotState; label: string } =
    connection === "offline"
      ? { dot: "warn", label: t("collab.status.offline") }
      : connection === "connecting"
        ? { dot: "pending", label: t("collab.status.connecting") }
        : apart
          ? { dot: "warn", label: t("collab.status.apart", { box: meta.box }) }
          : { dot: "ok", label: t("collab.status.live") };
  const ago = edited === null ? "" : when(edited.at, now) || t("collab.when.now");
  const owner = meta.remote ? meta.owner : "";

  return (
    <div className="mx-auto flex w-full max-w-4xl flex-col gap-4" data-collab-page>
      <Link
        to={meta.remote ? "/shared" : "/files"}
        className="inline-flex w-fit items-center gap-1 rounded-control text-[13px] text-muted hover:text-ink focus-visible:outline-2 focus-visible:outline-accent"
      >
        <HugeiconsIcon icon={ArrowLeft01Icon} size={14} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        {meta.remote ? t("collab.back.shared") : t("collab.back.files")}
      </Link>

      <header className="flex flex-wrap items-start justify-between gap-x-6 gap-y-3">
        <div className="flex min-w-0 flex-col gap-1">
          <h1 className="truncate text-xl font-semibold tracking-tight" data-doc-name>
            {meta.name}
          </h1>
          <p className="text-[13px] text-muted">
            {meta.remote ? t("collab.where.remote", { owner, box: meta.box }) : t("collab.where.here", { box: meta.box })}
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-3">
          <Faces people={people} me={meta.me.address} />
          <Badge variant={state.dot === "ok" ? "ok" : state.dot === "warn" ? "warn" : "neutral"} data-connection={connection}>
            <StatusDot state={state.dot} />
            <span role="status">{state.label}</span>
          </Badge>
          {readOnly && <Badge variant="outline">{t("collab.status.readOnly")}</Badge>}
          {meta.remote && share !== undefined && (
            <Button size="sm" variant="ghost" onClick={() => window.location.assign(mesh.fileUrl(share, file))}>
              <HugeiconsIcon icon={Download01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              <span className="max-sm:sr-only">{t("collab.download")}</span>
            </Button>
          )}
          {!meta.remote && !readOnly && (
            <Button size="sm" variant="secondary" onClick={() => setSharing(true)} data-share-button>
              <HugeiconsIcon icon={Share08Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
              {t("collab.share")}
            </Button>
          )}
        </div>
      </header>

      {connection === "offline" ? (
        <Alert variant="warn">
          <AlertDescription>{t("collab.hint.offline")}</AlertDescription>
        </Alert>
      ) : apart ? (
        <Alert variant="warn" data-apart>
          <AlertDescription>{t("collab.hint.apart", { box: meta.box, here })}</AlertDescription>
        </Alert>
      ) : readOnly ? (
        <Alert>
          <AlertDescription>{t("collab.hint.readOnly", { owner: owner || meta.owner })}</AlertDescription>
        </Alert>
      ) : null}

      <CollabEditor
        key={`${file}:${share ?? ""}`}
        file={file}
        {...(share !== undefined ? { share } : {})}
        me={me}
        readOnly={readOnly}
        label={t("collab.label", { name: meta.name })}
        placeholderText={t("collab.placeholder")}
        onConnection={setConnection}
        onPresence={setPeople}
        onEdited={setEdited}
      />

      <p className="min-h-5 text-[12.5px] text-muted" data-edited>
        {edited !== null && t("collab.edited", { name: edited.name, box: boxOf(edited.address), when: ago })}
      </p>

      {!meta.remote && (
        <MeshShareDialog open={sharing} onOpenChange={setSharing} fileId={file} name={meta.name} />
      )}
    </div>
  );
}
