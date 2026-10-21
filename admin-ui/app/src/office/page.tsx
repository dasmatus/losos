import { useCallback, useMemo } from "react";
import { OfficeEditor } from "@/cloud/office/editor";
import { openOffice, OfficeFailure, type OfficeSession } from "@/cloud/office/api";
import { FOLDER_API, OfficeFiles } from "./files";

export interface OfficeLink {
  load: () => Promise<OfficeSession>;
  /** Where Back goes: the other box's files, or this box's home. */
  back: string;
}

/** Read the fragment. Anything that is neither form opens nothing. */
export function parseOfficeLink(fragment: string): OfficeLink {
  const p = new URLSearchParams(fragment);
  const file = p.get("file");
  if (file) {
    return { load: () => openOffice(file, FOLDER_API), back: "/office/" };
  }
  const url = p.get("url") ?? "";
  const token = p.get("token") ?? "";
  const ttl = Number(p.get("ttl"));
  // The editor is on this box's own /browser/; a link that frames anything
  // else is refused rather than followed.
  if (url.startsWith("/browser/") && token && Number.isFinite(ttl) && ttl > 0) {
    const session: OfficeSession = {
      url,
      accessToken: token,
      accessTokenTtl: ttl,
      writable: p.get("writable") === "1",
      name: p.get("name") ?? "",
    };
    const back = p.get("back") ?? "";
    return {
      load: () => Promise.resolve(session),
      back: /^https:\/\/[^/]/.test(back) ? back : "/",
    };
  }
  return { load: () => Promise.reject(new OfficeFailure("notFound")), back: "/" };
}

/** The list of the folder's documents with nothing after the #, else the
 * editor. */
export function OfficePage({ fragment }: { fragment: string }) {
  if (fragment === "") return <OfficeFiles />;
  return <OfficeDocument fragment={fragment} />;
}

function OfficeDocument({ fragment }: { fragment: string }) {
  const link = useMemo(() => parseOfficeLink(fragment), [fragment]);
  const close = useCallback(() => window.location.assign(link.back), [link]);
  return <OfficeEditor load={link.load} onClose={close} />;
}
