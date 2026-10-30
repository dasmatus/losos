import * as React from "react";
import { closePalette, isPaletteColon, isPaletteShortcut, openPalette, paletteState, subscribePalette } from "./store";

/* The command palette (./command-palette.tsx): its own chunk, fetched the
 * first time Ctrl/Cmd+Shift+P, a colon or the sidebar's search button opens
 * it. */
const CommandPalette = React.lazy(() => import("./command-palette"));

/* The palette on a page: the two ways in from the keyboard, and the palette
 * itself once something has opened it. The admin page and LosOS Cloud
 * (src/cloud/app.tsx) each hold one, so the keys work on every page the
 * sidebar's search button is on; before, LosOS Cloud and the documents
 * opened from it drew the button and answered neither key.
 *
 * Ctrl/Cmd+Shift+P from anywhere on the page, or a colon typed outside a
 * text field. A key some field has already handled (a code editor's own
 * binding) is left to it. The listener is on the capture phase of the
 * window, so an editor that stops a key on its way up (a document's own
 * Ctrl+Shift+P) cannot swallow the chord before the page sees it; whether
 * it was handled is read after, on the bubble side, for the colon. */
export function PaletteHost({ enabled }: { enabled: boolean }) {
  const open = React.useSyncExternalStore(subscribePalette, () => paletteState().open, () => false);
  // Once fetched, the palette stays mounted so it can close the way it opened.
  const [loaded, setLoaded] = React.useState(false);
  React.useEffect(() => {
    if (open) setLoaded(true);
  }, [open]);

  React.useEffect(() => {
    if (!enabled) {
      closePalette();
      return;
    }
    const onChord = (event: KeyboardEvent): void => {
      if (!isPaletteShortcut(event) || event.defaultPrevented) return;
      event.preventDefault();
      event.stopPropagation();
      if (paletteState().open) closePalette();
      else openPalette();
    };
    const onColon = (event: KeyboardEvent): void => {
      if (event.defaultPrevented || paletteState().open || !isPaletteColon(event)) return;
      event.preventDefault();
      openPalette();
    };
    window.addEventListener("keydown", onChord, true);
    window.addEventListener("keydown", onColon);
    return () => {
      window.removeEventListener("keydown", onChord, true);
      window.removeEventListener("keydown", onColon);
    };
  }, [enabled]);

  if (!enabled || !loaded) return null;
  return (
    <React.Suspense fallback={null}>
      <CommandPalette />
    </React.Suspense>
  );
}

/** The chord inside a same-origin frame (the Office editor): keys pressed
 *  in a frame go to its own window and never reach the page's, so a person
 *  typing in a document could not open the palette. Returns the undo; a
 *  frame of another origin is left alone. */
export function followFrameKeys(frame: HTMLIFrameElement): () => void {
  let target: Window | null = null;
  const onKey = (event: KeyboardEvent): void => {
    if (!isPaletteShortcut(event)) return;
    event.preventDefault();
    event.stopPropagation();
    if (paletteState().open) closePalette();
    else openPalette();
  };
  const attach = (): void => {
    detach();
    try {
      target = frame.contentWindow;
      target?.addEventListener("keydown", onKey, true);
    } catch {
      target = null;
    }
  };
  const detach = (): void => {
    try {
      target?.removeEventListener("keydown", onKey, true);
    } catch {
      /* gone with its document */
    }
    target = null;
  };
  frame.addEventListener("load", attach);
  attach();
  return () => {
    frame.removeEventListener("load", attach);
    detach();
  };
}
