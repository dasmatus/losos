/* Where an error message sends the owner: the matching page of the handbook.
 *
 * Every box serves its own copy of the handbook at /handbook/ (the
 * losos-handbook package, modules/containers.nix), so these are relative
 * paths on the admin origin and work with no internet. The right-hand side
 * is the page's slug under handbook/docs/, and each troubleshooting page
 * declares that slug in its front matter so a file rename cannot move it:
 * `npm run check:links` in handbook/ reads this file and fails when an
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
  "look-and-widgets": "manual/look-and-widgets/",
  "custom-domain": "types/official-edge/",
  tpm: "reference/tpm/",
} as const;

export type HandbookEntry = keyof typeof HANDBOOK_ENTRIES;

/** The address of a handbook page on this box. */
export function handbookHref(entry: HandbookEntry): string {
  return HANDBOOK_BASE + HANDBOOK_ENTRIES[entry];
}
