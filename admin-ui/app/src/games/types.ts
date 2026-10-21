/* The contract between the arcade (arcade.tsx) and the five games.
 *
 * A game is a plain object over a canvas: it never touches the DOM, the
 * network or storage, and it draws in logical units that the arcade scales
 * to the canvas's real size. The arcade owns the frame loop, the input,
 * pausing, the scores and every word on screen, so a game is only its rules
 * and its picture. */

export type GameId = "upstream" | "eel" | "merge" | "bricks" | "pairs";

export type Direction = "up" | "down" | "left" | "right";

/** The box's palette, read from tokens.css so the games follow the theme. */
export interface Palette {
  surface: string;
  sunk: string;
  ink: string;
  muted: string;
  faint: string;
  line: string;
  accent: string;
  accentWash: string;
  ok: string;
  warn: string;
  font: string;
}

export type Phase = "idle" | "playing" | "over";

export interface GameHost {
  /** The round's score changed. */
  score(value: number): void;
  /** The round ended with this score. */
  over(final: number): void;
  /** Draw one frame: a turn-based game changed after an input or a timer. */
  redraw(): void;
  /** In [0, 1); the arcade hands in Math.random. */
  random(): number;
}

export interface Game {
  readonly phase: Phase;
  /** Start a new round. The input that asked for it starts it too. */
  restart(): void;
  /** A key went down (KeyboardEvent.key). True when the game used it. */
  key(key: string): boolean;
  /** A key came up. */
  keyUp?(key: string): void;
  /** A tap or click at a logical point. */
  tap?(x: number, y: number): void;
  /** A swipe or drag across the canvas. */
  swipe?(direction: Direction): void;
  /** The pointer is over the canvas at this logical point. */
  point?(x: number, y: number): void;
  /** The pointer or key that a tap or a jump started came up. */
  release?(): void;
  /** Advance by `dt` seconds. Only the action games have it. */
  step?(dt: number): void;
  draw(ctx: CanvasRenderingContext2D, palette: Palette): void;
  /** Drop any timer: the game is leaving the page. */
  destroy?(): void;
}

export interface GameSpec {
  id: GameId;
  /** Nothing moves between inputs. Only these are offered under reduced
   *  motion, and they draw on demand rather than every frame. */
  turnBased: boolean;
  /** The logical canvas, which keeps its aspect ratio at any width. */
  width: number;
  height: number;
  /** The score is a count of moves, and fewer is better. */
  lowerIsBetter: boolean;
  create(host: GameHost): Game;
}
