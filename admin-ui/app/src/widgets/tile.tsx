/* One tile: run a widget, draw what it returned, and contain what it did not.
 *
 * A widget that throws greys out its own tile and takes nothing else down.
 * That is two separate containments, because there are two separate ways to
 * fail:
 *
 *   - the widget function rejects, or returns something that is not a
 *     WidgetResult. Caught around the call, below.
 *   - the RENDERER throws while drawing what the widget returned — a shape
 *     that typechecked at the boundary but is wrong deeper in. An exception
 *     during render unmounts the whole React tree above it unless something
 *     catches it, so every tile carries an error boundary. Without it one bad
 *     custom widget blanks the entire admin page, which on a box with no shell
 *     is the worst failure mode available.
 *
 * Error boundaries have no hook form. The class at the bottom is the only
 * class component in this app and it exists for exactly that reason.
 */

import * as React from "react";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import {
  Alert02Icon,
  ArrowDown01Icon,
  ArrowUp01Icon,
  Delete02Icon,
  RefreshIcon,
} from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Skeleton, SkeletonLine } from "@/components/ui/skeleton";
import { t } from "@/lib/i18n";
import { useLocale, useT } from "@/lib/i18n-react";
import { cn, setCssVar } from "@/lib/utils";
import type { WidgetInstance } from "@/lib/widgets";
import { catalogueEntry, CUSTOM_REFRESH_MS, spanForKind, type WidgetSpan } from "./catalogue";
import { BarView } from "./render/bar";
import { HeatmapView } from "./render/heatmap";
import { ListView } from "./render/list";
import { NumberView } from "./render/number";
import { getLookState, getServerLookState, subscribeLook } from "@/lib/look";
import { HandFrame } from "./hand-frame";
import { createSandbox } from "./sandbox";
import { compileSpec } from "./spec";
import { WidgetError, type WidgetFn, type WidgetResult } from "./types";
import "./widgets.css";

// ── Resolving an instance to something runnable ───────────────────────────

interface Runnable {
  run: WidgetFn;
  span: WidgetSpan;
  refreshMs: number;
  /** Used until the first run returns a title of its own. */
  name: string;
}

function resolve(instance: WidgetInstance): Runnable | { error: string } {
  if (instance.source.kind === "hand") {
    // Never reached: WidgetTile hands a hand-written widget to HandTile
    // before resolving. Kept so the switch stays exhaustive.
    return { error: t("look.frame.gone") };
  }
  if (instance.source.kind === "builtin") {
    const entry = catalogueEntry(instance.source.id);
    if (entry === undefined) {
      return { error: t("widgets.tile.notInVersion") };
    }
    return { run: entry.run, span: entry.span, refreshMs: entry.refreshMs, name: t(entry.name) };
  }

  const spec = instance.source.spec;
  try {
    return {
      run: compileSpec(spec),
      span: spanForKind(spec.type),
      refreshMs: CUSTOM_REFRESH_MS,
      name: spec.title,
    };
  } catch (error) {
    return { error: error instanceof Error ? error.message : t("widgets.tile.couldNotRead") };
  }
}

// ── Running it ────────────────────────────────────────────────────────────

type RunState =
  | { status: "loading" }
  | { status: "ready"; result: WidgetResult }
  | { status: "failed"; message: string };

function messageFor(error: unknown): string {
  if (error instanceof WidgetError) return error.message;
  if (error instanceof Error && error.message.trim().length > 0) {
    const message = error.message.trim();
    return message.length > 160 ? `${message.slice(0, 157)}…` : message;
  }
  return t("widgets.tile.stopped");
}

/* A returned value is checked before it is drawn.
 *
 * A built-in is typed, but a compiled spec produces a WidgetResult by
 * construction and a future widget source might not. The renderers index on
 * `type`, so a missing or unknown one has to fail here — where it greys one
 * tile — rather than three frames later inside a renderer. */
function isResult(value: unknown): value is WidgetResult {
  if (typeof value !== "object" || value === null) return false;
  const record = value as { type?: unknown; title?: unknown; data?: unknown };
  if (typeof record.title !== "string") return false;
  if (typeof record.data !== "object" || record.data === null) return false;
  return (
    record.type === "heatmap" ||
    record.type === "number" ||
    record.type === "bar" ||
    record.type === "list"
  );
}

export interface WidgetTileProps {
  instance: WidgetInstance;
  /** Position on the board; drives the enter stagger and the move buttons. */
  index: number;
  total: number;
  onRemove: (id: string) => void;
  onMove: (id: string, direction: "up" | "down") => void;
}

/* Two kinds of tile behind one name. A built-in or spec widget RUNS — a
 * function, a result, a renderer. A widget written by hand does not run
 * here at all: it is a document the box keeps, drawn by the sandboxed frame
 * (hand-frame.tsx), so its tile is a different thing with the same chrome. */
export function WidgetTile(props: WidgetTileProps) {
  return props.instance.source.kind === "hand" ? <HandTile {...props} /> : <RunTile {...props} />;
}

