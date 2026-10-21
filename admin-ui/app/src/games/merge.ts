import { mix, roundRect } from "./draw";
import type { Direction, Game, GameHost, GameSpec, Palette, Phase } from "./types";

/* Merge: slide every tile on a four by four board one way; two equal tiles
 * that meet become one of twice the value, and a new 2 (sometimes a 4)
 * lands on an empty square. The round ends when no slide changes anything.
 * The score is the sum of every tile a merge made. Turn-based: nothing
 * moves between inputs, so it is one of the two offered under reduced
 * motion. */

const N = 4;
const GAP = 8;
const TILE = 64;
const SIZE = N * TILE + (N + 1) * GAP;

const KEYS: Record<string, Direction> = {
  ArrowUp: "up",
  ArrowDown: "down",
  ArrowLeft: "left",
  ArrowRight: "right",
  w: "up",
  s: "down",
  a: "left",
  d: "right",
  W: "up",
  S: "down",
  A: "left",
  D: "right",
};

/** One row slid towards index 0: the new row and the points it made. */
export function slideRow(row: number[]): { row: number[]; points: number } {
  const tiles = row.filter((v) => v !== 0);
  const out: number[] = [];
  let points = 0;
  for (let i = 0; i < tiles.length; i++) {
    const tile = tiles[i] ?? 0;
    if (tile === tiles[i + 1]) {
      out.push(tile * 2);
      points += tile * 2;
      i += 1;
    } else {
      out.push(tile);
    }
  }
  while (out.length < row.length) out.push(0);
  return { row: out, points };
}

/* The board is sixteen squares row by row, board[y * N + x]. Each
 * direction reads it as four lines of squares that slide towards their
 * start, and writes them back. */
function lines(direction: Direction): number[][] {
  const result: number[][] = [];
  for (let a = 0; a < N; a++) {
    const line: number[] = [];
    for (let b = 0; b < N; b++) {
      if (direction === "left") line.push(a * N + b);
      if (direction === "right") line.push(a * N + N - 1 - b);
      if (direction === "up") line.push(b * N + a);
      if (direction === "down") line.push((N - 1 - b) * N + a);
    }
    result.push(line);
  }
  return result;
}

export function slide(board: number[], direction: Direction): { board: number[]; points: number; moved: boolean } {
  const next = [...board];
  let points = 0;
  let moved = false;
  for (const line of lines(direction)) {
    const before = line.map((square) => board[square] ?? 0);
    const after = slideRow(before);
    points += after.points;
    line.forEach((square, i) => {
      next[square] = after.row[i] ?? 0;
      if (after.row[i] !== before[i]) moved = true;
    });
  }
  return { board: next, points, moved };
}

class Merge implements Game {
  phase: Phase = "playing";
  private board: number[] = [];
  private points = 0;

  constructor(private readonly host: GameHost) {
    this.reset();
  }

  private reset() {
    this.board = Array<number>(N * N).fill(0);
    this.points = 0;
    this.drop();
    this.drop();
    this.host.score(0);
  }

  restart() {
    this.reset();
    this.phase = "playing";
    this.host.redraw();
  }

  private drop() {
    const free = this.board.flatMap((v, square) => (v === 0 ? [square] : []));
    const square = free[Math.floor(this.host.random() * free.length)];
    if (square !== undefined) this.board[square] = this.host.random() < 0.9 ? 2 : 4;
  }

  private stuck(): boolean {
    return (["up", "down", "left", "right"] as const).every((d) => !slide(this.board, d).moved);
  }

  swipe(direction: Direction) {
    if (this.phase !== "playing") return;
    const result = slide(this.board, direction);
    if (!result.moved) return;
    this.board = result.board;
    this.points += result.points;
    this.drop();
    this.host.score(this.points);
    if (this.stuck()) {
      this.phase = "over";
      this.host.over(this.points);
    }
    this.host.redraw();
  }

  key(key: string) {
    const direction = KEYS[key];
    if (direction === undefined) return false;
    this.swipe(direction);
    return true;
  }

  draw(ctx: CanvasRenderingContext2D, p: Palette) {
    ctx.clearRect(0, 0, SIZE, SIZE);
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    for (let y = 0; y < N; y++) {
      for (let x = 0; x < N; x++) {
        const value = this.board[y * N + x] ?? 0;
        const left = GAP + x * (TILE + GAP);
        const top = GAP + y * (TILE + GAP);
        roundRect(ctx, left, top, TILE, TILE, 10);
        if (value === 0) {
          ctx.fillStyle = p.line;
          ctx.fill();
          continue;
        }
        // 2 is the wash, 256 and beyond the full accent: steep enough that
        // the small tiles, which are most of a game, tell apart.
        const depth = Math.min(1, (Math.log2(value) - 1) / 7);
        ctx.fillStyle = mix(p.accentWash, p.accent, depth);
        ctx.fill();
        ctx.fillStyle = depth > 0.45 ? p.surface : p.ink;
        const digits = String(value).length;
        ctx.font = `600 ${digits <= 2 ? 26 : digits === 3 ? 22 : 18}px ${p.font}`;
        ctx.fillText(String(value), left + TILE / 2, top + TILE / 2 + 1);
      }
    }
  }
}

export const merge: GameSpec = {
  id: "merge",
  turnBased: true,
  width: SIZE,
  height: SIZE,
  lowerIsBetter: false,
  create: (host) => new Merge(host),
};
