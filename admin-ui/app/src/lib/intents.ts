/* How one LosOS app hands something to another: a file to mail, a file to
 * a chat, a person to call, a file to the app that edits it.
 *
 * Each hand-over is a link on this origin, so it works from any page (the
 * admin shell, LosOS Cloud's own page, Chat's) and survives a reload. The
 * app on the other end reads its own parameters; this file is the one
 * place both sides agree on their shape, so no app invents a second one.
 *
 *   Mail      /mail?to=a&to=b&cc=…&subject=…&body=…&attach=<cloud path>…
 *   Chat      /chat/?with=<Matrix ID or sign-in name>[&call=voice|video]
 *             /chat/?share=<cloud path>…&text=…   (Chat asks which room)
 *   Contacts  /contacts?person=<sign-in name>   /contacts?id=<contact id>
 *   Cloud     /cloud/files?fileid=<id>          the file in its folder
 *             /cloud/open/<id>                  the file in the app that edits it
 *   Issues    /git/issues?new=1&title=…&body=…&into=<project key>
 *             /git/issues/<key>                 one issue
 *
 * A link into Mail, Chat or Contacts is only drawn while that app is live
 * for the person (`sharedApp`): those run on the shared side alone, and a
 * button that can only lead to a 404 is not offered. */

import { sharedApp } from "@/lib/api";
import { probeCode } from "@/lib/apps";

/** What Mail's composer opens with. */
export interface MailIntent {
  to: string[];
  cc: string[];
  subject: string;
  body: string;
  /** Paths in the person's LosOS Cloud, attached when the composer opens. */
  attach: string[];
}

const nonEmpty = (values: readonly string[] | undefined): string[] =>
  (values ?? []).map((v) => v.trim()).filter((v) => v !== "");

/** `/mail?…`: Mail's composer, filled in. */
export function composeMailHref(intent: Partial<MailIntent>): string {
  const q = new URLSearchParams();
  for (const to of nonEmpty(intent.to)) q.append("to", to);
  for (const cc of nonEmpty(intent.cc)) q.append("cc", cc);
  if (intent.subject !== undefined && intent.subject.trim() !== "") q.set("subject", intent.subject.trim());
  if (intent.body !== undefined && intent.body !== "") q.set("body", intent.body);
  for (const path of nonEmpty(intent.attach)) q.append("attach", path);
  // An empty composer still opens: "Write" from another app.
  const query = q.toString();
  return query === "" ? "/mail?compose=1" : `/mail?${query}`;
}

/** What `composeMailHref` wrote, or null when the address asks for no
 *  composer. `to` may also be a comma-separated list. */
export function readMailIntent(search: URLSearchParams): MailIntent | null {
  const keys = ["to", "cc", "subject", "body", "attach", "compose"];
  if (!keys.some((k) => search.has(k))) return null;
  const list = (k: string): string[] => nonEmpty(search.getAll(k).flatMap((v) => v.split(",")));
  return {
    to: list("to"),
    cc: list("cc"),
    subject: search.get("subject") ?? "",
    body: search.get("body") ?? "",
    attach: nonEmpty(search.getAll("attach")),
  };
}

/** The same address with every parameter `readMailIntent` reads taken
 *  out, for history.replaceState once the composer has them. */
export function withoutMailIntent(search: URLSearchParams): string {
  const rest = new URLSearchParams(search);
  for (const k of ["to", "cc", "subject", "body", "attach", "compose"]) rest.delete(k);
  const query = rest.toString();
  return query === "" ? "" : `?${query}`;
}

/** `/chat/?with=…`: the direct room with someone, and a call in it when
 *  asked. `person` is a Matrix ID, or a sign-in name on this box, which
 *  Chat turns into one. */
export function chatWithHref(person: string, options: { call?: "voice" | "video" } = {}): string {
  const q = new URLSearchParams({ with: person });
  if (options.call !== undefined) q.set("call", options.call);
  return `/chat/?${q.toString()}`;
}

/** `/chat/?share=…`: Chat asks which room, then sends these Cloud files
 *  and puts `text` in the composer. */
export function chatShareHref(share: { attach?: readonly string[]; text?: string }): string {
  const q = new URLSearchParams();
  for (const path of nonEmpty(share.attach)) q.append("share", path);
  if (share.text !== undefined && share.text !== "") q.set("text", share.text);
  return `/chat/?${q.toString()}`;
}

/** A person's card in Contacts. */
export function contactHref(who: { person: string } | { id: string | number }): string {
  return "person" in who
    ? `/contacts?person=${encodeURIComponent(who.person)}`
    : `/contacts?id=${encodeURIComponent(String(who.id))}`;
}

/** A Cloud file in its folder, marked. */
export function cloudFileHref(id: number): string {
  return `/cloud/files?fileid=${id}`;
}

/** A Cloud file in the app that edits it: Office for documents, Diagrams
 *  for .canvas, Notes for Markdown, the live editor for text; else its
 *  folder. LosOS Cloud's page decides, from the openers registered there. */
export function openFileHref(id: number): string {
  return `/cloud/open/${id}`;
}

/** `/git/issues?new=1&…`: LosOS Git's tracker with a new issue started,
 *  its title and description filled in, in project `into` when that is
 *  one the person may write to. */
export function newIssueHref(draft: { title?: string; body?: string; into?: string }): string {
  const q = new URLSearchParams({ new: "1" });
  if (draft.title !== undefined && draft.title.trim() !== "") q.set("title", draft.title.trim().slice(0, 200));
  if (draft.body !== undefined && draft.body !== "") q.set("body", draft.body.slice(0, 4000));
  if (draft.into !== undefined && draft.into !== "") q.set("into", draft.into);
  return `/git/issues?${q.toString()}`;
}

/** A title for an issue made of picked text: its first line, cut short. */
export function issueTitleFrom(text: string): string {
  const line = text.trim().split("\n")[0]?.trim() ?? "";
  return line.length > 80 ? `${line.slice(0, 79).trimEnd()}…` : line;
}

/** Picked text as a Markdown quote, cut short, with where it came from. */
export function quotedFrom(text: string, from: string): string {
  const cut = text.trim().length > 1500 ? `${text.trim().slice(0, 1499).trimEnd()}…` : text.trim();
  const quote = cut
    .split("\n")
    .map((line) => `> ${line}`.trimEnd())
    .join("\n");
  return from === "" ? quote : `${quote}\n\n${from}`;
}

let git: "unknown" | "absent" | "present" = "unknown";
let asked = false;

/** Whether this box runs LosOS Git, for "Make an issue": asked once a page,
 *  and offered until the box has said no. */
export function canMakeIssue(): boolean {
  if (!asked) {
    asked = true;
    void probeCode().then(
      (answer) => {
        git = answer;
      },
      () => {},
    );
  }
  return git !== "absent";
}

/** Whether a link into this app may be drawn now. */
export function canHandTo(app: "mail" | "chat" | "contacts"): boolean {
  return sharedApp(app);
}
