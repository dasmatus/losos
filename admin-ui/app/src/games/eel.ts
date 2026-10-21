import { roundRect } from "./draw";
import type { Direction, Game, GameHost, GameSpec, Palette, Phase } from "./types";

/* Eel: steer an eel round a pond, eat the shrimp, grow, and keep off the
 * walls and your own tail. It moves a cell per tick and goes a little
 * faster with every shrimp. */

const COLS = 20;
const ROWS = 12;
const CELL = 16;
const W = COLS * CELL;
const H = ROWS * CELL;
const START_TICK = 0.15;
const FASTEST_TICK = 0.075;

interface Cell {
  x: number;
  y: number;
}

const STEP: Record<Direction, Cell> = {
  up: { x: 0, y: -1 },
  down: { x: 0, y: 1 },
  left: { x: -1, y: 0 },
  right: { x: 1, y: 0 },
};

const OPPOSITE: Record<Direction, Direction> = { up: "down", down: "up", left: "right", right: "left" };

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

class Eel implements Game {
  phase: Phase = "idle";
  private body: Cell[] = [];
  private heading: Direction = "right";
  private turns: Direction[] = [];
  private shrimp: Cell = { x: 0, y: 0 };
  private clock = 0;
  private tick = START_TICK;
  private eaten = 0;

  constructor(private readonly host: GameHost) {
    this.reset();
  }

  private reset() {
    const y = Math.floor(ROWS / 2);
    this.body = [
      { x: 6, y },
      { x: 5, y },
      { x: 4, y },
    ];
    this.heading = "right";
    this.turns = [];
    this.clock = 0;
    this.tick = START_TICK;
    this.eaten = 0;
    this.placeShrimp();
    this.host.score(0);
  }

  restart() {
    this.reset();
    this.phase = "playing";
  }

  private placeShrimp() {
    const free: Cell[] = [];
    for (let x = 0; x < COLS; x++) {
      for (let y = 0; y < ROWS; y++) {
        if (!this.body.some((c) => c.x === x && c.y === y)) free.push({ x, y });
      }
    }
    this.shrimp = free[Math.floor(this.host.random() * free.length)] ?? { x: 0, y: 0 };
  }

  private turn(direction: Direction) {
    if (this.phase === "idle") this.phase = "playing";
    if (this.phase !== "playing") return;
    const last = this.turns.at(-1) ?? this.heading;
    if (direction === last || direction === OPPOSITE[last]) return;
    // Two queued turns let a quick U-turn land; more would lag behind.
    if (this.turns.length < 2) this.turns.push(direction);
  }

  key(key: string) {
    const direction = KEYS[key];
    if (direction !== undefined) {
      this.turn(direction);
      return true;
    }
    if (key === " " || key === "Enter") {
      if (this.phase === "idle") this.phase = "playing";
      return true;
    }
    return false;
  }

  swipe(direction: Direction) {
    this.turn(direction);
  }

  /* A tap turns the eel towards it: up or down when it is going sideways,
   * left or right when it is going up or down. */
  tap(x: number, y: number) {
    if (this.phase === "idle") {
      this.phase = "playing";
      return;
    }
    const head = this.body[0]!;
    const cx = head.x * CELL + CELL / 2;
    const cy = head.y * CELL + CELL / 2;
    const going = this.turns.at(-1) ?? this.heading;
    if (going === "left" || going === "right") this.turn(y < cy ? "up" : "down");
    else this.turn(x < cx ? "left" : "right");
  }

  step(dt: number) {
    if (this.phase !== "playing") return;
    this.clock += dt;
    while (this.clock >= this.tick && this.phase === "playing") {
      this.clock -= this.tick;
      this.advance();
    }
  }

  private advance() {
    const next = this.turns.shift();
    if (next !== undefined) this.heading = next;
    const head = this.body[0]!;
    const move = STEP[this.heading];
    const to = { x: head.x + move.x, y: head.y + move.y };
    const eats = to.x === this.shrimp.x && to.y === this.shrimp.y;
    // The tail moves out of the way this tick unless the eel grows.
    const rest = eats ? this.body : this.body.slice(0, -1);
    const out = to.x < 0 || to.y < 0 || to.x >= COLS || to.y >= ROWS;
    if (out || rest.some((c) => c.x === to.x && c.y === to.y)) {
      this.phase = "over";
      this.host.over(this.eaten);
      return;
    }
    this.body = [to, ...rest];
    if (eats) {
      this.eaten += 1;
      this.tick = Math.max(FASTEST_TICK, this.tick * 0.97);
      this.host.score(this.eaten);
      this.placeShrimp();
    }
  }

  draw(ctx: CanvasRenderingContext2D, p: Palette) {
    ctx.clearRect(0, 0, W, H);

    // The pond's floor: a dot per cell, faint enough to read past.
    ctx.fillStyle = p.line;
    for (let x = 0; x < COLS; x++) {
      for (let y = 0; y < ROWS; y++) {
        ctx.fillRect(x * CELL + CELL / 2 - 0.75, y * CELL + CELL / 2 - 0.75, 1.5, 1.5);
      }
    }

    // The shrimp: a curl and a tail.
    const sx = this.shrimp.x * CELL + CELL / 2;
    const sy = this.shrimp.y * CELL + CELL / 2;
    ctx.strokeStyle = p.warn;
    ctx.lineWidth = 3;
    ctx.lineCap = "round";
    ctx.beginPath();
    ctx.arc(sx, sy, 4, Math.PI * 0.15, Math.PI * 1.45);
    ctx.stroke();
    ctx.lineCap = "butt";

    this.body.forEach((cell, index) => {
      const inset = index === 0 ? 1 : 2;
      ctx.fillStyle = index === 0 ? p.accent : index % 2 === 0 ? p.accent : p.ok;
      roundRect(ctx, cell.x * CELL + inset, cell.y * CELL + inset, CELL - inset * 2, CELL - inset * 2, 5);
      ctx.fill();
    });

    // Eyes on the head, on the side it is going.
    const head = this.body[0]!;
    const move = STEP[this.heading];
    const hx = head.x * CELL + CELL / 2 + move.x * 3;
    const hy = head.y * CELL + CELL / 2 + move.y * 3;
    ctx.fillStyle = p.surface;
    for (const side of [-1, 1]) {
      ctx.beginPath();
      ctx.arc(hx + move.y * side * 3, hy + move.x * side * 3, 1.6, 0, Math.PI * 2);
      ctx.fill();
    }
  }
}

export const eel: GameSpec = {
  id: "eel",
  turnBased: false,
  width: W,
  height: H,
  lowerIsBetter: false,
  create: (host) => new Eel(host),
};
