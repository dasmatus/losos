import { roundRect } from "./draw";
import type { Game, GameHost, GameSpec, Palette, Phase } from "./types";

/* Bricks: keep a ball in play with a paddle and knock out the wall above.
 * Three balls a round; a cleared wall comes back a little faster. The
 * paddle follows the pointer, or the arrow keys; Space or a tap serves. */

const W = 320;
const H = 200;
const COLS = 8;
const ROWS = 5;
const BRICK_W = 34;
const BRICK_H = 10;
const BRICK_GAP = 4;
const WALL_TOP = 26;
const WALL_LEFT = (W - (COLS * BRICK_W + (COLS - 1) * BRICK_GAP)) / 2;
const PADDLE_W = 52;
const PADDLE_H = 6;
const PADDLE_Y = H - 16;
const PADDLE_SPEED = 260;
const BALL_R = 4;
const START_SPEED = 170;
const BALLS = 3;

class Bricks implements Game {
  phase: Phase = "idle";
  private paddle = W / 2;
  private steer = 0;
  private held = new Set<string>();
  private ball = { x: W / 2, y: PADDLE_Y - BALL_R, vx: 0, vy: 0 };
  private served = false;
  private wall: boolean[] = [];
  private balls = BALLS;
  private points = 0;
  private level = 0;

  constructor(private readonly host: GameHost) {
    this.reset();
  }

  private reset() {
    this.balls = BALLS;
    this.points = 0;
    this.level = 0;
    this.wall = Array<boolean>(COLS * ROWS).fill(true);
    this.paddle = W / 2;
    this.served = false;
    this.host.score(0);
  }

  restart() {
    this.reset();
    this.phase = "playing";
  }

  private speed() {
    return START_SPEED * (1 + this.level * 0.12);
  }

  private serve() {
    if (this.phase === "idle") this.phase = "playing";
    if (this.phase !== "playing" || this.served) return;
    this.served = true;
    const angle = -Math.PI / 2 + (this.host.random() - 0.5) * 0.8;
    this.ball.vx = Math.cos(angle) * this.speed();
    this.ball.vy = Math.sin(angle) * this.speed();
  }

  key(key: string) {
    if (key === " " || key === "Enter" || key === "ArrowUp") {
      this.serve();
      return true;
    }
    if (key === "ArrowLeft" || key === "ArrowRight" || key === "a" || key === "d" || key === "A" || key === "D") {
      this.held.add(key.toLowerCase());
      if (this.phase === "idle") this.phase = "playing";
      return true;
    }
    return false;
  }

  keyUp(key: string) {
    this.held.delete(key.toLowerCase());
  }

  release() {
    this.held.clear();
  }

  point(x: number) {
    this.paddle = Math.min(W - PADDLE_W / 2, Math.max(PADDLE_W / 2, x));
    // A ball waiting to be served rides on the paddle.
    if (!this.served) this.ball.x = this.paddle;
  }

  tap(x: number) {
    this.point(x);
    this.serve();
  }

  step(dt: number) {
    if (this.phase !== "playing") return;
    const left = this.held.has("arrowleft") || this.held.has("a");
    const right = this.held.has("arrowright") || this.held.has("d");
    this.steer = (right ? 1 : 0) - (left ? 1 : 0);
    if (this.steer !== 0) this.point(this.paddle + this.steer * PADDLE_SPEED * dt);

    if (!this.served) {
      this.ball.x = this.paddle;
      this.ball.y = PADDLE_Y - BALL_R - 1;
      return;
    }

    // Small steps, so a fast ball cannot pass through a brick.
    const parts = Math.ceil((Math.hypot(this.ball.vx, this.ball.vy) * dt) / 3);
    for (let i = 0; i < parts && this.phase === "playing" && this.served; i++) this.move(dt / parts);
  }

