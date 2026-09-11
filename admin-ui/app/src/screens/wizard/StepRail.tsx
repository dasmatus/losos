/* The progress rail: four thin bars, one per step.
 *
 * Done is a full accent bar, the current step is the same bar at half opacity,
 * and the steps ahead are an empty track. One accent, no second hue — the
 * distinction is fill, exactly as it is everywhere else in this app.
 *
 * The bars are the only thing on screen, but they are not the only thing the
 * rail says: each carries a screen-reader-only label, and the line underneath
 * names the step in numbers and in words. A row of four unlabelled rectangles
 * is decoration to a sighted user and nothing at all to anyone else.
 */

import { cn } from "@/lib/utils";
import { STEPS, STEP_COUNT, stepIndex, stepTitle, type StepId } from "./steps";

export function StepRail({ current }: { current: StepId }) {
  const index = stepIndex(current);

  return (
    <div className="flex flex-col gap-2">
      <ol aria-label="Setup progress" className="flex gap-1.5">
        {STEPS.map((step, position) => (
          <li
            key={step.id}
            aria-current={position === index ? "step" : undefined}
            className="h-1 flex-1 overflow-hidden rounded-full bg-sunk"
          >
            <span
              className={cn(
                "block h-full rounded-full bg-accent",
                "transition-[width,opacity] duration-300 ease-out",
                position < index && "w-full",
                position === index && "w-full opacity-50",
                position > index && "w-0",
              )}
            />
            <span className="sr-only">
              {step.rail}
              {position < index ? " (done)" : position === index ? " (current)" : ""}
            </span>
          </li>
        ))}
      </ol>

      <p className="text-[12.5px] leading-snug text-faint">
        <span className="numeric">
          Step {index + 1} of {STEP_COUNT}
        </span>
        <span aria-hidden="true"> &middot; </span>
        <span className="text-muted">{stepTitle(current)}</span>
      </p>
    </div>
  );
}
