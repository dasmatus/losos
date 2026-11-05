/* Declarations for front.mjs, which the browser checks run a call against:
 * `tsc --noEmit` (tsconfig.tools.json) reads a `.mjs` import only through a
 * `.d.mts` — the module itself is invisible without allowJs, which is why
 * calls.browser.ts's check failed on CI with TS7016. The real shape lives in
 * front.mjs; keep the two in step by hand.
 *
 * The two processes are loose on purpose: sfu is Bun's Subprocess and nginx
 * its Server, and the checks only end them (`sfu.kill()`, `nginx.stop(true)`)
 * — typing them fully would drag Bun's globals into a fixture that has no
 * further use for them. */

interface Front {
  sfu: { kill(): void };
  /* The tap directory the SFU records into (tests/calls.browser.ts reads the
   * `.ivf` and `.flv` it writes there). */
  tap: string;
  nginx: { stop(force?: boolean): void };
  /* "host:port" the front answers on, and the calls API's own address. */
  front: string;
  /* Null is a guest: the call rides without a signed-in name (no `/as/…`). */
  api(name?: string | null): string;
  stop(): void;
}

export function startFront(opts: {
  /* The losos-calls binary. */
  bin: string;
  address: string;
  serverName: string;
  /* Directory for the tap and the relay token. */
  tmp: string;
  registrar?: string;
}): Promise<Front>;
