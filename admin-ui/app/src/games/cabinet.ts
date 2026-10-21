import { readPalette } from "./draw";
import type { BoardView, Direction, Game, GameSpec, Palette, Phase } from "./types";

/* The machine a game runs in: the canvas, the frame loop and the input.
 *
 * Kept out of React because a frame loop is imperative and sixty renders a
 * second would cost a two-core box more than the game does. The arcade
 * (arcade.tsx) hears about the few things it draws in the DOM: the phase,
 * the score and pausing.
 *
 * The loop runs only while an action game is being played: never for a
 * turn-based game, which draws once per input, never before the first
 * input, and never while paused. A hidden tab pauses, and so do an action
 * game losing focus and one scrolled out of view, so a game is never left
 * running unseen. */

/** Physics steps at a fixed rate, so a slow frame cannot tunnel a ball. */
const STEP = 1 / 120;
/** A frame longer than this (a stalled tab) is not caught up. */
const LONGEST_FRAME = 0.05;
/** A pointer that travels this far, in logical units, is a swipe. */
const SWIPE = 18;
/** The canvas's resolution stops at twice the CSS pixels. */
const MOST_DPR = 2;

export interface CabinetEvents {
  phase(phase: Phase): void;
  score(score: number): void;
  over(final: number): void;
  paused(paused: boolean): void;
  /** A turn-based board changed (or was first drawn). */
  view?(view: BoardView): void;
}

export class Cabinet {
  private readonly ctx: CanvasRenderingContext2D;
  private game: Game;
  private palette: Palette = readPalette();
  private frame: number | null = null;
  private last = 0;
  private carry = 0;
  private paused = false;
  private phase: Phase;
  private press: { x: number; y: number; id: number } | null = null;
  private readonly cleanup: (() => void)[] = [];

  constructor(
    private readonly canvas: HTMLCanvasElement,
    private readonly spec: GameSpec,
    private readonly events: CabinetEvents,
    random: () => number = Math.random,
  ) {
    const ctx = canvas.getContext("2d");
    if (ctx === null) throw new Error("no 2D canvas");
    this.ctx = ctx;
    this.game = spec.create({
      score: (value) => events.score(value),
      over: (final) => events.over(final),
      redraw: () => this.draw(),
      random,
    });
    this.phase = this.game.phase;
    events.phase(this.phase);

    this.listen(canvas, "keydown", (e) => this.onKey(e as KeyboardEvent));
    this.listen(canvas, "keyup", (e) => this.game.keyUp?.((e as KeyboardEvent).key));
    this.listen(canvas, "pointerdown", (e) => this.onDown(e as PointerEvent));
    this.listen(canvas, "pointermove", (e) => this.onMove(e as PointerEvent));
    this.listen(canvas, "pointerup", (e) => this.onUp(e as PointerEvent));
    this.listen(canvas, "pointercancel", () => this.letGo());
    // A key or pointer that comes up elsewhere is never heard, so whatever
    // is held lets go with the focus.
    this.listen(canvas, "blur", () => {
      this.letGo();
      if (!spec.turnBased && this.game.phase === "playing") this.setPaused(true);
    });
    this.listen(document, "visibilitychange", () => {
      if (document.hidden && this.game.phase === "playing") this.setPaused(true);
    });

    // The theme: an explicit choice on <html>, or the system's.
    const repaint = () => {
      this.palette = readPalette();
      this.draw();
    };
    const themes = new MutationObserver(repaint);
    themes.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme", "class"] });
    this.cleanup.push(() => themes.disconnect());
    const scheme = window.matchMedia("(prefers-color-scheme: dark)");
    scheme.addEventListener("change", repaint);
    this.cleanup.push(() => scheme.removeEventListener("change", repaint));

    // Scrolled out of view counts as hidden.
    const seen = new IntersectionObserver((entries) => {
      if (entries.some((e) => !e.isIntersecting) && this.game.phase === "playing" && !spec.turnBased) {
        this.setPaused(true);
      }
    });
    seen.observe(canvas);
    this.cleanup.push(() => seen.disconnect());

    const size = new ResizeObserver(() => this.fit());
    size.observe(canvas);
    this.cleanup.push(() => size.disconnect());
    this.fit();
  }

  destroy() {
    if (this.frame !== null) cancelAnimationFrame(this.frame);
    this.frame = null;
    this.game.destroy?.();
    for (const undo of this.cleanup) undo();
  }

  /** The canvas's backing store follows its CSS width; height keeps the
   *  game's aspect ratio, which the canvas's own attributes then carry. */
  private fit() {
    const dpr = Math.min(MOST_DPR, window.devicePixelRatio || 1);
    const css = this.canvas.clientWidth || this.spec.width;
    const width = Math.round(css * dpr);
    const height = Math.round((css * dpr * this.spec.height) / this.spec.width);
    if (this.canvas.width !== width || this.canvas.height !== height) {
      this.canvas.width = width;
      this.canvas.height = height;
    }
    this.draw();
  }

