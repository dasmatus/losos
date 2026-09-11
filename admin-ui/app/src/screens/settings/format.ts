/* Formatting for the settings screen. Pure, no DOM, no imports.
 *
 * Deliberately local rather than reaching into another screen's helpers:
 * these three functions are the whole of what this screen needs to turn a
 * number into something a person reads, and a settings pane that shares a
 * formatter with a dashboard widget acquires a reason to change when the
 * dashboard changes.
 */

const KIB = 1024;

/* Base-1024, the way `df -h` counts — which is the tool the owner would use
 * if this box had a shell, and the number lososd reports comes from the same
 * place. Calling 1024 bytes "1 KB" is the lie every disk utility tells; the
 * alternative is a capacity meter that disagrees with the filesystem. */
const UNITS = ["B", "KB", "MB", "GB", "TB", "PB"] as const;

/** A byte count as a person reads it. `null` and nonsense both render as a dash. */
export function formatBytes(bytes: number | null, digits = 1): string {
  if (bytes === null || !Number.isFinite(bytes) || bytes < 0) return "—";
  if (bytes < KIB) return `${Math.round(bytes)} B`;

  let value = bytes;
  let unit = 0;
  while (value >= KIB && unit < UNITS.length - 1) {
    value /= KIB;
    unit += 1;
  }

  /* Two significant figures past the point of usefulness is noise, and a
   * legend reading "412 GB · 168 GB · 96.0 GB" has one number dressed
   * differently from its neighbours for no reason a reader can see. So the
   * decimal appears only where it carries information: below ten, where the
   * difference between 4 GB and 4.4 GB is a tenth of the figure. */
  const shown = value >= 10 ? value.toFixed(0) : value.toFixed(digits);
  return `${shown} ${UNITS[unit] ?? "B"}`;
}

/** A duration in minutes as words: "8 hours", "7 h 30 min", "45 min". */
export function formatMinutes(total: number): string {
  if (!Number.isFinite(total) || total <= 0) return "no time";
  const hours = Math.floor(total / 60);
  const minutes = Math.round(total % 60);
  if (hours === 0) return `${minutes} min`;
  if (minutes === 0) return hours === 1 ? "1 hour" : `${hours} hours`;
  return `${hours} h ${minutes} min`;
}

/** "3 apps" / "1 app". The plural of an English noun that just takes an s. */
export function plural(count: number, one: string, many = `${one}s`): string {
  return `${count} ${count === 1 ? one : many}`;
}
