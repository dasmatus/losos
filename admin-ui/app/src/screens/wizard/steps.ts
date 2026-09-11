/* The four steps, in order, and the arithmetic for moving between them.
 *
 * Split out of Wizard.tsx because StepRail.tsx needs the same list and the
 * same index, and a rail that disagrees with the state machine about how many
 * steps there are is the kind of bug nobody notices until a box is on a
 * stranger's desk.
 *
 * Steps are identified by name rather than by number. The order is data, so
 * inserting a step is one edit here; a `step === 2` scattered through four
 * components is four edits and a silent miss.
 */

export const STEP_IDS = ["trust", "signin", "recovery", "finish"] as const;

export type StepId = (typeof STEP_IDS)[number];

/** Which way the last move went. Drives the slide direction, nothing else. */
export type StepDirection = "forward" | "back";

export interface StepMeta {
  id: StepId;
  /** The card's heading, and the line under the rail. */
  title: string;
  /** The rail's own label. Screen-reader-only, so it says which of the four
   *  rather than restating a title the reader has already heard. */
  rail: string;
}

export const STEPS: readonly StepMeta[] = [
  { id: "trust", title: "Trust this box", rail: "Trust this box" },
  { id: "signin", title: "Choose how you sign in", rail: "Choose how you sign in" },
  { id: "recovery", title: "Write down your recovery code", rail: "Recovery code" },
  { id: "finish", title: "Sign in", rail: "Sign in" },
] as const;

export const STEP_COUNT = STEPS.length;

/** 0-based position of `id`. Total: every StepId is in STEP_IDS by construction. */
export function stepIndex(id: StepId): number {
  return STEP_IDS.indexOf(id);
}

export function stepTitle(id: StepId): string {
  return STEPS.find((step) => step.id === id)?.title ?? "";
}

/* The neighbours, or null at the ends.
 *
 * Returning null rather than clamping is deliberate: the footer decides
 * whether Back exists by asking for one, and clamping would give it a Back
 * button on step 1 that moves nowhere. */
export function nextStep(id: StepId): StepId | null {
  return STEP_IDS[stepIndex(id) + 1] ?? null;
}

export function previousStep(id: StepId): StepId | null {
  const index = stepIndex(id);
  return index <= 0 ? null : (STEP_IDS[index - 1] ?? null);
}
