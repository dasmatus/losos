/* A hand-written widget's files, as the page handles them.
 *
 * A widget is `index.html` plus the files it links (a stylesheet, a script,
 * an SVG picture, some JSON). The rules match backend/src/look.rs, which
 * has the last word: lowercase names with one of a few text extensions, no
 * directories, `index.html` always there. The frame
 * (public/widget-frame/index.html) puts the linked files in place.
 */

import type { WidgetFile } from "@/lib/api";

/** The file the frame renders. */
export const ENTRY_FILE = "index.html";

/** What the editor highlights and completes a file as. */
export type FileLanguage = "html" | "css" | "javascript" | "json" | "markdown" | "text";

export function languageOf(name: string): FileLanguage {
  const kind = name.slice(name.lastIndexOf(".") + 1);
  switch (kind) {
    case "html":
    case "svg":
      return "html";
    case "css":
      return "css";
    case "js":
    case "mjs":
      return "javascript";
    case "json":
      return "json";
    case "md":
      return "markdown";
    default:
      return "text";
  }
}

/** Whether lososd would take `name`, given the kinds it allows. */
export function isFileName(name: string, kinds: readonly string[], maxChars: number): boolean {
  const dot = name.lastIndexOf(".");
  if (dot <= 0 || [...name].length > maxChars) return false;
  const stem = name.slice(0, dot);
  return (
    kinds.includes(name.slice(dot + 1)) &&
    /^[a-z0-9][a-z0-9._-]*$/.test(stem) &&
    !stem.includes("..")
  );
}

/** Bytes of all the files together, as lososd counts them. */
export function widgetBytes(files: readonly WidgetFile[]): number {
  const encoder = new TextEncoder();
  return files.reduce((sum, file) => sum + encoder.encode(file.content).length, 0);
}

/** `index.html` first, then the rest by name. */
export function sortFiles(files: readonly WidgetFile[]): WidgetFile[] {
  return [...files].sort((a, b) => {
    if (a.name === ENTRY_FILE) return -1;
    if (b.name === ENTRY_FILE) return 1;
    return a.name.localeCompare(b.name);
  });
}

/** The same files with `name`'s content replaced. */
export function withContent(files: readonly WidgetFile[], name: string, content: string): WidgetFile[] {
  return files.map((file) => (file.name === name ? { name, content } : file));
}

/** Whether two sets of files are the same, name for name. */
export function sameFiles(a: readonly WidgetFile[], b: readonly WidgetFile[]): boolean {
  if (a.length !== b.length) return false;
  const sortedB = sortFiles(b);
  return sortFiles(a).every((file, i) => file.name === sortedB[i]?.name && file.content === sortedB[i]?.content);
}

/** What a finished build hands back: its files, or from an older edge the
 *  single source as `index.html`. */
export function builtFiles(view: { files?: WidgetFile[] | null; source: string | null }): WidgetFile[] | null {
  if (Array.isArray(view.files) && view.files.length > 0) return sortFiles(view.files);
  if (view.source !== null) return [{ name: ENTRY_FILE, content: view.source }];
  return null;
}
