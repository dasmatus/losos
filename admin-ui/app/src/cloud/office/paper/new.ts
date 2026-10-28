import { escapeText } from "@/mdx/escape";
import { CloudError, joinPath, upload, type Entry } from "../../api";

/* LosOS Office's own files: a document is `.paper`, a presentation
 * `.slides`, both MDX (src/mdx/). A new one lands in the folder as
 * "Untitled document.paper" (then "Untitled document 2.paper" and on) and
 * opens in the editor, where its name is one click from changing. A
 * presentation starts with a title slide and one more.
 *
 * This file stays small on purpose: the file list and the openers import
 * it, and the editor itself is a chunk of its own. */

export type Kind = "paper" | "slides";

/** Whether a file is one of LosOS Office's own documents. */
export function isPaper(name: string): boolean {
  return /\.(paper|slides)$/i.test(name);
}

export function kindOf(name: string): Kind {
  return /\.slides$/i.test(name) ? "slides" : "paper";
}

export interface Starter {
  /** The file's name without its extension. */
  name: string;
  title: string;
  /** Under the title on a presentation's first slide. */
  subtitle: string;
  /** The second slide's heading and its first point. */
  next: string;
  point: string;
}

/** Text as the editor writes it, so the first save changes nothing. */
export function starterText(kind: Kind, s: Starter): string {
  if (kind === "paper") return `# ${line(s.title)}\n`;
  return (
    `<Slide layout="title">\n# ${line(s.title)}\n\n${line(s.subtitle)}\n</Slide>\n\n` +
    `<Slide layout="content">\n## ${line(s.next)}\n\n- ${line(s.point)}\n</Slide>\n`
  );
}

// The starter's words come from the translations; nothing in them may
// start markup.
function line(text: string): string {
  return escapeText(text.replace(/\n/g, " "), true);
}

/** Makes the file, under the first free name, and answers with it. */
export async function createPaper(folder: string, kind: Kind, s: Starter): Promise<Entry> {
  const body = new Blob([starterText(kind, s)], { type: "text/plain" });
  const ext = kind === "paper" ? "paper" : "slides";
  for (let n = 1; n < 100; n++) {
    const name = n === 1 ? `${s.name}.${ext}` : `${s.name} ${n}.${ext}`;
    try {
      return await upload(joinPath(folder, name), body, { ifNew: true }).done;
    } catch (err) {
      if (err instanceof CloudError && err.code === "exists") continue;
      throw err;
    }
  }
  throw new CloudError(409, "exists", "no free name");
}
