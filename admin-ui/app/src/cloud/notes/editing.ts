import { autocompletion, type Completion, type CompletionContext, type CompletionResult } from "@codemirror/autocomplete";
import type { Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { baseName, noteName } from "@/notes/links";
import type { Vault } from "@/notes/vault";

/* What Notes adds to the editor: `[[` offers the vault's notes and files,
 * and every change is reported so the page can draw the note beside the
 * text. The vault is read through a getter, since the editor starts once
 * and the vault keeps changing. */

/** How a link names a note: its bare name when that is unique, else its
 * path without the extension, as Obsidian writes them. */
export function linkName(vault: Vault, path: string): string {
  const name = noteName(path);
  const same = vault.notes.filter((n) => noteName(n.path).toLowerCase() === name.toLowerCase());
  return same.length > 1 ? path.replace(/\.md$/i, "") : name;
}

function notesFor(vault: Vault): Completion[] {
  const out: Completion[] = [];
  for (const n of vault.notes) {
    const name = linkName(vault, n.path);
    out.push({ label: name, detail: n.path.includes("/") ? n.path.slice(0, n.path.lastIndexOf("/")) : "", type: "text" });
    for (const alias of n.aliases) out.push({ label: alias, apply: `${name}|${alias}`, detail: name, type: "keyword" });
  }
  for (const f of vault.index.files) out.push({ label: baseName(f.path), detail: f.mime, type: "constant" });
  return out;
}

export function wikilinkCompletion(vault: () => Vault | null): Extension {
  const source = (context: CompletionContext): CompletionResult | null => {
    const v = vault();
    if (v === null) return null;
    const before = context.matchBefore(/!?\[\[[^\]\n]*$/);
    if (before === null) return null;
    const open = before.text.indexOf("[[") + 2;
    const inner = before.text.slice(open);
    // Past the name: a heading or an alias is the writer's own.
    if (inner.includes("|") || inner.includes("#")) return null;
    const after = context.state.sliceDoc(context.pos, context.pos + 2);
    const close = after === "]]" ? "" : "]]";
    return {
      from: before.from + open,
      options: notesFor(v).map((c) => ({ ...c, apply: `${typeof c.apply === "string" ? c.apply : c.label}${close}` })),
      validFor: /^[^\]|#\n]*$/,
    };
  };
  return autocompletion({ override: [source], icons: false });
}

/** Calls `onText` with the whole text after each change. */
export function onText(callback: (text: string) => void): Extension {
  return EditorView.updateListener.of((update) => {
    if (update.docChanged) callback(update.state.doc.toString());
  });
}
