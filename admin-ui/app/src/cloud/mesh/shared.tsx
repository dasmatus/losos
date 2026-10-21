import * as React from "react";
import { Link, useNavigate, useParams, useSearchParams } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import { ArrowLeft01Icon, Download01Icon, Folder01Icon, UserSharingIcon } from "@hugeicons/core-free-icons";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Spinner } from "@/components/ui/spinner";
import { toast } from "@/components/ui/toast";
import { useT } from "@/lib/i18n-react";
import { CloudError, isAbort } from "../api";
import { editable, editHref, mesh, type Incoming, type RemoteEntry } from "../collab/api";
import { PersonAvatar } from "@/components/person";
import { accentFor, nameOf } from "../collab/people";
import { formatBytes, formatWhen, iconFor } from "../format";
import { meshError } from "./share-dialog";

/* /cloud/shared: what people on other boxes shared with this person, and
 * /cloud/shared/<share>?folder=<id> to look inside a shared folder. The
 * files stay on the owner's box; a text file opens in the editor here as a
 * copy kept in step with theirs, anything else downloads through this box. */

function mimeOf(name: string): string {
  const ext = name.slice(name.lastIndexOf(".") + 1).toLowerCase();
  const known: Record<string, string> = {
    md: "text/markdown",
    txt: "text/plain",
    csv: "text/csv",
    pdf: "application/pdf",
    png: "image/png",
    jpg: "image/jpeg",
    jpeg: "image/jpeg",
    odt: "application/vnd.oasis.opendocument.text",
    docx: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    xlsx: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    ods: "application/vnd.oasis.opendocument.spreadsheet",
  };
  return known[ext] ?? "application/octet-stream";
}

function Owner({ s }: { s: Incoming }) {
  const name = nameOf(s.ownerRemote);
  return (
    <PersonAvatar
      aria-hidden="true"
      className="size-9 text-[12.5px]"
      person={{ name, displayName: s.ownerName || name, accent: accentFor(name) }}
    />
  );
}

