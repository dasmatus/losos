import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { ArrowLeft01Icon, ArrowRight01Icon, Cancel01Icon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { renderNote, type Resolver } from "@/notes/markdown";
import { useNotesReady } from "@/notes/note-view";
import { layOut, type Slide } from "./slides";
import "@/notes/note.css";
import "./markdown.css";

/* A slide as LosOS draws one: 16:9, the text in the middle of what is
 * left of it, centred, and every picture on the slide in a column on the
 * right. Sizes are in the slide's own width (container units), so the
 * same slide reads the same in the strip, beside the editor and on a
 * projector. */

export function SlideView({
  slide,
  resolver,
  className,
  current,
}: {
  slide: Slide;
  resolver: Resolver;
  className?: string;
  current?: boolean;
}) {
  const ready = useNotesReady();
  const laid = React.useMemo(() => layOut(ready ? renderNote(slide.source, resolver).html : ""), [ready, slide.source, resolver]);
  const pictures = laid.pictures.length > 0;
  return (
    <div className={cn("md-slide", className)} data-current={current ? "" : undefined}>
      <div className="md-slide-inner" data-pictures={pictures ? "" : undefined}>
        {/* Sanitised by renderNote (DOMPurify, no style), then only split. */}
        <div className="md-slide-text note-body" dangerouslySetInnerHTML={{ __html: laid.text }} />
        {pictures && <div className="md-slide-pictures" dangerouslySetInnerHTML={{ __html: laid.pictures.join("") }} />}
      </div>
    </div>
  );
}

/** Every slide small, the one being edited marked; a click goes to it.
 * The list is the scroll view: only as tall as the column, the rest of
 * the slides moving inside it. */
export function Filmstrip({
  slides,
  current,
  resolver,
  onPick,
}: {
  slides: readonly Slide[];
  current: number;
  resolver: Resolver;
  onPick: (index: number) => void;
}) {
  const t = useT();
  const ref = React.useRef<HTMLOListElement>(null);
  React.useEffect(() => {
    ref.current?.querySelector<HTMLElement>(`[data-index="${current}"]`)?.scrollIntoView({ block: "nearest" });
  }, [current]);
  return (
    <ol ref={ref} className="md-strip flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto p-3" aria-label={t("office.md.slides")} data-testid="md-strip">
      {slides.map((slide, i) => (
        <li key={i} data-index={i} className="flex items-start gap-2">
          <span className="w-4 pt-1 text-right text-[11px] text-muted tabular-nums">{i + 1}</span>
          <button
            type="button"
            className={cn(
              "block min-w-0 flex-1 rounded-[6px] ring-1 ring-line transition-shadow focus-visible:outline-2 focus-visible:outline-accent",
              i === current && "ring-2 ring-accent",
            )}
            aria-label={t("office.md.goToSlide", { n: i + 1 })}
            aria-current={i === current ? "true" : undefined}
            onClick={() => onPick(i)}
          >
            <SlideView slide={slide} resolver={resolver} className="pointer-events-none rounded-[6px]" />
          </button>
        </li>
      ))}
    </ol>
  );
}

/** The slides full screen: arrows, space and the page keys move, Escape
 * leaves, and so does leaving full screen. */
export function Presenter({
  slides,
  start,
  resolver,
  onClose,
}: {
  slides: readonly Slide[];
  start: number;
  resolver: Resolver;
  onClose: (at: number) => void;
}) {
  const t = useT();
  const ref = React.useRef<HTMLDivElement>(null);
  const [at, setAt] = React.useState(start);
  const atRef = React.useRef(at);
  atRef.current = at;
  const last = slides.length - 1;
  const close = React.useRef(onClose);
  close.current = onClose;

  React.useEffect(() => {
    const el = ref.current;
    el?.focus();
    // Full screen where the browser allows it; the overlay covers the
    // window either way.
    el?.requestFullscreen?.().catch(() => {});
    let entered = false;
    const change = () => {
      if (document.fullscreenElement === el) entered = true;
      else if (entered) close.current(atRef.current);
    };
    document.addEventListener("fullscreenchange", change);
    return () => {
      document.removeEventListener("fullscreenchange", change);
      if (document.fullscreenElement === el) void document.exitFullscreen().catch(() => {});
    };
  }, []);

  const move = (by: number) => setAt((n) => Math.min(last, Math.max(0, n + by)));
  const onKey = (e: React.KeyboardEvent) => {
    switch (e.key) {
      case "ArrowRight":
      case "ArrowDown":
      case "PageDown":
      case " ":
      case "Enter":
        move(1);
        break;
      case "ArrowLeft":
      case "ArrowUp":
      case "PageUp":
      case "Backspace":
        move(-1);
        break;
      case "Home":
        setAt(0);
        break;
      case "End":
        setAt(last);
        break;
      case "Escape":
        onClose(at);
        break;
      default:
        return;
    }
    e.preventDefault();
  };

  const slide = slides[at];
  return (
    <div
      ref={ref}
      role="dialog"
      aria-modal="true"
      aria-label={t("office.md.presenting", { n: at + 1, of: slides.length })}
      tabIndex={-1}
      onKeyDown={onKey}
      className="md-present fixed inset-0 z-50 flex items-center justify-center bg-black outline-none"
      data-testid="md-presenter"
    >
      {slide !== undefined && (
        <button type="button" className="md-present-stage" aria-label={t("office.md.next")} onClick={() => move(1)}>
          <SlideView slide={slide} resolver={resolver} />
        </button>
      )}
      <div className="md-present-bar absolute right-4 bottom-4 flex items-center gap-1 rounded-full bg-black/60 px-2 py-1 text-white">
        <Button variant="ghost" size="icon-sm" className="text-white hover:bg-white/10" aria-label={t("office.md.previous")} onClick={() => move(-1)} disabled={at === 0}>
          <HugeiconsIcon icon={ArrowLeft01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </Button>
        <span className="min-w-12 text-center text-[13px] tabular-nums" data-testid="md-present-count">
          {at + 1} / {slides.length}
        </span>
        <Button variant="ghost" size="icon-sm" className="text-white hover:bg-white/10" aria-label={t("office.md.next")} onClick={() => move(1)} disabled={at === last}>
          <HugeiconsIcon icon={ArrowRight01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </Button>
        <Button variant="ghost" size="icon-sm" className="text-white hover:bg-white/10" aria-label={t("office.md.stopPresenting")} onClick={() => onClose(at)}>
          <HugeiconsIcon icon={Cancel01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </Button>
      </div>
    </div>
  );
}
