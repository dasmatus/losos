import { CloudError, joinPath, upload, type Entry } from "../../api";

/* A new document or presentation: a Markdown file in the folder, named
 * "Untitled document.md" (then "Untitled document 2.md" and on) and opened
 * in LosOS Office, where its name is one click from changing. A
 * presentation says so in its properties and starts with a title slide
 * and one more. */

export type Kind = "doc" | "deck";

/** Whether a file opens in LosOS's own editor rather than in Collabora. */
export function isMarkdown(name: string): boolean {
  return /\.(md|markdown)$/i.test(name);
}

export interface Starter {
  /** The file's name without ".md". */
  name: string;
  title: string;
  /** Under the title on a presentation's first slide. */
  subtitle: string;
  /** The second slide's heading and its first point. */
  next: string;
  point: string;
}

export function starterText(kind: Kind, s: Starter): string {
  if (kind === "doc") return `# ${s.title}\n\n`;
  return `---\nslides: true\n---\n\n# ${s.title}\n\n${s.subtitle}\n\n---\n\n## ${s.next}\n\n- ${s.point}\n`;
}

/** Makes the file, under the first free name, and answers with it. */
export async function createDocument(folder: string, kind: Kind, s: Starter): Promise<Entry> {
  const body = new Blob([starterText(kind, s)], { type: "text/markdown" });
  for (let n = 1; n < 100; n++) {
    const name = n === 1 ? `${s.name}.md` : `${s.name} ${n}.md`;
    try {
      return await upload(joinPath(folder, name), body, { ifNew: true }).done;
    } catch (err) {
      if (err instanceof CloudError && err.code === "exists") continue;
      throw err;
    }
  }
  throw new CloudError(409, "exists", "no free name");
}
