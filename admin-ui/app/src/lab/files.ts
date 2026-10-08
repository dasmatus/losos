/* Setups as files: Save, Open, and the last one kept in this browser.
 * The document itself is the core's (export_setup / import_setup); this is
 * the browser half that files.js had: the download, the file picker's size
 * cap, and localStorage. */

export const LAST_KEY = "losos-lab-last";
export const MAX_FILE = 1 << 20;

interface Downloads {
  save(input: { filename: string; data: Blob }): Promise<void>;
}
interface ClaudeRuntime {
  use?: (name: string) => Promise<unknown>;
}

/** Hand the setup to the viewer as a file. Resolves to false when the person
 *  declined or the viewer cannot save; throws nothing. */
export async function download(filename: string, text: string): Promise<"saved" | "declined" | "unsupported"> {
  const blob = new Blob([text], { type: "application/json" });
  // Inside a claude.ai artifact a plain download link does nothing; the
  // viewer's own save prompt does. Everywhere else (the box) the link.
  const claude = (window as unknown as { claude?: ClaudeRuntime }).claude;
  const dl =
    claude && typeof claude.use === "function"
      ? ((await claude.use("downloads").catch(() => null)) as Downloads | null)
      : null;
  if (dl) {
    try {
      await dl.save({ filename, data: blob });
      return "saved";
    } catch (e) {
      return (e as { code?: string } | null)?.code === "declined" ? "declined" : "unsupported";
    }
  }
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(a.href), 1000);
  return "saved";
}

/** Whatever was on the canvas when the tab closed. Storage can be missing
 *  (private window, blocked site data): then nothing is kept and nothing breaks. */
export function readLast(): string | null {
  try {
    return window.localStorage.getItem(LAST_KEY);
  } catch {
    return null;
  }
}

export function writeLast(text: string): void {
  try {
    window.localStorage.setItem(LAST_KEY, text);
  } catch {
    /* storage blocked */
  }
}
