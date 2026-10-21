import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Cancel01Icon, CheckmarkCircle02Icon, ShuffleIcon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { useT } from "@/lib/i18n-react";
import type { MessageKey } from "@/lib/i18n";
import { cn } from "@/lib/utils";
import { bricks } from "./bricks";
import { Cabinet } from "./cabinet";
import { eel } from "./eel";
import { merge } from "./merge";
import { pairs } from "./pairs";
import type { BoardView, CardView, GameId, GameSpec, Phase, ShapeName } from "./types";
import { upstream } from "./upstream";

/* The game a long wait offers (components/play-while-waiting.tsx), in its
 * own chunk: nothing here is in the bundle until someone presses Play.
 *
 * It sits under the wait's own status and never in front of it, picks one
 * of five games at random, and owns every word around the canvas: the
 * game's name, the score and the best, how to play, and the overlay that
 * says how to start, that the round is over or that the game is paused.
 * It never starts a round by itself and makes no sound. Under reduced
 * motion only the two turn-based games are offered, and they turn at once.
 * When the wait ends the canvas goes and a line says so; the wait's own
 * screen is what the owner came back for. Nothing here fetches anything,
 * so it works on a box nobody has signed in to yet. */

export const GAMES: readonly GameSpec[] = [upstream, eel, merge, bricks, pairs];

const NAME: Record<GameId, MessageKey> = {
  upstream: "games.name.upstream",
  eel: "games.name.eel",
  merge: "games.name.merge",
  bricks: "games.name.bricks",
  pairs: "games.name.pairs",
};

const HOW: Record<GameId, MessageKey> = {
  upstream: "games.how.upstream",
  eel: "games.how.eel",
  merge: "games.how.merge",
  bricks: "games.how.bricks",
  pairs: "games.how.pairs",
};

const SHAPE: Record<ShapeName, MessageKey> = {
  disc: "games.shape.disc",
  ring: "games.shape.ring",
  square: "games.shape.square",
  triangle: "games.shape.triangle",
  diamond: "games.shape.diamond",
  plus: "games.shape.plus",
  star: "games.shape.star",
  salmon: "games.shape.salmon",
};

type T = ReturnType<typeof useT>;

/** A turn-based board in words, for the screen reader: Merge row by row,
 *  Pairs as the card under the cursor, what is face up and how many pairs
 *  are found. */
export function describeBoard(t: T, view: BoardView): string {
  if (view.kind === "tiles") {
    const rows: string[] = [];
    for (let row = 0; row * view.columns < view.values.length; row++) {
      const tiles = view.values
        .slice(row * view.columns, (row + 1) * view.columns)
        .map((value) => (value === 0 ? t("games.board.empty") : String(value)));
      rows.push(t("games.board.row", { row: row + 1, tiles: tiles.join(", ") }));
    }
    return rows.join(" ");
  }
  const card = (c: CardView) =>
    c.state === "down"
      ? t("games.card.down")
      : c.state === "matched"
        ? t("games.card.matched", { shape: t(SHAPE[c.shape]) })
        : t(SHAPE[c.shape]);
  const under = view.cards[view.cursor] ?? { state: "down" };
  const parts = [
    t("games.board.card", {
      row: Math.floor(view.cursor / view.columns) + 1,
      column: (view.cursor % view.columns) + 1,
      card: card(under),
    }),
  ];
  const up = view.cards.flatMap((c) => (c.state === "up" ? [t(SHAPE[c.shape])] : []));
  if (up.length > 0) parts.push(t("games.board.up", { shapes: up.join(", ") }));
  const found = view.cards.filter((c) => c.state === "matched").length / 2;
  parts.push(t("games.board.found", { found, all: view.cards.length / 2 }));
  return parts.join(" ");
}

function reducedMotion(): boolean {
  try {
    return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  } catch {
    return false;
  }
}

/** A game at random, never the one just played. */
export function pickGame(previous: GameId | null, reduced: boolean, random = Math.random): GameSpec {
  const pool = GAMES.filter((g) => (!reduced || g.turnBased) && g.id !== previous);
  return pool[Math.floor(random() * pool.length)] ?? upstream;
}

// ── Best scores ────────────────────────────────────────────────────────────

/* Per browser, in localStorage: a convenience, so a private window or
 * blocked storage just means no best yet. */
const BEST_KEY = "losos-games-best";

function readBest(): Partial<Record<GameId, number>> {
  try {
    const raw = window.localStorage.getItem(BEST_KEY);
    const parsed: unknown = raw === null ? {} : JSON.parse(raw);
    return typeof parsed === "object" && parsed !== null ? (parsed as Partial<Record<GameId, number>>) : {};
  } catch {
    return {};
  }
}

/* What this visit scored, which blocked storage would read back as
 * nothing: a worse round must not take the best's place. */
const visit: Partial<Record<GameId, number>> = {};

function bestOf(spec: GameSpec): number | undefined {
  const stored = readBest()[spec.id];
  const seen = visit[spec.id];
  if (typeof stored !== "number") return seen;
  if (seen === undefined) return stored;
  return better(spec, seen, stored) ? seen : stored;
}

function saveBest(id: GameId, value: number): void {
  visit[id] = value;
  try {
    window.localStorage.setItem(BEST_KEY, JSON.stringify({ ...readBest(), [id]: value }));
  } catch {
    /* Storage refused: the best lives for this visit only. */
  }
}

