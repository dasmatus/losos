/* The Office editor: Collabora Online in a frame that fills its parent,
 * under two LosOS bars. The first has the way back, the document's name,
 * whether it is saved, who else is in it, the zoom, PDF and Save; the second
 * is the formatting bar (toolbar.tsx). Collabora's own bars are hidden by
 * the stylesheet the box serves it (admin-ui/themes/office/branding.css),
 * so the document sits in the same shell, type and buttons as every other
 * app. The shell's top bar stays above it, so the logo and the person are
 * not repeated.
 *
 * Collabora talks to the page that frames it with postMessage, one JSON
 * object per message ({MessageId, Values}). It waits for Host_PostmessageReady
 * before it sends anything, says App_LoadingStatus/Document_Loaded when the
 * document is up, and UI_Close when its own close button is pressed. Messages
 * are taken only from the frame's own window and this origin: the editor is
 * on /browser/ of this same box.
 *
 * LosOS Cloud mounts <OfficeEditor> at /cloud/office/:fileId; Documents
 * (src/screens/Office.tsx) shows it for a plain folder and for a document
 * shared from another box. */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import {
  ArrowLeft01Icon,
  FloppyDiskIcon,
  MinusSignIcon,
  Pdf01Icon,
  PlusSignIcon,
} from "@hugeicons/core-free-icons";
import { PersonAvatar } from "@/components/person";
import { followFrameKeys } from "@/palette/host";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useLocale, useT } from "@/lib/i18n-react";
import { getResolvedTheme } from "@/lib/theme";
import { editorSrc, httpsAddress, matchEditorTheme, OfficeFailure, type OfficeSession } from "./api";
import { useDocument, type Viewer } from "./document";
import { docKind, OfficeToolbar, watchedCommands } from "./toolbar";

export interface OfficeEditorProps {
  /** Starts the session: the cloud's open call, or one minted elsewhere. */
  load: () => Promise<OfficeSession>;
  /** Where "Back" and Collabora's close button go. */
  onClose: () => void;
  backLabel?: string;
}

type State =
  | { kind: "opening" }
  | { kind: "failed"; error: OfficeFailure }
  | { kind: "open"; session: OfficeSession; loaded: boolean };

interface CoolMessage {
  MessageId?: string;
  Values?: unknown;
}

