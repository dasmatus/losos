/* Where an error message sends the owner: the matching page of the handbook.
 *
 * Every box serves its own copy of the handbook at /handbook/ (the
 * losos-handbook package, modules/containers.nix), so these are relative
 * paths on the admin origin and work with no internet. The right-hand side
 * is the page's slug under handbook/docs/, and each troubleshooting page
 * declares that slug in its front matter so a file rename cannot move it:
 * `bun run check:links` in handbook/ reads this file and fails when an
 * entry here names a page that is not there.
 *
 * `index` is the fallback: an error with no page of its own sends the owner
 * to the chapter's start, which is the five-minute checklist. An error site
 * passes an entry to toast.error({ help }) or renders <HelpLink entry>, and
 * both say "What to do" and open the page in a new tab, so the screen the
 * error is on (a wizard step, a half-filled form) is still there when the
 * owner comes back. */

export const HANDBOOK_BASE = "/handbook/";

export const HANDBOOK_ENTRIES = {
  index: "troubleshooting/",
  "only-the-lan-works": "troubleshooting/only-the-lan-works/",
  "cannot-reach-the-box": "troubleshooting/cannot-reach-the-box/",
  "name-does-not-resolve": "troubleshooting/name-does-not-resolve/",
  "certificate-warning": "troubleshooting/certificate-warning/",
  "forgot-password": "troubleshooting/forgot-password/",
  "stuck-at-passphrase": "troubleshooting/stuck-at-passphrase/",
  "wizard-waits": "troubleshooting/wizard-waits/",
  "app-tile-404": "troubleshooting/app-tile-404/",
  "apply-fails": "troubleshooting/apply-fails/",
  "out-of-room": "troubleshooting/out-of-room/",
  "edge-not-found": "troubleshooting/edge-not-found/",
  "sharing-refused": "troubleshooting/sharing-refused/",
  "getting-help": "troubleshooting/getting-help/",
  "sign-in-and-spare-key": "manual/sign-in-and-spare-key/",
  moderation: "manual/moderation/",
  apps: "manual/apps/",
  "look-and-widgets": "manual/look-and-widgets/",
  pages: "manual/pages/",
  "backup-and-erase": "manual/backup-and-erase/",
  terminal: "manual/terminal/",
  chat: "manual/chat/",
  calls: "manual/calls/",
  mail: "manual/mail/",
  calendar: "manual/calendar/",
  pictures: "manual/pictures/",
  git: "manual/losos-git/",
  "mesh-ci": "manual/mesh-ci/",
  machines: "manual/machines/",
  "custom-domain": "types/official-edge/",
  "edge-manages-box": "types/company-edge/",
  "edge-tiers": "in-depth/edge-federation/",
  tpm: "reference/tpm/",
  credentials: "reference/credentials/",
  "problem-reports": "manual/problem-reports/",
  wiki: "manual/wiki/",
  // The chapters, as LosOS Wiki lists the handbook among the box's wikis.
  "chapter-start": "start/what-is-losos/",
  "chapter-types": "types/",
  "chapter-manual": "manual/admin-pages/",
  "chapter-troubleshooting": "troubleshooting/",
  "chapter-reference": "reference/glossary/",
  "chapter-in-depth": "in-depth/",
} as const;

export type HandbookEntry = keyof typeof HANDBOOK_ENTRIES;

/* A company edge's manage page (src/manage/) is served by the edge, which
 * carries no handbook, so it links the published copy, where every page has
 * the same path. */
let base: string = HANDBOOK_BASE;

/** Link the handbook at `next` instead of this box's own copy. */
export function linkHandbookAt(next: string): void {
  base = next;
}

/** The handbook's own front page. */
export function handbookHome(): string {
  return base;
}

/** The address of a handbook page on this box. */
export function handbookHref(entry: HandbookEntry): string {
  return base + HANDBOOK_ENTRIES[entry];
}

/** The handbook's own search page, for `q`, in the reader's language
 *  where the handbook has it (English at the top, the others below their
 *  code). */
export function handbookSearchHref(q: string, locale: string): string {
  const lang = locale === "sk" || locale === "de" ? `${locale}/` : "";
  return `${base}${lang}search?q=${encodeURIComponent(q)}`;
}
