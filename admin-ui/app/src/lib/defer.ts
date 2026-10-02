/* Keeping the main thread free.
 *
 * Everything here is about one rule: the first paint and the next input event
 * come first. Work that nobody is waiting on — a heartbeat, a probe, a
 * journal write — is handed to the browser's idle time instead of running
 * inside whatever task happened to ask for it.
 *
 * There is deliberately no Worker. The appliance serves this page under
 * `script-src 'self'` with `worker-src` falling back to it, so a bundled
 * same-origin worker would load, but none of the work here is heavy enough to
 * earn a second realm and a message protocol; the cost is scheduling, not
 * compute. */

interface IdleDeadline {
  readonly didTimeout: boolean;
  timeRemaining(): number;
}

type IdleScheduler = {
  requestIdleCallback?: (cb: (deadline: IdleDeadline) => void, opts?: { timeout: number }) => number;
  cancelIdleCallback?: (handle: number) => void;
};

/** Longest an idle task may be starved before it runs anyway. */
const IDLE_TIMEOUT_MS = 2000;

/* Where requestIdleCallback does not exist (Safari), a macrotask after the
 * next paint is the closest honest substitute. */
const FALLBACK_DELAY_MS = 200;

/** Run `task` when the browser has nothing better to do. Returns a canceller. */
export function whenIdle(task: () => void): () => void {
  const scheduler = globalThis as unknown as IdleScheduler;

  if (typeof scheduler.requestIdleCallback === "function") {
    const handle = scheduler.requestIdleCallback(() => task(), { timeout: IDLE_TIMEOUT_MS });
    return () => scheduler.cancelIdleCallback?.(handle);
  }

  const timer = setTimeout(task, FALLBACK_DELAY_MS);
  return () => clearTimeout(timer);
}

/** An effect body that starts `start` at idle time and tears down whatever it
 *  returned — or cancels it, if idle time never came. */
export function startWhenIdle(start: () => void | (() => void)): () => void {
  let stop: void | (() => void);
  const cancel = whenIdle(() => {
    stop = start();
  });
  return () => {
    cancel();
    if (typeof stop === "function") stop();
  };
}