function RunTile({ instance, index, total, onRemove, onMove }: WidgetTileProps) {
  const t = useT();
  const locale = useLocale();
  // The locale is a dependency: resolving names a built-in and may produce a
  // parse error, both of which are text.
  const resolved = React.useMemo(() => {
    void locale;
    return resolve(instance);
  }, [instance, locale]);
  const [state, setState] = React.useState<RunState>({ status: "loading" });
  const [nonce, setNonce] = React.useState(0);

  const runnable = "error" in resolved ? null : resolved;
  const resolveError = "error" in resolved ? resolved.error : null;

  React.useEffect(() => {
    if (runnable === null) {
      setState({ status: "failed", message: resolveError ?? t("widgets.tile.couldNotRead") });
      return;
    }

    let live = true;
    const controller = new AbortController();

    const once = async (): Promise<void> => {
      try {
        const value = await runnable.run(createSandbox({ signal: controller.signal }));
        if (!live) return;
        if (!isResult(value)) {
          setState({ status: "failed", message: t("widgets.tile.nothingToDraw") });
          return;
        }
        /* A refresh redraws the tile, which is never more urgent than a click
         * or a keystroke. As a transition React yields to input between
         * slices of the render instead of holding the thread for all of it. */
        React.startTransition(() => setState({ status: "ready", result: value }));
      } catch (error) {
        if (!live) return;
        React.startTransition(() => setState({ status: "failed", message: messageFor(error) }));
      }
    };

    void once();

    /* Only while the tab is visible. A backgrounded dashboard polling a
     * mini-PC for hours is the sort of thing that gets noticed as "the box is
     * always busy", and there is nobody looking at the answer anyway. */
    const timer = setInterval(() => {
      if (document.visibilityState === "visible") void once();
    }, runnable.refreshMs);

    return () => {
      live = false;
      controller.abort();
      clearInterval(timer);
    };
    /* `runnable` changes identity with the locale (see `resolved`), so a
     * language change re-runs every tile and the built-ins' text follows. */
  }, [runnable, resolveError, nonce]);

  const title =
    instance.title ??
    (state.status === "ready" ? state.result.title : (runnable?.name ?? t("widgets.tile.fallbackTitle")));

  const failed = state.status === "failed";

  return (
    /* The stagger lives on a wrapper, not on the Card.
     *
     * Two reasons. The grid item is what `col-span` has to sit on, and Card's
     * own `animate-rise` would otherwise race the staggered animation for the
     * same property — so the Card is told `animate-none` and the wrapper
     * carries the motion. */
    <div
      ref={(element) => {
        setCssVar(element, "--tile-i", String(index));
      }}
      className={cn("widget-tile", runnable?.span === "full" && "md:col-span-2")}
    >
      <Card className={cn("flex h-full animate-none flex-col", failed && "bg-sunk")}>
      <TileHeader
        title={title}
        failed={failed}
        instance={instance}
        index={index}
        total={total}
        onRemove={onRemove}
        onMove={onMove}
      />

      <div className="flex flex-1 flex-col justify-between gap-3 px-4 pb-4">
        {state.status === "loading" && <TileSkeleton />}

        {state.status === "failed" && (
          <FailedBody message={state.message} onRetry={() => setNonce((n) => n + 1)} />
        )}

        {state.status === "ready" && (
          <TileBoundary onError={(message) => setState({ status: "failed", message })}>
            <WidgetBody result={state.result} />
          </TileBoundary>
        )}

        {state.status === "ready" && state.result.foot !== undefined && (
          <p className="text-[11.5px] leading-snug text-faint">{state.result.foot}</p>
        )}
      </div>
      </Card>
    </div>
  );
}

/* The tile's top row: the title and the three buttons, shared by both kinds
 * of tile so moving and removing a hand-written widget is the same gesture
 * as for any other. */
function TileHeader({
  title,
  failed,
  instance,
  index,
  total,
  onRemove,
  onMove,
}: WidgetTileProps & { title: string; failed: boolean }) {
  const t = useT();
  return (
    <div className="flex items-start gap-2 px-4 pt-4 pb-2">
      <h3
        className={cn(
          "min-w-0 flex-1 truncate text-[13px] font-semibold tracking-wide uppercase",
          failed ? "text-faint" : "text-muted",
        )}
      >
        {title}
      </h3>

      <div className="flex shrink-0 items-center gap-0.5">
        <IconButton
          label={t("widgets.tile.moveEarlier", { title })}
          icon={ArrowUp01Icon}
          disabled={index === 0}
          onClick={() => onMove(instance.id, "up")}
        />
        <IconButton
          label={t("widgets.tile.moveLater", { title })}
          icon={ArrowDown01Icon}
          disabled={index >= total - 1}
          onClick={() => onMove(instance.id, "down")}
        />
        <IconButton
          label={t("widgets.tile.remove", { title })}
          icon={Delete02Icon}
          onClick={() => onRemove(instance.id)}
        />
      </div>
    </div>
  );
}

