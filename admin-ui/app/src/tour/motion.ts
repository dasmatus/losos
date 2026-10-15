/* The tour's motion system: every curve, envelope and helper the scenes use.
 *
 * The scenes are drawn per frame, as pure functions of a frame number
 * (scenes.tsx), and these are the only easings they take. One small set,
 * named for how each one feels, is what keeps six scenes moving like one
 * piece rather than six. The curves and helpers follow cinetic's house
 * motion tokens (github.com/Leonxlnx/cinetic, MIT, Copyright (c) 2026
 * Leonxlnx), rewritten here without Remotion, which the admin page cannot
 * load: its player injects a <style> element, and `style-src 'self'`
 * refuses it.
 *
 * Rules the scenes keep: entrances decay and exits accelerate, scale moves in
 * log space (lmix), every value that feeds geometry is clamped, and discrete
 * state (a typed letter, a clock reading) is decided on a whole frame (fd). */

export type Ease = (t: number) => number;

export const FPS = 60;
/** 120 BPM at 60 fps: a beat is 30 frames, a bar 120. Scenes are whole bars. */
export const BEAT = 30;
export const BAR = BEAT * 4;

/* A CSS-style cubic bezier from (0,0) to (1,1): solve x(s) = t for s by
 * Newton's method, falling back to bisection where the slope is flat, then
 * read y(s). */
export function bezier(x1: number, y1: number, x2: number, y2: number): Ease {
  const cx = 3 * x1;
  const bx = 3 * (x2 - x1) - cx;
  const ax = 1 - cx - bx;
  const cy = 3 * y1;
  const by = 3 * (y2 - y1) - cy;
  const ay = 1 - cy - by;
  const x = (s: number) => ((ax * s + bx) * s + cx) * s;
  const dx = (s: number) => (3 * ax * s + 2 * bx) * s + cx;
  const y = (s: number) => ((ay * s + by) * s + cy) * s;
  return (t: number) => {
    if (t <= 0) return 0;
    if (t >= 1) return 1;
    let s = t;
    for (let i = 0; i < 8; i++) {
      const err = x(s) - t;
      if (Math.abs(err) < 1e-6) return y(s);
      const slope = dx(s);
      if (Math.abs(slope) < 1e-6) break;
      s -= err / slope;
    }
    let lo = 0;
    let hi = 1;
    s = t;
    for (let i = 0; i < 30; i++) {
      if (x(s) < t) lo = s;
      else hi = s;
      s = (lo + hi) / 2;
    }
    return y(s);
  };
}

/** The easing tokens. A scene uses a handful of them and nothing else. */
export const E = {
  out: bezier(0.16, 1, 0.3, 1), // arrivals and reveals, never a fade-out
  outSoft: bezier(0.22, 1, 0.36, 1), // gentle settles
  exit: bezier(0.55, 0.055, 0.675, 0.19), // leaving: accelerates away
  inOut: bezier(0.87, 0, 0.13, 1), // a container growing and shrinking back
  smooth: bezier(0.65, 0, 0.35, 1), // fades and drifts
  ui: bezier(0.4, 0, 0.2, 1), // small state changes: a switch, a highlight
  glide: bezier(0.47, 0.2, 0.15, 1), // 150-350 px moves with a soft landing
  linear: (t: number) => t, // drift only
} satisfies Record<string, Ease>;

export const clamp = (v: number, lo = 0, hi = 1): number => Math.min(hi, Math.max(lo, v));
export const mix = (a: number, b: number, t: number): number => a + (b - a) * t;
/** Log-space mix for scale: equal ratios per frame, so a zoom never seems to speed up. */
export const lmix = (a: number, b: number, t: number): number => Math.exp(mix(Math.log(a), Math.log(b), t));

/** 0..1 between two frames, clamped at both ends. It is 0 on `from`. */
export function prog(frame: number, from: number, to: number, ease: Ease = E.out): number {
  if (to <= from) return frame >= to ? 1 : 0;
  return ease(clamp((frame - from) / (to - from)));
}

/** In and back out: 0 before `inAt`, 1 from `inAt + dur` until `outAt`, 0 again `dur` after. */
export function hold(frame: number, inAt: number, outAt: number, dur = 18, ease: Ease = E.smooth): number {
  return prog(frame, inAt, inAt + dur, ease) * (1 - prog(frame, outAt, outAt + dur, ease));
}

/** Attack-decay envelope for a press or a tick, `t` frames after it: no velocity step at either end. */
export function hitPulse(t: number, attack = 2, tau = 5): number {
  if (t <= 0 || t > attack + 6 * tau) return 0;
  return t < attack ? Math.sin((t / attack) * (Math.PI / 2)) : Math.exp(-(t - attack) / tau);
}

/** The whole frame discrete state is decided on. */
export const fd = Math.round;
