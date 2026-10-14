/* The frame clock the scenes are drawn from.
 *
 * Every scene is a pure function of one integer, its frame (scenes.tsx).
 * This is what turns wall time into that integer: requestAnimationFrame,
 * 60 frames to the second whatever the display's own rate, looped over the
 * scene's length. Nothing in a scene runs a CSS transition or keyframes of
 * its own, so a paused frame is exactly the frame the clock says, and the
 * frames the tests and the handbook pictures are taken from are the frames
 * an owner sees.
 *
 * With prefers-reduced-motion the clock does not run at all and the scene
 * shows its still: the one frame chosen to say what the scene is about.
 * requestAnimationFrame already stops in a hidden tab. */

import * as React from "react";
import { FPS } from "./motion";

const REDUCE = "(prefers-reduced-motion: reduce)";

export function useReducedMotion(): boolean {
  return React.useSyncExternalStore(
    (notify) => {
      const query = window.matchMedia(REDUCE);
      query.addEventListener("change", notify);
      return () => query.removeEventListener("change", notify);
    },
    () => window.matchMedia(REDUCE).matches,
    () => false,
  );
}

/** Frames since `restart` last changed, looped over `length`; `still` when motion is reduced or `running` is off. */
export function useLoopFrame(length: number, still: number, running: boolean, restart: unknown): number {
  const reduced = useReducedMotion();
  const [frame, setFrame] = React.useState(0);
  const live = running && !reduced;

  React.useEffect(() => {
    if (!live) return;
    let raf = 0;
    let start: number | null = null;
    setFrame(0);
    const tick = (now: number): void => {
      start ??= now;
      setFrame(Math.floor(((now - start) / 1000) * FPS) % length);
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [live, length, restart]);

  return live ? frame : still;
}

/** Frames since `key` last changed, counting up to `length` and stopping there; `length` at once when motion is reduced. */
export function useOneShot(length: number, key: unknown): number {
  const reduced = useReducedMotion();
  const [frame, setFrame] = React.useState(length);

  React.useEffect(() => {
    if (reduced) return;
    let raf = 0;
    let start: number | null = null;
    setFrame(0);
    const tick = (now: number): void => {
      start ??= now;
      const next = Math.min(length, Math.floor(((now - start) / 1000) * FPS));
      setFrame(next);
      if (next < length) raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [reduced, length, key]);

  return reduced ? length : frame;
}
