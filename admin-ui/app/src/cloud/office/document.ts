/* The open document, as the bars around the editor frame see it.
 *
 * Collabora's page is on this same origin (/browser/), so once it says
 * Document_Loaded the page reads its map (`app.map`) directly: each
 * command's state (`.uno:Bold` is "true", `.uno:StyleApply` is "Heading 1",
 * `.uno:Undo` is "disabled"), the `commandstatechanged` events that follow
 * it, and the zoom. Everything the page does to the document goes the
 * documented way, as postMessage: Send_UNO_Command, Action_Save,
 * Action_Export, Get_Views. When the map cannot be read (a newer Collabora
 * that moved it), the bars still work and only show no pressed state. */

import { useCallback, useEffect, useRef, useState, type RefObject } from "react";
import type { CommandArgs } from "./toolbar";

/** The part of Collabora's map the page uses. */
interface CoolMap {
  stateChangeHandler?: { getItemValue: (command: string) => unknown };
  on: (type: string, fn: (event: CoolEvent) => void) => void;
  off: (type: string, fn: (event: CoolEvent) => void) => void;
  getZoom: () => number;
  getZoomScale?: (to: number, from: number) => number;
  setZoom: (zoom: number) => void;
  zoomIn: () => void;
  zoomOut: () => void;
}

interface CoolEvent {
  commandName?: string;
  state?: unknown;
}

/** Someone else in the document, as Get_Views_Resp names them. */
export interface Viewer {
  id: string;
  name: string;
  /** Their cursor's colour in the document, #rrggbb. */
  color: string;
}

interface CoolMessage {
  MessageId?: string;
  Values?: unknown;
}

interface CoolView {
  ViewId?: string | number;
  UserName?: string;
  Color?: string | number;
  IsCurrentView?: boolean;
}

/** Collabora's zoom step for a page at its real size. */
const ACTUAL_SIZE = 10;

/** A colour as Collabora gives it: "#rrggbb" or a number 0xrrggbb. */
export function viewColor(color: string | number | undefined): string {
  if (typeof color === "string" && /^#[0-9a-f]{6}$/i.test(color)) return color;
  if (typeof color === "number" && Number.isFinite(color)) {
    return `#${(color & 0xffffff).toString(16).padStart(6, "0")}`;
  }
  return "#8c98a4";
}

/** The other people in the document, from Get_Views_Resp's list. */
export function otherViewers(values: unknown): Viewer[] {
  if (!Array.isArray(values)) return [];
  return (values as CoolView[])
    .filter((v) => v.IsCurrentView !== true && v.ViewId !== undefined)
    .map((v) => ({ id: String(v.ViewId), name: v.UserName ?? "", color: viewColor(v.Color) }));
}

function asState(value: unknown): string | undefined {
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return undefined;
}

export interface OpenDocument {
  /** A command's state as the document reports it. */
  state: (command: string) => string | undefined;
  run: (command: string, args?: CommandArgs) => void;
  /** Changes not yet written back to the box. */
  modified: boolean;
  save: () => void;
  exportPdf: () => void;
  viewers: Viewer[];
  /** The zoom in percent; null until the document is up. */
  zoom: number | null;
  zoomIn: () => void;
  zoomOut: () => void;
}

/** The document in `frame`, once `loaded`; `watched` are the commands the
 * bars show. `post` sends one message to the frame. */
export function useDocument(
  frame: RefObject<HTMLIFrameElement | null>,
  loaded: boolean,
  watched: readonly string[],
  post: (message: object) => void,
): OpenDocument & { onMessage: (msg: CoolMessage) => void } {
  const [states, setStates] = useState<Record<string, string>>({});
  const [viewers, setViewers] = useState<Viewer[]>([]);
  const [zoom, setZoom] = useState<number | null>(null);
  const map = useRef<CoolMap | null>(null);

  useEffect(() => {
    if (!loaded) return;
    let found: CoolMap | null = null;
    try {
      const win = frame.current?.contentWindow as (Window & { app?: { map?: CoolMap } }) | null;
      found = win?.app?.map ?? null;
    } catch {
      found = null;
    }
    map.current = found;
    post({ MessageId: "Get_Views" });
    if (found === null) return;
    const m = found;
    const keep = new Set([...watched, ".uno:ModifiedStatus"]);

    const first: Record<string, string> = {};
    for (const command of keep) {
      const value = asState(m.stateChangeHandler?.getItemValue(command));
      if (value !== undefined) first[command] = value;
    }
    setStates(first);

    const onState = (e: CoolEvent) => {
      if (e.commandName === undefined || !keep.has(e.commandName)) return;
      const value = asState(e.state);
      if (value === undefined) return;
      const command = e.commandName;
      setStates((s) => (s[command] === value ? s : { ...s, [command]: value }));
    };
    const readZoom = () => {
      const scale = m.getZoomScale?.(m.getZoom(), ACTUAL_SIZE) ?? 1;
      setZoom(Math.round(scale * 100));
    };
    m.on("commandstatechanged", onState);
    m.on("zoomend", readZoom);
    // The page at its real size: Collabora opens a text document fitted to
    // the window, which on a wide screen is a page larger than paper.
    try {
      m.setZoom(ACTUAL_SIZE);
    } catch {
      /* an older map: keep Collabora's zoom */
    }
    readZoom();
    return () => {
      m.off("commandstatechanged", onState);
      m.off("zoomend", readZoom);
      map.current = null;
      setZoom(null);
    };
  }, [frame, loaded, post, watched]);

  const onMessage = useCallback(
    (msg: CoolMessage) => {
      if (msg.MessageId === "View_Added" || msg.MessageId === "View_Removed") {
        post({ MessageId: "Get_Views" });
      } else if (msg.MessageId === "Get_Views_Resp") {
        setViewers(otherViewers(msg.Values));
      }
    },
    [post],
  );

  const state = useCallback((command: string) => states[command], [states]);

  const run = useCallback(
    (command: string, args?: CommandArgs) => {
      post({ MessageId: "Send_UNO_Command", Values: { Command: command, Args: args ?? {} } });
      // The button took the keyboard; give it back to the document.
      post({ MessageId: "Grab_Focus" });
    },
    [post],
  );

  const save = useCallback(() => {
    post({ MessageId: "Action_Save", Values: { DontTerminateEdit: true, DontSaveIfUnmodified: true } });
    post({ MessageId: "Grab_Focus" });
  }, [post]);

  const exportPdf = useCallback(() => post({ MessageId: "Action_Export", Values: { Format: "pdf" } }), [post]);

  const zoomIn = useCallback(() => map.current?.zoomIn(), []);
  const zoomOut = useCallback(() => map.current?.zoomOut(), []);

  return {
    state,
    run,
    modified: states[".uno:ModifiedStatus"] === "true",
    save,
    exportPdf,
    viewers,
    zoom,
    zoomIn,
    zoomOut,
    onMessage,
  };
}
