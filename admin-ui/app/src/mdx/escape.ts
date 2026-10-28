/* Plain text as MDX reads it back, on its own so a page that only writes
 * a starter text (src/cloud/office/paper/new.ts) does not load the editor's
 * model to do it. */

/** Text as Markdown and MDX read it back as the same text. */
export function escapeText(text: string, lineStart: boolean): string {
  let out = text.replace(/[\\`*[\]<>{}~$|]/g, "\\$&");
  // Underscores only open emphasis at a word's edge.
  out = out.replace(/(^|[^\p{L}\p{N}])_|_(?=[^\p{L}\p{N}]|$)/gu, (m) => m.replace("_", "\\_"));
  out = out.replace(/&(?=[a-zA-Z#][a-zA-Z0-9]*;)/g, "\\&");
  if (lineStart) {
    out = out.replace(/^(\s*)([#+=-])/, "$1\\$2").replace(/^(\s*\d+)([.)])/, "$1\\$2");
  }
  return out;
}
