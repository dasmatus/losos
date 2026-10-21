/* The Office editor: Collabora Online in a frame that fills its parent,
 * under a thin bar with the way back and the document's name. The shell's
 * top bar stays above it, so the logo and the person are not repeated.
 *
 * Collabora talks to the page that frames it with postMessage, one JSON
 * object per message ({MessageId, Values}). It waits for Host_PostmessageReady
 * before it sends anything, says App_LoadingStatus/Document_Loaded when the
 * document is up, and UI_Close when its own close button is pressed. Messages
 * are taken only from the frame's own window and this origin: the editor is
 * on /browser/ of this same box.
 *
 * LosOS cloud mounts <OfficeEditor> at /cloud/office/:fileId; Documents
 * (src/screens/Office.tsx) shows it for a plain folder and for a document
 * shared from another box. */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { ArrowLeft01Icon } from "@hugeicons/core-free-icons";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import { useLocale, useT } from "@/lib/i18n-react";
import { getResolvedTheme } from "@/lib/theme";
import {
  editorLook,
  editorSrc,
  httpsAddress,
  matchEditorTheme,
  OfficeFailure,
  type OfficeSession,
} from "./api";

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
  Values?: { Status?: string };
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
      if (msg.MessageId === "App_LoadingStatus") {
        if (msg.Values?.Status === "Frame_Ready") post({ MessageId: "Host_PostmessageReady" });
        if (msg.Values?.Status === "Document_Loaded") {
          setState((s) => (s.kind === "open" ? { ...s, loaded: true } : s));
        }
      } else if (msg.MessageId === "UI_Close") {
        onClose();
      }
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, [onClose, post]);

  const name = state.kind === "open" ? state.session.name : "";
  useEffect(() => {
    document.title = name ? `${name} · LosOS` : "LosOS";
  }, [name]);

  // The editor's colours, type and corners, read from the page when the
  // document opens, in the theme matchEditorTheme gave the editor.
  const look = useMemo(() => (state.kind === "open" ? editorLook(document.documentElement) : ""), [state.kind]);
  const busy = state.kind === "opening" || (state.kind === "open" && !state.loaded);

  return (
    <div className="flex h-full min-h-0 flex-col bg-ground text-ink">
      <header className="flex h-11 shrink-0 items-center gap-2 border-b border-line bg-surface px-2 sm:px-4">
        <Button variant="ghost" size="sm" onClick={onClose}>
          <HugeiconsIcon icon={ArrowLeft01Icon} size={18} strokeWidth={1.5} aria-hidden="true" />
          <span className="max-sm:sr-only">{backLabel ?? t("office.back")}</span>
        </Button>
        <span aria-hidden="true" className="h-5 w-px bg-line" />
        <h1 className="min-w-0 truncate text-sm font-medium">{name}</h1>
        {state.kind === "open" && !state.session.writable && (
          <Badge variant="secondary">{t("office.readOnly")}</Badge>
        )}
      </header>

      <main className="relative min-h-0 flex-1" aria-busy={busy}>
        {state.kind === "open" && (
          <iframe
            ref={frame}
            title={t("office.frameTitle", { name })}
            src={editorSrc(state.session, locale, look)}
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
      </main>
    </div>
  );
}
