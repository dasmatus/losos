import { roundRect, salmon } from "./draw";
import type { Game, GameHost, GameSpec, Palette, Phase } from "./types";

/* Upstream: the salmon swims up a river and leaps the rocks in its way.
 *
 * One input, jump, held for a higher leap and let go for a short hop. The
 * river speeds up as the distance grows; the score is the distance. */

const W = 320;
const H = 140;
const SURFACE = 100;
const FISH_X = 56;
const FISH_LENGTH = 30;
const GRAVITY = 1100;
const LEAP = -360;
const SHORT_HOP = -150;
const START_SPEED = 130;
const TOP_SPEED = 280;
const SPEED_UP = 5;

interface Rock {
  x: number;
  w: number;
  h: number;
}

class Upstream implements Game {
  phase: Phase = "idle";
  private y = SURFACE;
  private vy = 0;
  private holding = false;
  private speed = START_SPEED;
  private distance = 0;
  private rocks: Rock[] = [];
  private nextRock = 0;
  private reported = 0;

  constructor(private readonly host: GameHost) {
    this.reset();
  }

  private reset() {
    this.y = SURFACE;
    this.vy = 0;
    this.speed = START_SPEED;
    this.distance = 0;
    this.rocks = [];
    this.nextRock = W + 40;
    this.reported = 0;
    this.host.score(0);
  }

  restart() {
    this.reset();
    this.phase = "playing";
  }

  private jump() {
    if (this.phase === "idle") this.phase = "playing";
    if (this.phase !== "playing") return;
    this.holding = true;
    if (this.y >= SURFACE - 0.5) this.vy = LEAP;
  }

  key(key: string) {
    if (key === " " || key === "ArrowUp" || key === "w" || key === "W" || key === "Enter") {
      this.jump();
      return true;
    }
    return false;
  }

  keyUp(key: string) {
    if (key === " " || key === "ArrowUp" || key === "w" || key === "W" || key === "Enter") this.release();
  }

  tap() {
    this.jump();
  }

  release() {
    this.holding = false;
    if (this.vy < SHORT_HOP) this.vy = SHORT_HOP;
  }

  step(dt: number) {
    if (this.phase !== "playing") return;
    this.speed = Math.min(TOP_SPEED, this.speed + SPEED_UP * dt);
    const dx = this.speed * dt;
    this.distance += dx;

    // Gravity, lighter while the jump is held on the way up.
    const g = this.holding && this.vy < 0 ? GRAVITY * 0.62 : GRAVITY;
    this.vy += g * dt;
    this.y = Math.min(SURFACE, this.y + this.vy * dt);
    if (this.y >= SURFACE) this.vy = 0;

    for (const rock of this.rocks) rock.x -= dx;
    this.rocks = this.rocks.filter((rock) => rock.x + rock.w > -10);
    this.nextRock -= dx;
    if (this.nextRock <= 0) {
      const tall = this.host.random();
      this.rocks.push({ x: W + 10, w: 14 + this.host.random() * 14, h: 14 + tall * 22 });
      // The gap grows with the speed so every gap can be cleared.
      this.nextRock = 120 + this.speed * 0.5 + this.host.random() * 150;
    }

    const score = Math.floor(this.distance / 10);
    if (score !== this.reported) {
      this.reported = score;
      this.host.score(score);
    }

    // The fish is a little smaller than it looks, so a graze is forgiven.
    const left = FISH_X - FISH_LENGTH * 0.38;
    const right = FISH_X + FISH_LENGTH * 0.38;
    const bottom = this.y + 4;
    for (const rock of this.rocks) {
      const top = SURFACE + 6 - rock.h;
      if (right > rock.x + 3 && left < rock.x + rock.w - 3 && bottom > top + 3) {
        this.phase = "over";
        this.host.over(score);
        return;
      }
    }
  }

  draw(ctx: CanvasRenderingContext2D, p: Palette) {
    ctx.clearRect(0, 0, W, H);

    // The rocks stand in the river, so the water is drawn over their feet.
    for (const rock of this.rocks) {
      ctx.fillStyle = p.faint;
      roundRect(ctx, rock.x, SURFACE + 6 - rock.h, rock.w, rock.h + 20, 6);
      ctx.fill();
    }

    ctx.fillStyle = p.accentWash;
    ctx.fillRect(0, SURFACE + 6, W, H - SURFACE - 6);

    // The surface: a slow wave that travels with the river.
    ctx.strokeStyle = p.accent;
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    for (let x = 0; x <= W; x += 8) {
      const y = SURFACE + 6 + Math.sin((x + this.distance) / 18) * 1.5;
      if (x === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    ctx.stroke();

    // Ripples under the surface, moving at half the river's speed.
    ctx.strokeStyle = p.line;
    ctx.lineWidth = 1;
    for (let i = 0; i < 6; i++) {
      const x = (((i * 71 - this.distance * 0.5) % (W + 40)) + W + 40) % (W + 40) - 20;
      const y = SURFACE + 16 + (i % 3) * 7;
      ctx.beginPath();
      ctx.moveTo(x, y);
      ctx.lineTo(x + 14, y);
      ctx.stroke();
    }

    const tilt = Math.max(-0.5, Math.min(0.5, this.vy / 700));
    salmon(ctx, p, FISH_X, this.y, FISH_LENGTH, tilt);
  }
}

export const upstream: GameSpec = {
  id: "upstream",
  turnBased: false,
  width: W,
  height: H,
  lowerIsBetter: false,
  create: (host) => new Upstream(host),
};
