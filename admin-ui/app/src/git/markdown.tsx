import * as React from "react";
import { Link } from "react-router-dom";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";

/* A README, a pull request's description and a comment are Markdown. This
 * draws the common part of it (headings, paragraphs, lists, quotes, code,
 * tables, links, emphasis) as React elements, never as HTML from the text, so a
 * comment cannot put markup or script on the page. Raw HTML in the text
 * stays text; a link goes only to http(s), mailto or a path on the box.
 * Given the keys of the tracker's projects, `LOS-12` becomes a link to
 * that issue and `@eva` is set apart, as LosOS Git reads them
 * (backend/src/forge/tracker/refs.rs). */

type Block =
  | { kind: "heading"; level: number; text: string }
  | { kind: "code"; text: string; info: string }
  | { kind: "table"; head: string[]; align: Align[]; rows: string[][] }
  | { kind: "list"; ordered: boolean; items: string[] }
  | { kind: "quote"; text: string }
  | { kind: "rule" }
  | { kind: "paragraph"; text: string };

type Align = "left" | "center" | "right" | undefined;

const BULLET = /^\s*[-*+]\s+(.*)$/u;
const TABLE_RULE = /^\s*\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)*\|?\s*$/u;

/** A table row's cells: the outer pipes dropped, split on pipes a backslash
 * does not escape. */
function cells(line: string): string[] {
  const row = line.trim().replace(/^\|/u, "").replace(/(?<!\\)\|$/u, "");
  return row.split(/(?<!\\)\|/u).map((c) => c.trim().replace(/\\\|/gu, "|"));
}

/** A table starts at a row with a pipe followed by its rule of dashes. */
function tableAt(lines: string[], i: number): boolean {
  const next = lines[i + 1];
  return (lines[i] ?? "").includes("|") && next !== undefined && next.includes("-") && TABLE_RULE.test(next);
}
const NUMBERED = /^\s*\d+[.)]\s+(.*)$/u;

