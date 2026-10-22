import { sanitize, splitFrontMatter } from "@/notes/markdown";

/* A presentation in Markdown: one file, its slides apart by a line of
 * `---`, as Obsidian's Slides and Advanced Slides read it. The file says
 * it is one in its properties (`slides: true`, or Marp's `marp: true`), so
 * a plain note with a rule in it stays a document.
 *
 * Each slide is drawn by the notes renderer, then laid out as LosOS lays
 * every slide: the text in the middle, the pictures on the right. */

export interface Slide {
  /** The slide's own Markdown. */
  source: string;
  /** Its first line in the file, from 0, counting the properties. */
  line: number;
}

const SEPARATOR = /^---[ \t]*$/;
const FENCE = /^\s{0,3}(`{3,}|~{3,})/;

/** Whether a file's properties make it a presentation. */
export function isDeck(properties: Record<string, unknown> | null): boolean {
  return properties !== null && (properties["slides"] === true || properties["marp"] === true);
}

/** The slides of a presentation, in order; never none. */
export function splitSlides(text: string): Slide[] {
  const { body, lines: front } = splitFrontMatter(text);
  const lines = body.split("\n");
  const slides: Slide[] = [];
  let start = 0;
  let fenced: string | null = null;
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i] ?? "";
    const f = FENCE.exec(line);
    if (f !== null) {
      const mark = f[1] ?? "";
      if (fenced === null) fenced = mark;
      else if (mark[0] === fenced[0] && mark.length >= fenced.length) fenced = null;
      continue;
    }
    if (fenced === null && SEPARATOR.test(line)) {
      slides.push({ source: lines.slice(start, i).join("\n"), line: front + start });
      start = i + 1;
    }
  }
  slides.push({ source: lines.slice(start).join("\n"), line: front + start });
  // A file that opens with a separator has no empty first slide.
  if (slides.length > 1 && (slides[0]?.source.trim() ?? "") === "") slides.shift();
  return slides;
}

/** Which slide the line `line` (from 0) is on. */
export function slideAt(slides: readonly Slide[], line: number): number {
  let at = 0;
  for (let i = 0; i < slides.length; i++) if ((slides[i]?.line ?? 0) <= line) at = i;
  return at;
}

export interface Laid {
  /** The text, sanitised HTML. */
  text: string;
  /** The pictures, each as sanitised HTML. */
  pictures: string[];
}

/** A slide's HTML (renderNote's, already clean) split in two: the text,
 * and the pictures that go on the right. A paragraph left empty once its
 * picture moved goes with it. */
export function layOut(html: string): Laid {
  const doc = new DOMParser().parseFromString(`<body>${html}</body>`, "text/html");
  const pictures: string[] = [];
  for (const img of [...doc.body.querySelectorAll("img")]) {
    // A picture inside a link keeps it.
    const link = img.parentElement?.tagName === "A" ? img.parentElement : null;
    const moved = link ?? img;
    pictures.push(moved.outerHTML);
    const parent = moved.parentElement;
    moved.remove();
    // What a line break between the text and the picture leaves behind.
    const blank = (n: ChildNode | null | undefined): n is ChildNode =>
      n != null && (n.nodeName === "BR" || (n.nodeType === Node.TEXT_NODE && n.textContent?.trim() === ""));
    while (blank(parent?.lastChild)) parent?.lastChild?.remove();
    while (blank(parent?.firstChild)) parent?.firstChild?.remove();
    if (parent !== null && parent.tagName === "P" && parent.childNodes.length === 0) parent.remove();
  }
  // Serialised again, so cleaned again: what reaches the page is always
  // DOMPurify's own output.
  return { text: sanitize(doc.body.innerHTML), pictures: pictures.map(sanitize) };
}
