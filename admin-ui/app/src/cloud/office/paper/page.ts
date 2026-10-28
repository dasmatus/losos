/* Page setup: size, orientation and margins, kept in the document's front
 * matter as
 *
 *   page:
 *     size: A4
 *     orientation: portrait
 *     margin: normal
 *
 * which an export reads too. The page on the screen, the print and the
 * PDF all take them from here. */

import { renderMdx } from "@/mdx/wasm";

export const SIZES = ["A4", "A5", "Letter", "Legal"] as const;
export const ORIENTATIONS = ["portrait", "landscape"] as const;
export const MARGINS = ["narrow", "normal", "wide"] as const;

export type Size = (typeof SIZES)[number];
export type Orientation = (typeof ORIENTATIONS)[number];
export type Margin = (typeof MARGINS)[number];

export interface PageSetup {
  size: Size;
  orientation: Orientation;
  margin: Margin;
}

export const DEFAULT_PAGE: PageSetup = { size: "A4", orientation: "portrait", margin: "normal" };

/** Millimetres, portrait. */
export const SIZE_MM: Record<Size, [number, number]> = {
  A4: [210, 297],
  A5: [148, 210],
  Letter: [215.9, 279.4],
  Legal: [215.9, 355.6],
};

export const MARGIN_MM: Record<Margin, number> = { narrow: 12.7, normal: 25.4, wide: 38.1 };

/** The front matter's properties, or {} when there are none or they do
 * not read. */
export function properties(yaml: string | null): Record<string, unknown> {
  if (yaml === null || yaml.trim() === "") return {};
  try {
    const r = renderMdx(`---\n${yaml}\n---\n`, "mdx");
    return r.properties ?? {};
  } catch {
    return {};
  }
}

const pick = <T extends string>(list: readonly T[], value: unknown, fallback: T): T =>
  typeof value === "string" ? (list.find((v) => v.toLowerCase() === value.toLowerCase()) ?? fallback) : fallback;

export function pageOf(props: Record<string, unknown>): PageSetup {
  const page = props["page"];
  if (page === null || typeof page !== "object") return DEFAULT_PAGE;
  const p = page as Record<string, unknown>;
  return {
    size: pick(SIZES, p["size"], DEFAULT_PAGE.size),
    orientation: pick(ORIENTATIONS, p["orientation"], DEFAULT_PAGE.orientation),
    margin: pick(MARGINS, p["margin"], DEFAULT_PAGE.margin),
  };
}

function scalar(value: unknown): string {
  if (value === null || value === undefined) return "null";
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  const s = String(value);
  // Plain when YAML reads it back as the same string.
  return /^[\p{L}\p{N}][\p{L}\p{N} ._/-]*$/u.test(s) && !/^(true|false|null|yes|no|on|off|~|[-+]?[\d.]+)$/i.test(s) && !s.endsWith(" ")
    ? s
    : JSON.stringify(s);
}

/** Properties as YAML, the way a person would write them. */
export function toYaml(value: Record<string, unknown>, indent = ""): string {
  const lines: string[] = [];
  for (const [key, v] of Object.entries(value)) {
    const k = /^[\w-]+$/.test(key) ? key : JSON.stringify(key);
    if (Array.isArray(v)) {
      lines.push(v.length === 0 ? `${indent}${k}: []` : `${indent}${k}:`);
      for (const item of v) {
        if (item !== null && typeof item === "object") lines.push(`${indent}  -\n${toYaml(item as Record<string, unknown>, indent + "    ")}`);
        else lines.push(`${indent}  - ${scalar(item)}`);
      }
    } else if (v !== null && typeof v === "object") {
      lines.push(`${indent}${k}:`, toYaml(v as Record<string, unknown>, indent + "  "));
    } else {
      lines.push(`${indent}${k}: ${scalar(v)}`);
    }
  }
  return lines.join("\n");
}

/** `yaml` with its page setup set to `page`; the default page is left
 * out, so a document nobody set up carries no setup. */
export function withPage(yaml: string | null, page: PageSetup): string {
  const props = properties(yaml);
  const setup: Record<string, string> = {};
  if (page.size !== DEFAULT_PAGE.size) setup["size"] = page.size;
  if (page.orientation !== DEFAULT_PAGE.orientation) setup["orientation"] = page.orientation;
  if (page.margin !== DEFAULT_PAGE.margin) setup["margin"] = page.margin;
  if (Object.keys(setup).length === 0) delete props["page"];
  else props["page"] = setup;
  return toYaml(props);
}

/** The print's page rule: the size, the margins, and the header and
 * footer in the margin boxes with the page number where the document
 * puts one. */
export function printRule(page: PageSetup, header: string | null, footer: (string | null)[] | null): string {
  const [w, h] = SIZE_MM[page.size];
  const [pw, ph] = page.orientation === "portrait" ? [w, h] : [h, w];
  const quote = (s: string) => `"${s.replace(/[\\"]/g, "\\$&").replace(/\n/g, " ")}"`;
  const content = (parts: (string | null)[]) => parts.map((p) => (p === null ? "counter(page)" : quote(p))).join(" ") || '""';
  let boxes = "";
  if (header !== null && header !== "") boxes += `@top-center { content: ${quote(header)}; }`;
  if (footer !== null) boxes += `@bottom-center { content: ${content(footer)}; }`;
  return `@page { size: ${pw}mm ${ph}mm; margin: ${MARGIN_MM[page.margin]}mm; ${boxes} }`;
}
