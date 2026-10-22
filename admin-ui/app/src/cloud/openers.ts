import type { ComponentType } from "react";
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

/* An opener that first asks the box whether it applies (Office asks which
 * types its Collabora opens) registers late. /cloud/open waits for those
 * answers, so a link from another app does not fall back to the folder
 * while the question is still out. */
const pending: Promise<unknown>[] = [];

export function registeringOpener(answer: Promise<unknown>): void {
  pending.push(answer.catch(() => {}));
}

export function openersSettled(): Promise<void> {
  return Promise.all(pending).then(() => {});
}

export function openersFor(entry: Entry): readonly Opener[] {
  return entry.dir ? [] : openers.filter((o) => o.accepts(entry));
}

/* What can be done with a folder besides opening it: open it in Notes,
 * publish it as a knowledge base. A right click on a folder's row lists
 * them, as does the selection bar when one folder is picked. Each part of
 * LosOS that works on folders adds itself here, like the openers. */

export interface FolderDialogProps {
  folder: Entry;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export interface FolderAction {
  id: string;
  label: (folder: Entry) => MessageKey;
  icon: IconSvgElement;
  /** Whether it is offered for this folder now. */
  offers?: (folder: Entry) => boolean;
  /** Where it goes in the web app, or else the dialog it opens. */
  href?: (folder: Entry) => string;
  Dialog?: ComponentType<FolderDialogProps>;
}

const folderActions: FolderAction[] = [];

export function registerFolderAction(action: FolderAction): void {
  if (!folderActions.some((a) => a.id === action.id)) folderActions.push(action);
}

export function folderActionsFor(entry: Entry): readonly FolderAction[] {
  return entry.dir ? folderActions.filter((a) => a.offers?.(entry) ?? true) : [];
}
