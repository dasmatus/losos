/* The engine as React sees it: subscribed, so the badge, the VM tags and the
 * console pane redraw when a guest starts, stops or says it is ready. */

import { useSyncExternalStore } from "react";
import type { Engine } from "./engine";

let current: Engine | null = null;

export function setEngine(engine: Engine): void {
  current = engine;
}

export function useEngine(): Engine {
  const e = current;
  if (e === null) throw new Error("the Lab engine is not set up yet");
  useSyncExternalStore(
    (fn) => e.subscribe(fn),
    () => e.version,
  );
  return e;
}
