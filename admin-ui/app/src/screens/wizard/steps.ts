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

import { t, type MessageKey } from "@/lib/i18n";

export const STEP_IDS = ["trust", "signin", "recovery", "finish"] as const;

export type StepId = (typeof STEP_IDS)[number];

/** Which way the last move went. Drives the slide direction, nothing else. */
export type StepDirection = "forward" | "back";

export interface StepMeta {
  id: StepId;
  /** The card's heading, and the line under the rail. A message key, read
   *  at render — a string here would freeze in the language of import. */
  title: MessageKey;
  /** The rail's own label. Screen-reader-only, so it says which of the four
   *  rather than restating a title the reader has already heard. */
  rail: MessageKey;
}

export const STEPS: readonly StepMeta[] = [
  { id: "trust", title: "wizard.steps.trust", rail: "wizard.steps.trust" },
  { id: "signin", title: "wizard.steps.signin", rail: "wizard.steps.signin" },
  { id: "recovery", title: "wizard.steps.recovery.title", rail: "wizard.steps.recovery.rail" },
  { id: "finish", title: "wizard.steps.finish", rail: "wizard.steps.finish" },
] as const;

export const STEP_COUNT = STEPS.length;

/** 0-based position of `id`. Total: every StepId is in STEP_IDS by construction. */
export function stepIndex(id: StepId): number {
  return STEP_IDS.indexOf(id);
}

/** The step's title in the current language. Call while rendering a
 *  component that used `useT()`. */
export function stepTitle(id: StepId): string {
  const key = STEPS.find((step) => step.id === id)?.title;
  return key === undefined ? "" : t(key);
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