/* A hand-written widget's tile.
 *
 * The widget is looked up on the box's look (lib/look.ts) by the id the
 * board keeps; its source goes to the sandboxed frame, which draws it and
 * reports its height. A widget the box no longer has greys the tile out
 * with a sentence, like a spec that no longer parses. An error the widget
 * throws is shown under it rather than replacing it: a tile that painted
 * something and then threw is still worth seeing. */
function HandTile({ instance, index, total, onRemove, onMove }: WidgetTileProps) {
  const t = useT();
  const look = React.useSyncExternalStore(subscribeLook, getLookState, getServerLookState);
  const [threw, setThrew] = React.useState<string | null>(null);
  const [nonce, setNonce] = React.useState(0);
  const handId = instance.source.kind === "hand" ? instance.source.id : "";
  const widget = look.look?.widgets.find((w) => w.id === handId);

  const title = instance.title ?? widget?.name ?? t("widgets.tile.fallbackTitle");
  const waiting = look.look === null && look.status !== "failed";
  const failed = !waiting && widget === undefined;

  return (
    <div
      ref={(element) => {
        setCssVar(element, "--tile-i", String(index));
      }}
      data-testid="hand-tile"
      className={cn("widget-tile", widget?.span === "full" && "md:col-span-2")}
    >
      <Card className={cn("flex h-full animate-none flex-col", failed && "bg-sunk")}>
        <TileHeader
          title={title}
          failed={failed}
          instance={instance}
          index={index}
          total={total}
          onRemove={onRemove}
          onMove={onMove}
        />
        <div className="flex flex-1 flex-col justify-between gap-3 px-4 pb-4">
          {waiting && <TileSkeleton />}
          {failed && (
            <FailedBody message={t("look.frame.gone")} onRetry={() => setNonce((n) => n + 1)} />
          )}
          {widget !== undefined && (
            <>
              <HandFrame
                key={nonce}
                files={widget.files}
                name={t("look.frame.title", { name: widget.name })}
                onError={setThrew}
              />
              {threw !== null && (
                <FailedBody
                  message={t("look.frame.threw", { message: threw })}
                  onRetry={() => {
                    setThrew(null);
                    setNonce((n) => n + 1);
                  }}
                />
              )}
            </>
          )}
        </div>
      </Card>
    </div>
  );
}

/** The four renderers behind one discriminated switch. Exported so the custom
 *  editor's live preview draws through exactly the same path a real tile does
 *  — a preview that agrees with the tile is the only kind worth having. */
export function WidgetBody({ result }: { result: WidgetResult }) {
  switch (result.type) {
    case "heatmap":
      return <HeatmapView data={result.data} summary={result.foot ?? result.title} />;
    case "number":
      return <NumberView data={result.data} />;
    case "bar":
      return <BarView data={result.data} />;
    case "list":
      return <ListView data={result.data} />;
  }
}

function TileSkeleton() {
  return (
    <div aria-busy="true" className="flex flex-col gap-3">
      <Skeleton className="h-8 w-32" />
      <SkeletonLine className="w-3/4" />
      <SkeletonLine className="w-1/2" />
    </div>
  );
}

/* The greyed-out tile.
 *
 * The message is the widget's own, printed as written. A widget author — or
 * the person who typed an expression into the Custom dialog — has to be able
 * to read what went wrong, and this box has no console to read it in. */
function FailedBody({ message, onRetry }: { message: string; onRetry: () => void }) {
  const t = useT();
  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-start gap-2.5">
        <HugeiconsIcon
          icon={Alert02Icon}
          size={20}
          strokeWidth={1.5}
          color="currentColor"
          className="mt-0.5 shrink-0 text-warn"
          aria-hidden="true"
        />
        <p className="text-[12.5px] leading-snug break-words text-muted">{message}</p>
      </div>
      <Button variant="secondary" size="sm" className="self-start" onClick={onRetry}>
        <HugeiconsIcon
          icon={RefreshIcon}
          size={15}
          strokeWidth={1.5}
          color="currentColor"
          aria-hidden="true"
        />
        {t("widgets.tile.tryAgain")}
      </Button>
    </div>
  );
}

function IconButton({
  label,
  icon,
  disabled,
  onClick,
}: {
  label: string;
  icon: IconSvgElement;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <Button
      variant="ghost"
      size="icon"
      className="size-7"
      aria-label={label}
      title={label}
      disabled={disabled === true}
      onClick={onClick}
    >
      <HugeiconsIcon icon={icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
    </Button>
  );
}

// ── The boundary ──────────────────────────────────────────────────────────

interface BoundaryProps {
  onError: (message: string) => void;
  children: React.ReactNode;
}

/* One tile's blast radius.
 *
 * Keyed on nothing: the parent swaps to the failed body as soon as onError
 * fires, so this instance is unmounted rather than asked to re-render. The
 * `hasError` state is only there to stop React rendering children again on
 * the way out. */
class TileBoundary extends React.Component<BoundaryProps, { hasError: boolean }> {
  constructor(props: BoundaryProps) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(): { hasError: boolean } {
    return { hasError: true };
  }

  override componentDidCatch(error: unknown): void {
    this.props.onError(messageFor(error));
  }

  override render(): React.ReactNode {
    return this.state.hasError ? null : this.props.children;
  }
}