function blocks(source: string): Block[] {
  const lines = source.replace(/\r\n?/gu, "\n").split("\n");
  const out: Block[] = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i] ?? "";
    const fence = /^\s*(```|~~~)\s*([^\s`]*)/u.exec(line);
    if (fence !== null) {
      const body: string[] = [];
      i += 1;
      while (i < lines.length && !(lines[i] ?? "").trimStart().startsWith(fence[1] ?? "```")) {
        body.push(lines[i] ?? "");
        i += 1;
      }
      i += 1;
      out.push({ kind: "code", text: body.join("\n"), info: fence[2] ?? "" });
      continue;
    }
    if (line.trim() === "") {
      i += 1;
      continue;
    }
    const heading = /^(#{1,6})\s+(.*?)\s*#*\s*$/u.exec(line);
    if (heading !== null) {
      out.push({ kind: "heading", level: heading[1]?.length ?? 1, text: heading[2] ?? "" });
      i += 1;
      continue;
    }
    if (/^\s*([-*_])(\s*\1){2,}\s*$/u.test(line)) {
      out.push({ kind: "rule" });
      i += 1;
      continue;
    }
    if (line.trimStart().startsWith(">")) {
      const body: string[] = [];
      while (i < lines.length && (lines[i] ?? "").trimStart().startsWith(">")) {
        body.push((lines[i] ?? "").trimStart().replace(/^>\s?/u, ""));
        i += 1;
      }
      out.push({ kind: "quote", text: body.join("\n") });
      continue;
    }
    if (tableAt(lines, i)) {
      const head = cells(line);
      const align = cells(lines[i + 1] ?? "").map((c): Align => {
        const left = c.startsWith(":");
        const right = c.endsWith(":");
        return left && right ? "center" : right ? "right" : left ? "left" : undefined;
      });
      const rows: string[][] = [];
      i += 2;
      while (i < lines.length && (lines[i] ?? "").includes("|") && (lines[i] ?? "").trim() !== "") {
        const row = cells(lines[i] ?? "");
        rows.push(head.map((_, c) => row[c] ?? ""));
        i += 1;
      }
      out.push({ kind: "table", head, align, rows });
      continue;
    }
    const ordered = NUMBERED.test(line);
    if (ordered || BULLET.test(line)) {
      const pattern = ordered ? NUMBERED : BULLET;
      const items: string[] = [];
      while (i < lines.length) {
        const l = lines[i] ?? "";
        const m = pattern.exec(l);
        if (m !== null) items.push(m[1] ?? "");
        else if (l.trim() !== "" && /^\s+/u.test(l) && items.length > 0) items[items.length - 1] += ` ${l.trim()}`;
        else break;
        i += 1;
      }
      out.push({ kind: "list", ordered, items });
      continue;
    }
    const body: string[] = [];
    while (i < lines.length) {
      const l = lines[i] ?? "";
      if (l.trim() === "" || /^(#{1,6}\s|\s*(```|~~~)|\s*>)/u.test(l) || BULLET.test(l) || NUMBERED.test(l) || tableAt(lines, i)) break;
      body.push(l);
      i += 1;
    }
    out.push({ kind: "paragraph", text: body.join("\n") });
  }
  return out;
}

function safeHref(url: string): string | null {
  if (/^(https?:|mailto:)/iu.test(url)) return url;
  if (url.startsWith("/") && !url.startsWith("//")) return url;
  if (url.startsWith("#")) return url;
  return null;
}

const INLINE = /(`+)([\s\S]*?[^`])\1(?!`)|\*\*([\s\S]+?)\*\*|\*([^*\s](?:[^*]*[^*\s])?)\*|\b_([^_\s](?:[^_]*[^_\s])?)_\b|!?\[([^\]]+)\]\(([^)\s]+)\)|(https?:\/\/[^\s<>()]+[^\s<>().,;:!?'"])/gu;

/* The project keys of the Markdown being drawn, set for the length of one
 * synchronous render by `Markdown`. */
let projectKeys: ReadonlySet<string> | null = null;

const REFS = /\b([A-Z][A-Z0-9]{1,5})-(\d+)\b|(^|[^\w.@])@([A-Za-z0-9][A-Za-z0-9_.-]*[A-Za-z0-9]|[A-Za-z0-9])/gu;

/** Plain text, with issue keys and @-mentions picked out. */
function refs(text: string, key: string): React.ReactNode[] {
  if (projectKeys === null) return [text];
  const keys = projectKeys;
  const out: React.ReactNode[] = [];
  let last = 0;
  let n = 0;
  for (const m of text.matchAll(REFS)) {
    const k = `${key}-r${n++}`;
    if (m[1] !== undefined && m[2] !== undefined) {
      if (!keys.has(m[1])) continue;
      if (m.index > last) out.push(text.slice(last, m.index));
      out.push(
        <Link key={k} to={`/issues/${m[1]}-${m[2]}`} className="font-medium text-accent hover:underline" data-issue-ref>
          {m[0]}
        </Link>,
      );
      last = m.index + m[0].length;
    } else if (m[4] !== undefined) {
      const at = m.index + (m[3]?.length ?? 0);
      if (at > last) out.push(text.slice(last, at));
      out.push(
        <span key={k} className="rounded-control bg-accent-wash px-1 font-medium text-accent" data-mention>
          @{m[4]}
        </span>,
      );
      last = at + 1 + m[4].length;
    }
  }
  if (last < text.length) out.push(text.slice(last));
  return out;
}

function inline(text: string, key = "i"): React.ReactNode[] {
  const out: React.ReactNode[] = [];
  let last = 0;
  let n = 0;
  for (const m of text.matchAll(INLINE)) {
    const at = m.index;
    if (at > last) out.push(...refs(text.slice(last, at), `${key}-t${n}`));
    const k = `${key}-${n++}`;
    if (m[2] !== undefined) {
      out.push(
        <code key={k} className="rounded-sm bg-sunk px-1 py-0.5 font-mono text-[0.9em]">
          {m[2].replace(/^ (.*) $/u, "$1")}
        </code>,
      );
    } else if (m[3] !== undefined) {
      out.push(<strong key={k}>{inline(m[3], k)}</strong>);
    } else if (m[4] !== undefined || m[5] !== undefined) {
      out.push(<em key={k}>{inline(m[4] ?? m[5] ?? "", k)}</em>);
    } else if (m[6] !== undefined && m[7] !== undefined) {
      const href = safeHref(m[7]);
      out.push(
        href === null ? (
          m[6]
        ) : (
          <a key={k} href={href} className="text-accent underline underline-offset-2" rel="noreferrer noopener">
            {inline(m[6], k)}
          </a>
        ),
      );
    } else if (m[8] !== undefined) {
      out.push(
        <a key={k} href={m[8]} className="text-accent underline underline-offset-2" rel="noreferrer noopener">
          {m[8]}
        </a>,
      );
    }
    last = at + m[0].length;
  }
  if (last < text.length) out.push(...refs(text.slice(last), `${key}-t${n}`));
  return out;
}

/** Line breaks inside a paragraph stay breaks, as people type them. */
function lines(text: string, key: string): React.ReactNode[] {
  return text.split("\n").flatMap((l, i) => (i === 0 ? inline(l, `${key}-${i}`) : [<br key={`${key}-br${i}`} />, ...inline(l, `${key}-${i}`)]));
}

const HEADING = ["text-[19px]", "text-[17px]", "text-[15px]", "text-[14px]", "text-[13.5px]", "text-[13px]"];

export function Markdown({
  source,
  className,
  issueKeys,
}: {
  source: string;
  className?: string;
  /** The tracker's project keys, to link `LOS-12` and mark `@eva`. */
  issueKeys?: ReadonlySet<string>;
}) {
  const t = useT();
  const parsed = React.useMemo(() => blocks(source), [source]);
  projectKeys = issueKeys ?? null;
  try {
    return drawn(parsed, className, t);
  } finally {
    projectKeys = null;
  }
}

function drawn(parsed: Block[], className: string | undefined, t: ReturnType<typeof useT>) {
  return (
    <div className={cn("flex min-w-0 flex-col gap-3 text-[14px] leading-relaxed text-ink [overflow-wrap:anywhere]", className)} data-markdown>
      {parsed.map((b, i) => {
        const key = `b${i}`;
        switch (b.kind) {
          case "heading": {
            const Tag = `h${Math.min(b.level + 1, 6)}` as "h2";
            return (
              <Tag key={key} className={cn("font-semibold tracking-tight", HEADING[b.level - 1])}>
                {inline(b.text, key)}
              </Tag>
            );
          }
          case "code":
            // A reviewer's ```suggestion block: the line it would put in.
            if (b.info === "suggestion") {
              return (
                <figure key={key} className="overflow-hidden rounded-control border border-line" data-suggestion>
                  <figcaption className="border-b border-line bg-sunk px-3 py-1.5 text-[12px] font-medium text-muted">{t("git.markdown.suggestion")}</figcaption>
                  <pre className="overflow-x-auto bg-ok/10 px-3 py-2 font-mono text-[12.5px] leading-snug">
                    <code>{b.text}</code>
                  </pre>
                </figure>
              );
            }
            return (
              <pre key={key} className="overflow-x-auto rounded-control border border-line bg-sunk px-3 py-2 font-mono text-[12.5px] leading-snug">
                <code>{b.text}</code>
              </pre>
            );
          case "table":
            return (
              <div key={key} className="overflow-x-auto rounded-control border border-line">
                {/* Cells wrap at spaces only, so a file name stays whole. */}
                <table className="w-full border-collapse text-[13px] [overflow-wrap:normal]">
                  <thead className="bg-sunk">
                    <tr>
                      {b.head.map((h, c) => (
                        <th key={`${key}-h${c}`} className="border-b border-line px-3 py-1.5 text-left font-semibold" style={{ textAlign: b.align[c] }}>
                          {inline(h, `${key}-h${c}`)}
                        </th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {b.rows.map((row, r) => (
                      <tr key={`${key}-r${r}`} className="border-t border-line first:border-t-0">
                        {row.map((cell, c) => (
                          <td key={`${key}-r${r}-${c}`} className="px-3 py-1.5 align-top" style={{ textAlign: b.align[c] }}>
                            {inline(cell, `${key}-r${r}-${c}`)}
                          </td>
                        ))}
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            );
          case "list": {
            const Tag = b.ordered ? "ol" : "ul";
            return (
              <Tag key={key} className={cn("flex flex-col gap-1 pl-5", b.ordered ? "list-decimal" : "list-disc")}>
                {b.items.map((item, j) => (
                  <li key={`${key}-${j}`}>{inline(item, `${key}-${j}`)}</li>
                ))}
              </Tag>
            );
          }
          case "quote":
            return (
              <blockquote key={key} className="border-l-2 border-line pl-3 text-muted">
                {lines(b.text, key)}
              </blockquote>
            );
          case "rule":
            return <hr key={key} className="border-line" />;
          case "paragraph":
            return <p key={key}>{lines(b.text, key)}</p>;
        }
      })}
    </div>
  );
}
