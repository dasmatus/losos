import { formatFull, formatWhen } from "@/cloud/format";

/* Dates and ids as the pages show them. LosOS Git answers RFC 3339; the
 * words are LosOS Cloud's, so the two apps say "Today 14:05" alike. */

function seconds(iso: string | null | undefined): number {
  if (iso === null || iso === undefined || iso === "") return 0;
  const ms = Date.parse(iso);
  return Number.isNaN(ms) ? 0 : ms / 1000;
}

/** "Today 14:05", "12 Mar", "12 Mar 2024". */
export function when(iso: string | null | undefined): string {
  return formatWhen(seconds(iso));
}

/** The full date and time, for a title. */
export function whenFull(iso: string | null | undefined): string {
  const s = seconds(iso);
  return s > 0 ? formatFull(s) : "";
}

/** The first seven characters of a commit id. */
export function short(sha: string | null | undefined): string {
  return (sha ?? "").slice(0, 7);
}
