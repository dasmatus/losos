/* A presentation's slides as they are shown: drawn by losos-mdx from each
 * `<Slide>`'s own text, so the strip, the full screen and an export show
 * the same thing. The HTML has been through the crate's allowlist; the
 * speaker notes are in it and hidden until N shows them. */

import * as React from "react";
import { createPortal } from "react-dom";
import type { Node as PMNode } from "prosemirror-model";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { block } from "@/mdx/to-mdx";
import { renderMdx, type Resolver } from "@/mdx/wasm";

export interface SlideShown {
  /** Where the slide starts in the page, for going to it. */
  pos: number;
  layout: string;
  html: string;
  title: string;
}

/** The slides of `doc`, drawn. Blocks outside any slide are left out. */
export function slidesOf(doc: PMNode, resolver: Resolver): SlideShown[] {
  const out: SlideShown[] = [];
  doc.forEach((node, offset) => {
    if (node.type.name !== "slide") return;
    let html = "";
    try {
      html = renderMdx(block(node), "mdx", resolver).html;
    } catch {
      html = "";
    }
    let title = "";
    node.descendants((n) => {
      if (title === "" && n.type.name === "heading") title = n.textContent;
      return title === "";
    });
    out.push({ pos: offset, layout: String(node.attrs["layout"]), html, title });
  });
  return out;
}

function SlideFace({ slide, className }: { slide: SlideShown; className?: string }) {
  return (
    <div
      className={cn("paper-show mdx-document", `paper-slide-${slide.layout}`, className)}
      // losos-mdx's output: sanitized by its allowlist, nothing runs.
      dangerouslySetInnerHTML={{ __html: slide.html }}
    />
  );
}

export function Filmstrip({ slides, current, onPick }: { slides: SlideShown[]; current: number; onPick: (index: number) => void }) {
  const t = useT();
  return (
    <nav aria-label={t("paper.slides")} className="flex h-full flex-col gap-3 overflow-y-auto p-3" data-testid="paper-filmstrip">
      {slides.map((s, i) => (
        <button
          key={`${i}-${s.pos}`}
          type="button"
          onClick={() => onPick(i)}
          aria-current={i === current ? "true" : undefined}
          aria-label={`${i + 1}. ${s.title}`}
          className={cn(
            "flex items-start gap-2 rounded-control p-1 text-left outline-none focus-visible:ring-2 focus-visible:ring-accent",
            i === current ? "bg-accent-wash" : "hover:bg-sunk",
          )}
        >
          <span className="w-4 shrink-0 pt-0.5 text-[11px] text-muted tabular-nums">{i + 1}</span>
          <span className="paper-thumb">
            <SlideFace slide={s} />
          </span>
        </button>
      ))}
    </nav>
  );
}

/** Full screen, from `start`; `onClose` gets the slide it ended on. */
export function Presenter({ slides, start, onClose }: { slides: SlideShown[]; start: number; onClose: (at: number) => void }) {
  const t = useT();
  const [at, setAt] = React.useState(start);
  const [notes, setNotes] = React.useState(false);
  const ref = React.useRef<HTMLDivElement>(null);
  const atRef = React.useRef(at);
  atRef.current = at;

  React.useEffect(() => {
    const el = ref.current;
    el?.focus();
    el?.requestFullscreen?.().catch(() => {});
    const left = () => {
      if (document.fullscreenElement === null) onClose(atRef.current);
    };
    document.addEventListener("fullscreenchange", left);
    return () => {
      document.removeEventListener("fullscreenchange", left);
      if (document.fullscreenElement !== null) document.exitFullscreen().catch(() => {});
    };
  }, [onClose]);

  const go = (d: number) => setAt((n) => Math.max(0, Math.min(slides.length - 1, n + d)));
  const slide = slides[at];

  return createPortal(
    <div
      ref={ref}
      tabIndex={-1}
      role="dialog"
      aria-modal="true"
      aria-label={t("office.tool.present")}
      data-testid="paper-presenter"
      data-notes={notes ? "" : undefined}
      className="paper-presenter fixed inset-0 z-50 flex items-center justify-center bg-black outline-none"
      onKeyDown={(e) => {
        if (e.key === "ArrowRight" || e.key === "PageDown" || e.key === " ") go(1);
        else if (e.key === "ArrowLeft" || e.key === "PageUp") go(-1);
        else if (e.key === "Home") setAt(0);
        else if (e.key === "End") setAt(slides.length - 1);
        else if (e.key === "n" || e.key === "N") setNotes((v) => !v);
        else if (e.key === "Escape") onClose(at);
        else return;
        e.preventDefault();
      }}
      onClick={(e) => go(e.clientX > window.innerWidth / 3 ? 1 : -1)}
    >
      {slide !== undefined && <SlideFace slide={slide} className="paper-show-full" />}
      <p className="pointer-events-none absolute bottom-3 left-1/2 -translate-x-1/2 text-[12px] text-white/50">
        {at + 1} / {slides.length} · {t("paper.present.hint")}
      </p>
    </div>,
    document.body,
  );
}