  private move(dt: number) {
    const b = this.ball;
    b.x += b.vx * dt;
    b.y += b.vy * dt;

    if (b.x < BALL_R) {
      b.x = BALL_R;
      b.vx = Math.abs(b.vx);
    } else if (b.x > W - BALL_R) {
      b.x = W - BALL_R;
      b.vx = -Math.abs(b.vx);
    }
    if (b.y < BALL_R) {
      b.y = BALL_R;
      b.vy = Math.abs(b.vy);
    }

    // The paddle: where it lands on the paddle decides the angle back.
    if (b.vy > 0 && b.y + BALL_R >= PADDLE_Y && b.y + BALL_R <= PADDLE_Y + PADDLE_H + 4) {
      const offset = (b.x - this.paddle) / (PADDLE_W / 2);
      if (Math.abs(offset) <= 1.1) {
        const angle = -Math.PI / 2 + Math.max(-1, Math.min(1, offset)) * 1.05;
        const speed = this.speed();
        b.vx = Math.cos(angle) * speed;
        b.vy = Math.sin(angle) * speed;
        b.y = PADDLE_Y - BALL_R;
      }
    }

    if (b.y > H + BALL_R) {
      this.balls -= 1;
      this.served = false;
      if (this.balls <= 0) {
        this.phase = "over";
        this.host.over(this.points);
      }
      return;
    }

    for (let i = 0; i < this.wall.length; i++) {
      if (!this.wall[i]) continue;
      const bx = WALL_LEFT + (i % COLS) * (BRICK_W + BRICK_GAP);
      const by = WALL_TOP + Math.floor(i / COLS) * (BRICK_H + BRICK_GAP);
      const nx = Math.max(bx, Math.min(b.x, bx + BRICK_W));
      const ny = Math.max(by, Math.min(b.y, by + BRICK_H));
      if ((b.x - nx) ** 2 + (b.y - ny) ** 2 > BALL_R * BALL_R) continue;
      this.wall[i] = false;
      // Bounce on the side it came through: the shallower overlap.
      const overlapX = Math.min(b.x + BALL_R - bx, bx + BRICK_W - (b.x - BALL_R));
      const overlapY = Math.min(b.y + BALL_R - by, by + BRICK_H - (b.y - BALL_R));
      if (overlapX < overlapY) b.vx = -b.vx;
      else b.vy = -b.vy;
      // The top rows are worth more.
      this.points += (ROWS - Math.floor(i / COLS)) * 10;
      this.host.score(this.points);
      if (this.wall.every((brick) => !brick)) {
        this.level += 1;
        this.wall.fill(true);
        this.served = false;
      }
      break;
    }
  }

  draw(ctx: CanvasRenderingContext2D, p: Palette) {
    ctx.clearRect(0, 0, W, H);

    for (let i = 0; i < this.wall.length; i++) {
      if (!this.wall[i]) continue;
      const row = Math.floor(i / COLS);
      ctx.globalAlpha = 1 - row * 0.15;
      ctx.fillStyle = p.accent;
      roundRect(ctx, WALL_LEFT + (i % COLS) * (BRICK_W + BRICK_GAP), WALL_TOP + row * (BRICK_H + BRICK_GAP), BRICK_W, BRICK_H, 3);
      ctx.fill();
    }
    ctx.globalAlpha = 1;

    // Balls left, top right.
    ctx.fillStyle = p.faint;
    for (let i = 0; i < this.balls; i++) {
      ctx.beginPath();
      ctx.arc(W - 10 - i * 10, 12, 3, 0, Math.PI * 2);
      ctx.fill();
    }

    ctx.fillStyle = p.ink;
    roundRect(ctx, this.paddle - PADDLE_W / 2, PADDLE_Y, PADDLE_W, PADDLE_H, 3);
    ctx.fill();

    ctx.fillStyle = p.warn;
    ctx.beginPath();
    ctx.arc(this.ball.x, this.ball.y, BALL_R, 0, Math.PI * 2);
    ctx.fill();
  }
}

export const bricks: GameSpec = {
  id: "bricks",
  turnBased: false,
  width: W,
  height: H,
  lowerIsBetter: false,
  create: (host) => new Bricks(host),
};