export function OfficeEditor({ load, onClose, backLabel }: OfficeEditorProps) {
  const t = useT();
  const locale = useLocale();
  const frame = useRef<HTMLIFrameElement>(null);
  const [state, setState] = useState<State>({ kind: "opening" });
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let live = true;
    setState({ kind: "opening" });
    load().then(
      (session) => {
        if (!live) return;
        const https = httpsAddress(session, window.location);
        if (https !== null) {
          window.location.replace(https);
          return;
        }
        matchEditorTheme(getResolvedTheme() === "dark");
        setState({ kind: "open", session, loaded: false });
      },
      (e: unknown) => {
        if (live) {
          setState({
            kind: "failed",
            error: e instanceof OfficeFailure ? e : new OfficeFailure("other"),
          });
        }
      },
    );
    return () => {
      live = false;
    };
  }, [load, attempt]);

  const post = useCallback((message: object) => {
    frame.current?.contentWindow?.postMessage(JSON.stringify(message), window.location.origin);
  }, []);

  useEffect(() => {
    const onMessage = (event: MessageEvent) => {
      if (event.origin !== window.location.origin) return;
      if (event.source !== frame.current?.contentWindow) return;
      let msg: CoolMessage;
      try {
        msg = (typeof event.data === "string" ? JSON.parse(event.data) : event.data) as CoolMessage;
      } catch {
        return;
      }
      const status = (msg.Values as { Status?: string } | undefined)?.Status;
      if (msg.MessageId === "App_LoadingStatus") {
        if (status === "Frame_Ready") post({ MessageId: "Host_PostmessageReady" });
        if (status === "Document_Loaded") {
          setState((s) => (s.kind === "open" ? { ...s, loaded: true } : s));
        }
      } else {
        doc.onMessage(msg);
      }
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  });

  const name = state.kind === "open" ? state.session.name : "";
  useEffect(() => {
    document.title = name ? `${name} · LosOS` : "LosOS";
  }, [name]);

  const kind = docKind(name);
  const watched = useMemo(() => watchedCommands(kind), [kind]);
  const loaded = state.kind === "open" && state.loaded;
  const doc = useDocument(frame, loaded, watched, post);

  // Ctrl/Cmd+Shift+P opens the palette while the document has the keys.
  const open = state.kind === "open";
  useEffect(() => {
    const element = frame.current;
    return open && element !== null ? followFrameKeys(element) : undefined;
  }, [open]);
  const writable = state.kind === "open" && state.session.writable;
  const busy = state.kind === "opening" || (state.kind === "open" && !state.loaded);

  return (
    <div className="flex h-full min-h-0 flex-col bg-ground text-ink">
      <header data-testid="office-bar" className="flex h-12 shrink-0 items-center gap-2 border-b border-hair bg-surface px-2 sm:px-3">
        <Button variant="ghost" size="sm" onClick={onClose}>
          <HugeiconsIcon icon={ArrowLeft01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          <span className="max-sm:sr-only">{backLabel ?? t("office.back")}</span>
        </Button>
        <span aria-hidden="true" className="h-5 w-px shrink-0 bg-line" />
        <div className="flex min-w-0 flex-col">
          <h1 className="truncate text-sm leading-tight font-medium">{name}</h1>
          {loaded && writable && (
            <p className="truncate text-[12px] leading-tight text-muted" data-testid="office-saved" aria-live="polite">
              {doc.modified ? t("office.unsaved") : t("office.saved")}
            </p>
          )}
        </div>
        {state.kind === "open" && !state.session.writable && (
          <Badge variant="secondary">{t("office.readOnly")}</Badge>
        )}
        <span className="ml-auto" />
        {doc.viewers.length > 0 && <Viewers viewers={doc.viewers} />}
        {loaded && (
          <div className="flex items-center gap-0.5 max-sm:hidden">
            <Tool icon={MinusSignIcon} label={t("office.zoomOut")} onClick={doc.zoomOut} />
            <span className="w-11 text-center text-[13px] text-muted tabular-nums" data-testid="office-zoom">
              {doc.zoom === null ? "" : `${doc.zoom}%`}
            </span>
            <Tool icon={PlusSignIcon} label={t("office.zoomIn")} onClick={doc.zoomIn} />
            <span aria-hidden="true" className="mx-1 h-5 w-px bg-line" />
          </div>
        )}
        {loaded && <Tool icon={Pdf01Icon} label={t("office.pdf")} onClick={doc.exportPdf} />}
        {loaded && writable && (
          <Button size="sm" onClick={doc.save} disabled={!doc.modified} data-testid="office-save">
            <HugeiconsIcon icon={FloppyDiskIcon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            <span className="max-sm:sr-only">{t("office.save")}</span>
          </Button>
        )}
      </header>
      {writable && <OfficeToolbar kind={kind} doc={doc} disabled={!loaded} />}

      <div className="relative min-h-0 flex-1" aria-busy={busy}>
        {state.kind === "open" && (
          <iframe
            ref={frame}
            title={t("office.frameTitle", { name })}
            src={editorSrc(state.session, locale)}
            className="absolute inset-0 size-full border-0"
            allow="clipboard-read; clipboard-write; fullscreen"
          />
        )}
        {busy && (
          <div className="pointer-events-none absolute inset-0 flex items-center justify-center bg-ground/80">
            <Spinner
              size={28}
              label={name ? t("office.opening", { name }) : t("office.openingPlain")}
            />
          </div>
        )}
        {state.kind === "failed" && (
          <div className="flex h-full flex-col items-center justify-center gap-4 p-6 text-center">
            <p className="max-w-md text-sm text-muted">
              {state.error.reason === "unsupported"
                ? t("office.error.unsupported", { ext: state.error.extension })
                : t(`office.error.${state.error.reason}`)}
            </p>
            {(state.error.reason === "starting" || state.error.reason === "other") && (
              <Button variant="secondary" onClick={() => setAttempt((n) => n + 1)}>
                {t("office.retry")}
              </Button>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

/** An icon button that names itself in a tooltip, as the mail reader's do. */
function Tool({ icon, label, onClick }: { icon: IconSvgElement; label: string; onClick: () => void }) {
  return (
    <Tooltip>
      <TooltipTrigger render={<Button variant="ghost" size="icon-sm" aria-label={label} onClick={onClick} />}>
        <HugeiconsIcon icon={icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      </TooltipTrigger>
      <TooltipContent side="bottom">{label}</TooltipContent>
    </Tooltip>
  );
}

/** The others in the document, their pictures ringed in their cursor's
 * colour; four at most, then a count. */
function Viewers({ viewers }: { viewers: Viewer[] }) {
  const t = useT();
  const shown = viewers.slice(0, 4);
  const more = viewers.length - shown.length;
  return (
    <ul className="flex items-center -space-x-1.5 pr-1" aria-label={t("office.together")} data-testid="office-viewers">
      {shown.map((v) => (
        <li key={v.id} title={v.name}>
          <PersonAvatar
            person={{ name: v.name, displayName: v.name }}
            size="xs"
            aria-label={v.name}
            className="ring-2 ring-surface"
            style={{ boxShadow: `0 0 0 3.5px ${v.color}` }}
          />
        </li>
      ))}
      {more > 0 && (
        <li className="flex size-6 items-center justify-center rounded-full bg-sunk text-[10px] text-muted ring-2 ring-surface">
          +{more}
        </li>
      )}
    </ul>
  );
}
