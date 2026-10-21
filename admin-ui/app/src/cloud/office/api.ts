/* Office documents: the two calls the editor page and the file view make,
 * and the address of the editor frame.
 *
 * The box answers `POST <base>/open {fileId}` with an editor session
 * (backend/src/office/mod.rs, OpenSession): Collabora's editor page on this
 * same origin, under /browser/, and an access token for that one file. LosOS
 * cloud serves the calls at /cloud/api/office; a box that edits a plain
 * folder (losos.office.folder) at /office/api. */

import { INTL_TAG, type Locale } from "@/lib/i18n";

export interface OfficeSession {
  url: string;
  accessToken: string;
  /** When the token runs out, in milliseconds since 1970. */
  accessTokenTtl: number;
  writable: boolean;
  name: string;
  /** The editor works only on the box's https address (coolwsd sits behind
   * nginx's TLS and points its page at https and wss). */
  https?: boolean;
}

export interface OfficeType {
  extension: string;
  /** Whether Office edits this type, or only shows it. */
  edit: boolean;
}

export type OfficeError = "notFound" | "unsupported" | "starting" | "signedOut" | "other";

export class OfficeFailure extends Error {
  constructor(
    readonly reason: OfficeError,
    readonly extension = "",
  ) {
    super(reason);
  }
}

export const CLOUD_OFFICE_API = "/cloud/api/office";

async function failure(res: Response): Promise<OfficeFailure> {
  let body: { error?: string; extension?: string } = {};
  try {
    body = (await res.json()) as typeof body;
  } catch {
    /* not JSON: an nginx page */
  }
  if (res.status === 401) return new OfficeFailure("signedOut");
  if (res.status === 404) return new OfficeFailure("notFound");
  if (res.status === 422) return new OfficeFailure("unsupported", body.extension ?? "");
  if (res.status === 502 || res.status === 503) return new OfficeFailure("starting");
  return new OfficeFailure("other");
}

export async function openOffice(fileId: string, base = CLOUD_OFFICE_API): Promise<OfficeSession> {
  const res = await fetch(`${base}/open`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify({ fileId }),
  });
  if (!res.ok) throw await failure(res);
  return (await res.json()) as OfficeSession;
}

export async function officeTypes(base = CLOUD_OFFICE_API): Promise<OfficeType[]> {
  const res = await fetch(`${base}/extensions`, { credentials: "same-origin" });
  if (!res.ok) throw await failure(res);
  return ((await res.json()) as { extensions: OfficeType[] }).extensions;
}

/** Whether `name` is a file Office opens, by its extension. */
export function opensInOffice(name: string, types: readonly OfficeType[]): boolean {
  const dot = name.lastIndexOf(".");
  if (dot <= 0) return false;
  const ext = name.slice(dot + 1).toLowerCase();
  return types.some((t) => t.extension === ext);
}

/** The frame's address: the editor, the token, the interface language, and
 * a close button that posts `UI_Close` back to the page. The token travels
 * in the query because the page's policy allows no form posts; coolwsd
 * puts it in the document's WebSocket address anyway. */
export function editorSrc(session: OfficeSession, locale: Locale): string {
  const sep = session.url.includes("?") ? "&" : "?";
  const params = new URLSearchParams({
    access_token: session.accessToken,
    access_token_ttl: String(session.accessTokenTtl),
    lang: INTL_TAG[locale],
    closebutton: "1",
  });
  return `${session.url}${sep}${params.toString()}`;
}

/** Collabora keeps its light or dark look in this origin's localStorage, under
 * `darkTheme`, and reads it when the editor loads. The editor is on this same
 * origin, so the page sets it to the LosOS theme before the frame opens. */
export function matchEditorTheme(dark: boolean): void {
  try {
    window.localStorage.setItem("darkTheme", dark ? "true" : "false");
  } catch {
    /* storage refused: the editor opens in its own light look */
  }
}

/** The https address of this same page, when `session` needs one and the
 * page was opened over http; null when the page can stay. */
export function httpsAddress(session: OfficeSession, location: Location): string | null {
  if (!session.https || location.protocol !== "http:") return null;
  return `https://${location.host}${location.pathname}${location.search}${location.hash}`;
}
