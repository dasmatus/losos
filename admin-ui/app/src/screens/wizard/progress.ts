/* Where the wizard is, kept for this tab across a reload.
 *
 * The box records only that it has an owner (GET /api/setup/claim). Once
 * step 2 has claimed it, the tab holds the admin token, so after a reload the
 * shell would read "signed in" and open the app in the middle of setup: the
 * spare admin key gone from the screen and the first sign-in skipped. This
 * note is what keeps the wizard up until the owner leaves its last step.
 *
 * sessionStorage, beside the token and with the same lifetime: it is about
 * this tab's run through setup, and closing the tab ends both. The key itself
 * is not stored here. It is the token the claim released, so a resumed step 2
 * shows the token this tab already holds. */

import { getToken } from "@/lib/api";
import type { DesktopSignIn } from "./StepDesktop";
import { STEP_IDS, type StepId } from "./steps";

const SIGN_INS: readonly DesktopSignIn[] = ["person", "plainHttp", "noLogin", "address"];

const KEY = "losos-setup-progress";

export interface SetupProgress {
  step: StepId;
  /** The account step 2 set a password for, or null before that. */
  account: string | null;
  /** The LosOS Login name step 2 made, for step 3 to sign in with. */
  login: string | null;
  /** Step 2 claimed the box from this tab, so the spare key is the token. */
  keyShown: boolean;
  /** How step 3 left this tab signed in, or null before it has. Never the
   *  password: a tab reloaded on step 3 asks for it again. */
  signedIn: DesktopSignIn | null;
}

export function readProgress(): SetupProgress | null {
  try {
    const raw = window.sessionStorage.getItem(KEY);
    if (raw === null) return null;
    const value: unknown = JSON.parse(raw);
    if (typeof value !== "object" || value === null) return null;
    const v = value as Record<string, unknown>;
    const step = STEP_IDS.find((id) => id === v["step"]);
    if (step === undefined) return null;
    return {
      step,
      account: typeof v["account"] === "string" ? v["account"] : null,
      login: typeof v["login"] === "string" ? v["login"] : null,
      keyShown: v["keyShown"] === true,
      signedIn: SIGN_INS.find((how) => how === v["signedIn"]) ?? null,
    };
  } catch {
    return null;
  }
}

export function saveProgress(progress: SetupProgress): void {
  try {
    window.sessionStorage.setItem(KEY, JSON.stringify(progress));
  } catch {
    /* storage blocked: a reload starts setup over, as it always did */
  }
}

export function clearProgress(): void {
  try {
    window.sessionStorage.removeItem(KEY);
  } catch {
    /* nothing to clear */
  }
}

/** The spare key to show again on a resumed step 2, or null. */
export function resumedKey(progress: SetupProgress | null): string | null {
  return progress?.keyShown === true ? getToken() : null;
}