  private draw() {
    const scale = this.canvas.width / this.spec.width;
    this.ctx.setTransform(scale, 0, 0, scale, 0, 0);
    this.game.draw(this.ctx, this.palette);
    const view = this.game.view?.();
    if (view !== undefined) this.events.view?.(view);
  }

  private listen(target: EventTarget, type: string, handler: (e: Event) => void) {
    target.addEventListener(type, handler);
    this.cleanup.push(() => target.removeEventListener(type, handler));
  }

  setPaused(paused: boolean) {
    if (this.paused === paused) return;
    this.paused = paused;
    if (paused) this.letGo();
    this.events.paused(paused);
    if (!paused) this.kick();
  }

  /** Drop a pending press and every held key or button. */
  private letGo() {
    this.press = null;
    this.game.release?.();
  }

  restart() {
    this.game.restart();
    this.carry = 0;
    this.after();
  }

  /** After any input: report a phase change and start the loop if it is
   *  needed now. */
  private after() {
    this.report();
    this.kick();
  }

  private report() {
    if (this.game.phase !== this.phase) {
      this.phase = this.game.phase;
      this.events.phase(this.phase);
    }
    this.draw();
  }

  /** Start the loop. Only a loop that was not running starts its clock and
   *  its leftover time afresh: a frame shorter than STEP (a 144 Hz screen)
   *  carries its time into the next one. */
  private kick() {
    if (this.frame !== null || this.paused || this.spec.turnBased) return;
    if (this.game.phase !== "playing") return;
    this.last = performance.now();
    this.carry = 0;
    this.frame = requestAnimationFrame(this.tick);
  }

  private tick = (now: number) => {
    this.frame = null;
    if (this.paused || this.game.phase !== "playing") return;
    this.carry += Math.min(LONGEST_FRAME, Math.max(0, (now - this.last) / 1000));
    this.last = now;
    while (this.carry >= STEP && this.game.phase === "playing") {
      this.game.step?.(STEP);
      this.carry -= STEP;
    }
    this.report();
    if (!this.paused && this.game.phase === "playing") this.frame = requestAnimationFrame(this.tick);
  };

  /** True when the input only woke the game up. */
  private wake(): boolean {
    if (!this.paused) return false;
    this.setPaused(false);
    return true;
  }

  private onKey(e: KeyboardEvent) {
    if (e.altKey || e.ctrlKey || e.metaKey || e.key === "Tab" || e.key === "Escape") return;
    if (e.repeat && (e.key === " " || e.key === "Enter")) {
      e.preventDefault();
      return;
    }
    if (this.wake()) {
      e.preventDefault();
      return;
    }
    if (this.game.phase === "over" && (e.key === " " || e.key === "Enter")) {
      e.preventDefault();
      this.restart();
      return;
    }
    if (this.game.key(e.key)) e.preventDefault();
    this.after();
  }

  private logical(e: PointerEvent): { x: number; y: number } {
    const box = this.canvas.getBoundingClientRect();
    return {
      x: ((e.clientX - box.left) / box.width) * this.spec.width,
      y: ((e.clientY - box.top) / box.height) * this.spec.height,
    };
  }

  private onDown(e: PointerEvent) {
    if (e.button !== 0) return;
    this.canvas.focus({ preventScroll: true });
    if (this.wake()) return;
    const at = this.logical(e);
    if (this.game.phase === "over") {
      this.restart();
      return;
    }
    this.canvas.setPointerCapture?.(e.pointerId);
    this.press = { ...at, id: e.pointerId };
    this.game.point?.(at.x, at.y);
    // A game that tells taps from swipes decides when the pointer comes up.
    if (this.game.swipe === undefined) this.game.tap?.(at.x, at.y);
    this.after();
  }

  private onMove(e: PointerEvent) {
    if (this.paused) return;
    const at = this.logical(e);
    this.game.point?.(at.x, at.y);
    if (this.spec.turnBased) return;
    // Bricks keeps up with the pointer between frames; nothing else needs to.
    if (this.game.phase !== "playing") this.draw();
  }

  private onUp(e: PointerEvent) {
    const press = this.press;
    this.press = null;
    if (press === null || press.id !== e.pointerId) return;
    const at = this.logical(e);
    if (this.game.swipe !== undefined) {
      const dx = at.x - press.x;
      const dy = at.y - press.y;
      if (Math.max(Math.abs(dx), Math.abs(dy)) >= SWIPE) {
        const direction: Direction =
          Math.abs(dx) > Math.abs(dy) ? (dx > 0 ? "right" : "left") : dy > 0 ? "down" : "up";
        this.game.swipe(direction);
      } else {
        this.game.tap?.(at.x, at.y);
      }
    }
    this.game.release?.();
    this.after();
  }
}
