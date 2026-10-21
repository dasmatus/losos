import * as React from "react";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { GraphView } from "./graph";
import type { Heading } from "./markdown";
import { DEFAULT_GRAPH, displayName, type Vault } from "./vault";

/* What sits beside an open note, as Obsidian's right sidebar does: the
 * note's local graph (its neighbours, one to three links out), the notes
 * that link here, the ones it links, the names it links that have no note
 * yet, and its outline. */

export interface NoteSideProps {
  vault: Vault;
  path: string;
  headings: readonly Heading[];
  href: (path: string) => string;
  onOpen: (path: string) => void;
  onHeading: (id: string) => void;
  /** A missing name clicked; without it the names are plain text. */
  onMissing?: (target: string) => void;
  className?: string;
}

function Section({ title, count, children, data }: { title: string; count?: number; children: React.ReactNode; data: string }) {
  return (
    <section className="flex flex-col gap-1.5" data-side={data}>
      <h2 className="flex items-baseline gap-1.5 text-[12px] font-semibold tracking-wide text-muted uppercase">
        {title}
        {count !== undefined && <span className="font-normal text-faint tabular-nums">{count}</span>}
      </h2>
      {children}
    </section>
  );
}

function NoteLink({ path, label, href, onOpen }: { path: string; label: string; href: string; onOpen: (path: string) => void }) {
  return (
    <a
      href={href}
      onClick={(event) => {
        if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey) return;
        event.preventDefault();
        onOpen(path);
      }}
      className="block truncate rounded-control px-1.5 py-0.5 text-[13px] text-ink hover:bg-sunk hover:text-accent focus-visible:outline-2 focus-visible:outline-accent"
    >
      {label}
    </a>
  );
}

export function NoteSide({ vault, path, headings, href, onOpen, onHeading, onMissing, className }: NoteSideProps) {
  const t = useT();
  const [depth, setDepth] = React.useState("1");
  const graph = React.useMemo(() => vault.graph(DEFAULT_GRAPH, path, Number(depth)), [vault, path, depth]);
  const back = vault.linkedFrom(path);
  const resolved = vault.resolved.get(path);
  const out = (resolved?.out ?? []).map((p) => vault.note(p)).filter((n) => n !== undefined);
  const missing = resolved?.missing ?? [];

  return (
    <aside className={cn("flex flex-col gap-5", className)} aria-label={t("notes.side.label")} data-note-side>
      <Section title={t("notes.side.graph")} data="graph">
        <GraphView
          nodes={graph.nodes}
          edges={graph.edges}
          current={`n:${path}`}
          onOpen={(node) => {
            if (node.kind === "note" || node.kind === "canvas") onOpen(node.key);
          }}
          label={t("notes.graph.local", { name: displayName({ path, title: null }) })}
          className="h-56 rounded-card border border-hair bg-surface"
          labels={graph.nodes.length <= 14}
        />
        <ToggleGroup variant="outline" size="sm" value={depth} onValueChange={(v) => v !== "" && setDepth(v)} aria-label={t("notes.side.depth")} className="w-fit">
          {["1", "2", "3"].map((d) => (
            <ToggleGroupItem key={d} value={d} data-depth={d}>
              {t("notes.side.depthN", { count: Number(d) })}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
      </Section>

      <Section title={t("notes.side.backlinks")} count={back.length} data="backlinks">
        {back.length === 0 ? (
          <p className="px-1.5 text-[12.5px] text-faint">{t("notes.side.noBacklinks")}</p>
        ) : (
          back.map((n) => <NoteLink key={n.path} path={n.path} label={displayName(n)} href={href(n.path)} onOpen={onOpen} />)
        )}
      </Section>

      {out.length > 0 && (
        <Section title={t("notes.side.outgoing")} count={out.length} data="outgoing">
          {out.map((n) => (
            <NoteLink key={n.path} path={n.path} label={displayName(n)} href={href(n.path)} onOpen={onOpen} />
          ))}
        </Section>
      )}

      {missing.length > 0 && (
        <Section title={t("notes.side.missing")} count={missing.length} data="missing">
          {missing.map((m) =>
            onMissing === undefined ? (
              <span key={m} className="block truncate px-1.5 py-0.5 text-[13px] text-faint">
                {m}
              </span>
            ) : (
              <button
                key={m}
                type="button"
                onClick={() => onMissing(m)}
                title={t("notes.missing.create", { name: m })}
                className="block w-full truncate rounded-control px-1.5 py-0.5 text-left text-[13px] text-muted hover:bg-sunk hover:text-accent focus-visible:outline-2 focus-visible:outline-accent"
              >
                {m}
              </button>
            ),
          )}
        </Section>
      )}

      {headings.length > 0 && (
        <Section title={t("notes.side.outline")} data="outline">
          <ul className="flex flex-col">
            {headings.map((h, i) => (
              <li key={`${h.id}:${i}`} className={cn(h.level >= 3 && "pl-3", h.level >= 5 && "pl-6")}>
                <button
                  type="button"
                  onClick={() => onHeading(h.id)}
                  className="block w-full truncate rounded-control px-1.5 py-0.5 text-left text-[13px] text-muted hover:bg-sunk hover:text-ink focus-visible:outline-2 focus-visible:outline-accent"
                >
                  {h.text}
                </button>
              </li>
            ))}
          </ul>
        </Section>
      )}
    </aside>
  );
}
