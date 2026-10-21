import type { IconSvgElement } from "@hugeicons/react";
import type { MessageKey } from "@/lib/i18n";
import type { Entry } from "./api";

/* What opens a file other than the browser: LosOS Office for documents,
 * editing together for text. Each part of LosOS that opens files adds
 * itself here; a row's name opens with the first one that takes the file,
 * and the selection bar lists all of them. With none, a file opens in a
 * new tab as the browser shows it. */

export interface Opener {
  id: string;
  label: MessageKey;
  icon: IconSvgElement;
  accepts: (entry: Entry) => boolean;
  /** Where it opens, on this origin. */
  href: (entry: Entry) => string;
}

const openers: Opener[] = [];

export function registerOpener(opener: Opener): void {
  if (!openers.some((o) => o.id === opener.id)) openers.push(opener);
}

export function openersFor(entry: Entry): readonly Opener[] {
  return entry.dir ? [] : openers.filter((o) => o.accepts(entry));
}