export function SharedView() {
  const t = useT();
  const navigate = useNavigate();
  const [items, setItems] = React.useState<Incoming[] | null>(null);
  const [me, setMe] = React.useState<string>("");
  const [error, setError] = React.useState<string | null>(null);
  const [off, setOff] = React.useState(false);
  const [tick, setTick] = React.useState(0);

  React.useEffect(() => {
    const controller = new AbortController();
    mesh.incoming(controller.signal).then(
      (all) => {
        setItems(all);
        setError(null);
      },
      (err: unknown) => {
        if (isAbort(err)) return;
        setItems([]);
        // The box does not talk to other boxes.
        if (err instanceof CloudError && err.status === 404) setOff(true);
        else setError(t(meshError(err) ?? "collab.error.notFound"));
      },
    );
    mesh.box(controller.signal).then((b) => setMe(b.me), () => {});
    return () => controller.abort();
  }, [tick, t]);

  // New offers arrive while the page is open.
  React.useEffect(() => {
    if (off) return;
    const timer = window.setInterval(() => setTick((n) => n + 1), 5000);
    return () => window.clearInterval(timer);
  }, [off]);

  const answer = (s: Incoming, accept: boolean) => {
    (accept ? mesh.accept(s.id) : mesh.decline(s.id)).then(
      () => {
        if (accept) toast.done(t("mesh.shared.accepted", { name: s.name }));
        setTick((n) => n + 1);
      },
      (err: unknown) => {
        toast.error(t("mesh.shared.failed"), t(meshError(err) ?? "mesh.error.unreachable"), { help: "index" });
      },
    );
  };

  const open = (s: Incoming) => {
    if (s.folder) navigate(`/shared/${encodeURIComponent(s.id)}`);
    else window.location.assign(editable(s.name) ? editHref(s.fileId, s.id) : mesh.fileUrl(s.id, s.fileId));
  };

  const shown = (items ?? []).filter((s) => s.state !== "declined");

  return (
    <div className="flex flex-col gap-4" data-shared-view>
      <div className="flex flex-col gap-1">
        <h1 className="min-h-9 text-[15px] leading-9 font-semibold tracking-tight">{t("mesh.shared.title")}</h1>
        <p className="max-w-2xl text-[13px] text-muted">{t("mesh.shared.lead")}</p>
      </div>
      {error !== null && (
        <Alert variant="warn">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}
      {items === null ? (
        <div className="grid min-h-60 place-items-center">
          <Spinner />
        </div>
      ) : shown.length === 0 ? (
        <Empty className="min-h-72">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <HugeiconsIcon icon={UserSharingIcon} size={22} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            </EmptyMedia>
            <EmptyTitle>{t(off ? "mesh.shared.off" : "mesh.shared.empty.title")}</EmptyTitle>
            <EmptyDescription>
              {off ? t("mesh.share.off") : t("mesh.shared.empty.body", { address: me || "name@box" })}
            </EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : (
        <ul className="flex flex-col divide-y divide-hair rounded-card border border-line bg-surface" data-incoming>
          {shown.map((s) => (
            <li key={`${s.remote}/${s.id}`} className="flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3" data-incoming-share={s.name}>
              <Owner s={s} />
              <div className="flex min-w-0 flex-1 flex-col gap-0.5">
                <span className="flex min-w-0 items-center gap-2">
                  <HugeiconsIcon
                    icon={iconFor({ dir: s.folder, mime: mimeOf(s.name), name: s.name })}
                    size={16}
                    strokeWidth={1.5}
                    color="currentColor"
                    className={s.folder ? "text-accent" : "text-muted"}
                    aria-hidden="true"
                  />
                  {s.state === "accepted" ? (
                    <a
                      href={s.folder ? `/cloud/shared/${encodeURIComponent(s.id)}` : undefined}
                      onClick={(event) => {
                        event.preventDefault();
                        open(s);
                      }}
                      className="truncate text-[14px] font-medium hover:underline focus-visible:outline-2 focus-visible:outline-accent"
                      role={s.folder ? undefined : "link"}
                      tabIndex={0}
                    >
                      {s.name}
                    </a>
                  ) : (
                    <span className="truncate text-[14px] font-medium">{s.name}</span>
                  )}
                  {s.state === "pending" && <Badge variant="local">{t("mesh.shared.new")}</Badge>}
                  <Badge variant="outline">{t(s.perm === "edit" ? "mesh.shared.canEdit" : "mesh.shared.viewOnly")}</Badge>
                  {s.unsynced && <Badge variant="warn">{t("mesh.shared.unsynced", { box: s.remote })}</Badge>}
                </span>
                <span className="truncate text-[12.5px] text-muted">
                  {t("mesh.shared.from", { name: s.ownerName, address: s.ownerRemote })}
                  {s.folder ? "" : ` · ${formatBytes(s.size)}`}
                  {` · ${formatWhen(s.created)}`}
                </span>
              </div>
              {s.state === "pending" ? (
                <div className="flex items-center gap-2">
                  <Button size="sm" variant="ghost" onClick={() => answer(s, false)}>
                    {t("mesh.shared.decline")}
                  </Button>
                  <Button size="sm" onClick={() => answer(s, true)} data-accept={s.name}>
                    {t("mesh.shared.accept")}
                  </Button>
                </div>
              ) : (
                <Button size="sm" variant="secondary" onClick={() => open(s)} data-open={s.name}>
                  {s.folder || editable(s.name) ? t("mesh.shared.open") : t("collab.download")}
                </Button>
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

/** Inside a shared folder, read through the owner's box. */
export function SharedFolderView() {
  const t = useT();
  const params = useParams();
  const [search] = useSearchParams();
  const share = params["share"] ?? "";
  const folderParam = search.get("folder");
  const folder = folderParam === null ? undefined : Number(folderParam);
  const [entries, setEntries] = React.useState<RemoteEntry[] | null>(null);
  const [info, setInfo] = React.useState<Incoming | null>(null);
  const [error, setError] = React.useState<string | null>(null);

  React.useEffect(() => {
    const controller = new AbortController();
    setEntries(null);
    setError(null);
    mesh.incoming(controller.signal).then((all) => setInfo(all.find((s) => s.id === share) ?? null), () => {});
    mesh.list(share, folder, controller.signal).then(setEntries, (err: unknown) => {
      if (isAbort(err)) return;
      setEntries([]);
      setError(t(meshError(err) ?? "collab.error.notFound"));
    });
    return () => controller.abort();
  }, [share, folder, t]);

  return (
    <div className="flex flex-col gap-4" data-shared-folder>
      <Link
        to="/shared"
        className="inline-flex w-fit items-center gap-1 rounded-control text-[13px] text-muted hover:text-ink focus-visible:outline-2 focus-visible:outline-accent"
      >
        <HugeiconsIcon icon={ArrowLeft01Icon} size={14} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        {t("mesh.shared.back")}
      </Link>
      <div className="flex flex-col gap-1">
        <h1 className="flex items-center gap-2 text-[15px] leading-9 font-semibold tracking-tight">
          <HugeiconsIcon icon={Folder01Icon} size={18} strokeWidth={1.5} color="currentColor" className="text-accent" aria-hidden="true" />
          {info?.name ?? ""}
        </h1>
        {info !== null && (
          <p className="text-[12.5px] text-muted">{t("mesh.shared.from", { name: info.ownerName, address: info.ownerRemote })}</p>
        )}
      </div>
      {error !== null ? (
        <Alert variant="warn">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : entries === null ? (
        <div className="grid min-h-60 place-items-center">
          <Spinner />
        </div>
      ) : entries.length === 0 ? (
        <p className="text-[13px] text-faint">{t("mesh.shared.emptyFolder")}</p>
      ) : (
        <ul className="flex flex-col divide-y divide-hair rounded-card border border-line bg-surface">
          {entries.map((e) => {
            const href = e.folder
              ? `/cloud/shared/${encodeURIComponent(share)}?folder=${e.id}`
              : editable(e.name)
                ? editHref(e.id, share)
                : mesh.fileUrl(share, e.id);
            return (
              <li key={e.id} className="flex items-center gap-3 px-4 py-2.5">
                <HugeiconsIcon
                  icon={iconFor({ dir: e.folder, mime: mimeOf(e.name), name: e.name })}
                  size={20}
                  strokeWidth={1.5}
                  color="currentColor"
                  className={e.folder ? "text-accent" : "text-muted"}
                  aria-hidden="true"
                />
                {e.folder ? (
                  <Link to={href.replace(/^\/cloud/, "")} className="min-w-0 flex-1 truncate text-[14px] hover:underline">
                    {e.name}
                  </Link>
                ) : (
                  <a href={href} className="min-w-0 flex-1 truncate text-[14px] hover:underline" data-remote-file={e.name}>
                    {e.name}
                  </a>
                )}
                {!e.folder && <span className="text-[12.5px] text-muted tabular-nums">{formatBytes(e.size)}</span>}
                {!e.folder && (
                  <Button
                    size="icon-sm"
                    variant="ghost"
                    aria-label={t("collab.download")}
                    title={t("collab.download")}
                    onClick={() => window.location.assign(mesh.fileUrl(share, e.id))}
                  >
                    <HugeiconsIcon icon={Download01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                  </Button>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
