import { roundRect, salmon } from "./draw";
import type { Direction, Game, GameHost, GameSpec, Palette, Phase } from "./types";

/* Pairs: sixteen cards face down, eight shapes twice. Turn two over; a
 * pair stays up, anything else turns back. The score is how many tries it
 * took, so fewer is better. Turn-based, and one of the two games offered
 * under reduced motion: a card turns at once, with no flip animation. */

const N = 4;
const GAP = 8;
const CARD = 64;
const SIZE = N * CARD + (N + 1) * GAP;
const SHOW_MS = 800;
const SHAPES = 8;

const KEYS: Record<string, Direction> = {
  ArrowUp: "up",
  ArrowDown: "down",
  ArrowLeft: "left",
  ArrowRight: "right",
};

class Pairs implements Game {
  phase: Phase = "playing";
  private cards: number[] = [];
  private matched: boolean[] = [];
  private up: number[] = [];
  private tries = 0;
  private cursor = 0;
  /** The keyboard cursor is drawn only once the keyboard has been used. */
  private keyboard = false;
  private timer: ReturnType<typeof setTimeout> | null = null;

  constructor(private readonly host: GameHost) {
    this.reset();
  }

  private reset() {
    if (this.timer !== null) clearTimeout(this.timer);
    this.timer = null;
    const deck = Array.from({ length: SHAPES * 2 }, (_, i) => i % SHAPES);
    for (let i = deck.length - 1; i > 0; i--) {
      const j = Math.floor(this.host.random() * (i + 1));
      [deck[i], deck[j]] = [deck[j]!, deck[i]!];
    }
    this.cards = deck;
    this.matched = deck.map(() => false);
    this.up = [];
    this.tries = 0;
    this.host.score(0);
  }

  restart() {
    this.reset();
    this.phase = "playing";
    this.host.redraw();
  }

  destroy() {
    if (this.timer !== null) clearTimeout(this.timer);
    this.timer = null;
  }

  private turnBack() {
    if (this.timer !== null) clearTimeout(this.timer);
    this.timer = null;
    this.up = [];
  }

  private flip(index: number) {
    if (this.phase !== "playing" || this.matched[index] || this.up.includes(index)) return;
    // A third card while two are still showing turns them back first.
    if (this.up.length === 2) this.turnBack();
    this.up.push(index);
    if (this.up.length === 2) {
      this.tries += 1;
      this.host.score(this.tries);
      const [a, b] = this.up as [number, number];
      if (this.cards[a] === this.cards[b]) {
        this.matched[a] = true;
        this.matched[b] = true;
        this.up = [];
        if (this.matched.every(Boolean)) {
          this.phase = "over";
          this.host.over(this.tries);
        }
      } else {
        this.timer = setTimeout(() => {
          this.turnBack();
          this.host.redraw();
        }, SHOW_MS);
      }
    }
    this.host.redraw();
  }

  key(key: string) {
    const direction = KEYS[key];
    if (direction !== undefined) {
      this.keyboard = true;
      const x = this.cursor % N;
      const y = Math.floor(this.cursor / N);
      if (direction === "left") this.cursor = y * N + ((x + N - 1) % N);
      if (direction === "right") this.cursor = y * N + ((x + 1) % N);
      if (direction === "up") this.cursor = ((y + N - 1) % N) * N + x;
      if (direction === "down") this.cursor = ((y + 1) % N) * N + x;
      this.host.redraw();
      return true;
    }
    if (key === " " || key === "Enter") {
      this.keyboard = true;
      this.flip(this.cursor);
      return true;
    }
    return false;
  }

  tap(x: number, y: number) {
    const col = Math.floor((x - GAP / 2) / (CARD + GAP));
    const row = Math.floor((y - GAP / 2) / (CARD + GAP));
    if (col < 0 || row < 0 || col >= N || row >= N) return;
    this.keyboard = false;
    this.cursor = row * N + col;
    this.flip(this.cursor);
  }

  draw(ctx: CanvasRenderingContext2D, p: Palette) {
    ctx.clearRect(0, 0, SIZE, SIZE);
    this.cards.forEach((shape, i) => {
      const left = GAP + (i % N) * (CARD + GAP);
      const top = GAP + Math.floor(i / N) * (CARD + GAP);
      const open = this.matched[i] || this.up.includes(i);
      roundRect(ctx, left, top, CARD, CARD, 10);
      ctx.fillStyle = open ? (this.matched[i] ? p.accentWash : p.surface) : p.line;
      ctx.fill();
      if (open) {
        ctx.strokeStyle = this.matched[i] ? p.accentWash : p.accent;
        ctx.lineWidth = 1.5;
        ctx.stroke();
        drawShape(ctx, p, shape, left + CARD / 2, top + CARD / 2);
      } else {
        // The back: three dots, like the box's own mark for "more".
        ctx.fillStyle = p.faint;
        for (const dx of [-8, 0, 8]) {
          ctx.beginPath();
          ctx.arc(left + CARD / 2 + dx, top + CARD / 2, 2, 0, Math.PI * 2);
          ctx.fill();
        }
      }
      if (this.keyboard && i === this.cursor) {
        roundRect(ctx, left - 3, top - 3, CARD + 6, CARD + 6, 12);
        ctx.strokeStyle = p.ink;
        ctx.lineWidth = 2;
        ctx.stroke();
      }
    });
  }
}

function drawShape(ctx: CanvasRenderingContext2D, p: Palette, shape: number, x: number, y: number) {
  const r = 15;
  ctx.fillStyle = p.accent;
  ctx.strokeStyle = p.accent;
  ctx.lineWidth = 4;
  ctx.beginPath();
  switch (shape) {
    case 0: // disc
      ctx.arc(x, y, r, 0, Math.PI * 2);
      ctx.fill();
      return;
    case 1: // ring
      ctx.arc(x, y, r - 2, 0, Math.PI * 2);
      ctx.stroke();
      return;
    case 2: // square
      roundRect(ctx, x - r + 2, y - r + 2, (r - 2) * 2, (r - 2) * 2, 4);
      ctx.fill();
      return;
    case 3: // triangle
      ctx.moveTo(x, y - r);
      ctx.lineTo(x + r, y + r * 0.8);
      ctx.lineTo(x - r, y + r * 0.8);
      ctx.closePath();
      ctx.fill();
      return;
    case 4: // diamond
      ctx.moveTo(x, y - r);
      ctx.lineTo(x + r * 0.8, y);
      ctx.lineTo(x, y + r);
      ctx.lineTo(x - r * 0.8, y);
      ctx.closePath();
      ctx.fill();
      return;
    case 5: // plus
      ctx.lineWidth = 7;
      ctx.moveTo(x - r, y);
      ctx.lineTo(x + r, y);
      ctx.moveTo(x, y - r);
      ctx.lineTo(x, y + r);
      ctx.stroke();
      return;
    case 6: // star
      for (let i = 0; i < 10; i++) {
        const angle = -Math.PI / 2 + (i * Math.PI) / 5;
        const radius = i % 2 === 0 ? r + 1 : r * 0.45;
        ctx.lineTo(x + Math.cos(angle) * radius, y + Math.sin(angle) * radius);
      }
      ctx.closePath();
      ctx.fill();
      return;
    default: // the salmon
      salmon(ctx, p, x, y, r * 2.6);
  }
}

export const pairs: GameSpec = {
  id: "pairs",
  turnBased: true,
  width: SIZE,
  height: SIZE,
  lowerIsBetter: true,
  create: (host) => new Pairs(host),
};