function better(spec: GameSpec, value: number, best: number | undefined): boolean {
  if (best === undefined) return value > 0;
  return spec.lowerIsBetter ? value < best : value > best;
}

// ── The arcade ─────────────────────────────────────────────────────────────

export interface ArcadeProps {
  /** The wait is still on. When it turns false the game stops. */
  waiting: boolean;
  onClose: () => void;
}

export default function Arcade({ waiting, onClose }: ArcadeProps) {
  const t = useT();
  const [spec, setSpec] = React.useState(() => pickGame(null, reducedMotion()));

  if (!waiting) {
    return (
      <div
        role="status"
        data-testid="game-ready"
        className="flex items-center gap-3 rounded-control border border-line bg-surface px-3 py-2 animate-fade-in"
      >
        <p className="flex min-w-0 flex-1 items-center gap-1.5 text-[13px] text-ok">
          <HugeiconsIcon icon={CheckmarkCircle02Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          {t("games.ready")}
        </p>
        <Button size="sm" variant="secondary" onClick={onClose}>
          {t("games.close")}
        </Button>
      </div>
    );
  }

  return (
    <Screen
      key={spec.id}
      spec={spec}
      onAnother={() => setSpec(pickGame(spec.id, reducedMotion()))}
      onClose={onClose}
    />
  );
}

function Screen({ spec, onAnother, onClose }: { spec: GameSpec; onAnother: () => void; onClose: () => void }) {
  const t = useT();
  const canvas = React.useRef<HTMLCanvasElement>(null);
  const cabinet = React.useRef<Cabinet | null>(null);
  const [phase, setPhase] = React.useState<Phase>(spec.turnBased ? "playing" : "idle");
  const [score, setScore] = React.useState(0);
  const [best, setBest] = React.useState<number | undefined>(() => bestOf(spec));
  const [paused, setPaused] = React.useState(false);
  const [final, setFinal] = React.useState<number | null>(null);
  const [view, setView] = React.useState<BoardView | null>(null);

  React.useEffect(() => {
    const element = canvas.current;
    if (element === null) return;
    const machine = new Cabinet(element, spec, {
      phase: (next) => {
        setPhase(next);
        if (next === "playing") setFinal(null);
      },
      score: setScore,
      over: (value) => {
        setFinal(value);
        if (better(spec, value, bestOf(spec))) {
          saveBest(spec.id, value);
          setBest(value);
        }
      },
      paused: setPaused,
      view: setView,
    });
    cabinet.current = machine;
    // Focus, so the keyboard plays at once; the round still waits for a key.
    element.focus({ preventScroll: true });
    return () => {
      machine.destroy();
      cabinet.current = null;
    };
  }, [spec]);

  const name = t(NAME[spec.id]);
  const how = t(HOW[spec.id]);
  const tally = spec.lowerIsBetter ? "games.tries" : "games.score";

  let overlay: string | null = null;
  if (paused) overlay = t("games.paused");
  else if (phase === "idle") overlay = t("games.start");
  else if (phase === "over" && final !== null)
    overlay = spec.lowerIsBetter ? t("games.cleared", { count: final }) : t("games.over", { count: final });

  return (
    <section
      aria-label={name}
      data-testid="game"
      data-game={spec.id}
      data-phase={phase}
      data-paused={paused ? "true" : "false"}
      className="flex flex-col gap-2 rounded-control border border-line bg-sunk p-3 animate-rise"
    >
      <header className="flex items-center gap-2">
        <div className="min-w-0 flex-1">
          <h3 className="text-[13.5px] leading-tight font-medium text-ink">{name}</h3>
          <p className="numeric text-[12px] text-muted" data-testid="game-score">
            {t(tally, { count: score })}
            {best !== undefined && <span className="ml-2 text-faint">{t("games.best", { count: best })}</span>}
          </p>
        </div>
        <Button size="xs" variant="ghost" onClick={onAnother}>
          <HugeiconsIcon icon={ShuffleIcon} size={14} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          {t("games.another")}
        </Button>
        <Button size="icon-xs" variant="ghost" onClick={onClose} aria-label={t("games.close")}>
          <HugeiconsIcon icon={Cancel01Icon} size={14} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </Button>
      </header>

      <div className={cn("relative mx-auto w-full", spec.turnBased ? "max-w-[296px]" : "max-w-[440px]")}>
        <canvas
          ref={canvas}
          tabIndex={0}
          role="application"
          aria-roledescription={t("games.role")}
          aria-label={`${name}. ${how}`}
          width={spec.width}
          height={spec.height}
          className="block h-auto w-full touch-none rounded-control border border-line bg-surface select-none"
        />
        {overlay !== null && (
          <div
            aria-hidden="true"
            className={cn(
              "pointer-events-none absolute inset-0 flex items-center justify-center p-4",
              "rounded-control bg-surface/75 text-center text-[13px] font-medium text-ink",
            )}
          >
            <p className="max-w-[28ch]">{overlay}</p>
          </div>
        )}
      </div>

      <p className="text-center text-[12px] text-muted">{how}</p>
      {/* Read out when a round ends; the score itself changes too often. */}
      <p className="sr-only" aria-live="polite">
        {phase === "over" && overlay !== null ? overlay : ""}
      </p>
      {/* A turn-based board after each move, since its canvas is only
          pixels. The action games move too fast for words. */}
      {view !== null && (
        <p className="sr-only" aria-live="polite" data-testid="game-board">
          {describeBoard(t, view)}
        </p>
      )}
    </section>
  );
}
