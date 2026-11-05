/* The assistant's face in a video call.
 *
 * A call sends frames, so an assistant that is in one with a camera on needs
 * something to send. This draws a face on a canvas and hands its stream to
 * the call as the assistant's camera: it blinks, breathes, looks up while it
 * thinks, and its mouth follows the loudness of what the voice is saying. The
 * pictures are made in the page and sealed like every other frame; nothing
 * is rendered on the box.
 *
 * The drawing is a pure function of a clock, a phase and a level (`paint`),
 * so a test can ask for any moment of it without waiting. */

import type { Phase } from "./agent";

export const WIDTH = 480;
export const HEIGHT = 360;
const FPS = 24;

export interface Look {
  /* Seconds since the avatar started. */
  time: number;
  phase: Phase;
  /* 0 to 1: how loud the voice is right now. */
  level: number;
  name: string;
}

/* The palette is the app's, read once; the fallbacks are the light theme. */
interface Palette {
  skin: string;
  ink: string;
  glow: string;
  night: string;
  deep: string;
}

const FALLBACK: Palette = { skin: "#f4876f", ink: "#241512", glow: "72, 179, 192", night: "#12232c", deep: "#0a1419" };

function palette(): Palette {
  try {
    const css = getComputedStyle(document.documentElement);
    const glow = css.getPropertyValue("--accent-rgb").trim();
    return { ...FALLBACK, ...(glow !== "" ? { glow } : {}) };
  } catch {
    return FALLBACK;
  }
}

/* Whether the eyes are shut at `time`: a blink every few seconds, never at
 * the same beat twice. */
export function blinking(time: number): boolean {
  const beat = 3.4 + 0.9 * Math.sin(Math.floor(time / 3.4) * 2.7);
  return time % beat < 0.14;
}

/* Draw one frame. Everything is relative to the canvas, so any size works. */
export function paint(g: CanvasRenderingContext2D, look: Look, colours: Palette = FALLBACK): void {
  const w = g.canvas.width;
  const h = g.canvas.height;
  const u = Math.min(w, h) / 360;
  const cx = w / 2;
  const cy = h * 0.46;

  const back = g.createLinearGradient(0, 0, 0, h);
  back.addColorStop(0, colours.night);
  back.addColorStop(1, colours.deep);
  g.fillStyle = back;
  g.fillRect(0, 0, w, h);

  // A glow behind the head that swells with the voice, and a quiet breath
  // when it is only listening.
  const breath = 0.5 + 0.5 * Math.sin(look.time * 1.6);
  const swell = look.phase === "speaking" ? look.level : look.phase === "listening" ? 0.15 * breath : 0.05;
  const radius = (150 + swell * 70) * u;
  const aura = g.createRadialGradient(cx, cy, 60 * u, cx, cy, radius);
  aura.addColorStop(0, `rgba(${colours.glow}, ${0.28 + swell * 0.35})`);
  aura.addColorStop(1, `rgba(${colours.glow}, 0)`);
  g.fillStyle = aura;
  g.beginPath();
  g.arc(cx, cy, radius, 0, Math.PI * 2);
  g.fill();

  // The head.
  const bob = look.phase === "speaking" ? Math.sin(look.time * 5) * 2.5 * u * look.level : Math.sin(look.time * 1.6) * 2 * u;
  const hy = cy + bob;
  g.fillStyle = colours.skin;
  g.beginPath();
  g.arc(cx, hy, 96 * u, 0, Math.PI * 2);
  g.fill();
  g.fillStyle = "rgba(255, 255, 255, 0.16)";
  g.beginPath();
  g.ellipse(cx - 30 * u, hy - 52 * u, 46 * u, 26 * u, -0.5, 0, Math.PI * 2);
  g.fill();

  // Eyes: up and to the side while it thinks.
  const gaze = look.phase === "thinking" ? { x: 5 * u, y: -7 * u } : { x: 0, y: 0 };
  const shut = blinking(look.time);
  g.fillStyle = colours.ink;
  for (const side of [-1, 1]) {
    g.beginPath();
    g.ellipse(cx + side * 33 * u + gaze.x, hy - 14 * u + gaze.y, 9 * u, shut ? 1.6 * u : 12 * u, 0, 0, Math.PI * 2);
    g.fill();
  }

  // The mouth: a line at rest, open as far as the voice is loud.
  g.fillStyle = colours.ink;
  if (look.phase === "thinking") {
    for (let i = 0; i < 3; i++) {
      const lift = Math.max(0, Math.sin(look.time * 6 - i * 0.9)) * 7 * u;
      g.beginPath();
      g.arc(cx + (i - 1) * 20 * u, hy + 44 * u - lift, 5.5 * u, 0, Math.PI * 2);
      g.fill();
    }
  } else {
    const open = look.phase === "speaking" ? look.level : 0;
    const mw = (40 + open * 14) * u;
    const mh = (4 + open * 38) * u;
    g.beginPath();
    g.roundRect(cx - mw / 2, hy + 34 * u - mh / 2 + mh * 0.2, mw, mh, Math.min(mh / 2, 18 * u));
    g.fill();
  }

  // Its name, under the face.
  g.fillStyle = "rgba(255, 255, 255, 0.92)";
  g.font = `500 ${Math.round(20 * u)}px system-ui, sans-serif`;
  g.textAlign = "center";
  g.textBaseline = "alphabetic";
  g.fillText(look.name, cx, h - 26 * u);
}

export class Avatar {
  readonly stream: MediaStream;
  private canvas: HTMLCanvasElement;
  private g: CanvasRenderingContext2D;
  private timer: ReturnType<typeof setInterval>;
  private started = performance.now();
  private shown = 0;
  private colours = palette();
  private phase: Phase = "loading";

  constructor(
    private name: string,
    /* How loud the voice is, 0 to 1. */
    private level: () => number,
  ) {
    this.canvas = document.createElement("canvas");
    this.canvas.width = WIDTH;
    this.canvas.height = HEIGHT;
    const g = this.canvas.getContext("2d");
    if (g === null) throw new Error("no canvas");
    this.g = g;
    this.draw();
    this.stream = this.canvas.captureStream(FPS);
    // A timer, not requestAnimationFrame: a call in a tab the person has put
    // behind another one still sends its picture.
    this.timer = setInterval(() => this.draw(), 1000 / FPS);
  }

  setPhase(phase: Phase): void {
    this.phase = phase;
  }

  private draw(): void {
    // The voice's loudness rises fast and falls slowly, like a mouth.
    const now = this.level();
    this.shown = now > this.shown ? this.shown + (now - this.shown) * 0.6 : this.shown + (now - this.shown) * 0.25;
    paint(this.g, { time: (performance.now() - this.started) / 1000, phase: this.phase, level: this.shown, name: this.name }, this.colours);
  }

  close(): void {
    clearInterval(this.timer);
    for (const t of this.stream.getTracks()) t.stop();
  }
}
