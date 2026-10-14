/* The first-run tour: six steps, each a looping scene over one sentence.
 *
 * It opens by itself once, after the setup wizard (App.tsx calls
 * startTour() from the wizard's onDone), and again from the About pane.
 * Skipping, Escape, a click on the backdrop and Done all close it the same
 * way: closeTour() marks it seen in this browser (store.ts).
 *
 * The pictures are code, not video. Each scene in scenes.tsx is a function
 * of a frame number, the clock (clock.ts) supplies that number, and the
 * miniature page around it (mini-page.tsx) carries the sidebar's teal entry
 * from one step's section to the next. That keeps every word in the scenes
 * in the language on screen, every colour on the theme on screen, and the
 * whole tour a few kilobytes in the bundle; there is nothing to re-record
 * when a label changes. The dialog is the app's own native <dialog>
 * (components/ui/dialog.tsx). */

import * as React from "react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import type { MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { useLoopFrame, useOneShot } from "./clock";
import { ARRIVAL, MiniPage, VIEW_H, VIEW_W } from "./mini-page";
import { SCENES, sceneAt } from "./scenes";
import { closeTour } from "./store";

const COPY: Record<(typeof SCENES)[number]["id"], { title: MessageKey; body: MessageKey }> = {
  overview: { title: "tour.overview.title", body: "tour.overview.body" },
  apps: { title: "tour.apps.title", body: "tour.apps.body" },
  storage: { title: "tour.storage.title", body: "tour.storage.body" },
  mesh: { title: "tour.mesh.title", body: "tour.mesh.body" },
  history: { title: "tour.history.title", body: "tour.history.body" },
  about: { title: "tour.about.title", body: "tour.about.body" },
};

export interface TourSceneProps {
  index: number;
  /** The step before, for the sidebar entry's travel. */
  from: number;
  frame: number;
  arrival: number;
  className?: string;
}

/** One step's picture at a given frame: the miniature page with the scene inside. */
export function TourScene({ index, from, frame, arrival, className }: TourSceneProps) {
  const scene = sceneAt(index);
  const previous = sceneAt(from);
  const { Scene } = scene;
  return (
    <svg
      viewBox={`0 0 ${VIEW_W} ${VIEW_H}`}
      className={cn("block h-auto w-full font-sans select-none", className)}
      aria-hidden="true"
      focusable="false"
    >
      <MiniPage entry={scene.entry} from={previous.entry} arrival={arrival}>
        <Scene frame={frame} />
      </MiniPage>
    </svg>
  );
}

export default function Tour() {
  const t = useT();
  const [step, setStep] = React.useState(0);
  const [from, setFrom] = React.useState(0);
  const titleId = React.useId();
  const bodyId = React.useId();
  const scene = sceneAt(step);
  const last = step === SCENES.length - 1;

  const frame = useLoopFrame(scene.length, scene.still, true, step);
  const arrival = useOneShot(ARRIVAL, step);

  // The primary button holds the focus, so Enter walks the tour. showModal()
  // puts the focus on the first control it finds, the first step dot, after
  // React's autoFocus has run, so it is set here once the dialog is open.
  React.useEffect(() => {
    document.querySelector<HTMLButtonElement>("[data-tour-primary]")?.focus();
  }, [step]);

  const go = (next: number): void => {
    if (next < 0 || next >= SCENES.length || next === step) return;
    setFrom(step);
    setStep(next);
  };

  const onKeyDown = (event: React.KeyboardEvent): void => {
    if (event.key === "ArrowRight") go(step + 1);
    else if (event.key === "ArrowLeft") go(step - 1);
  };

  const copy = COPY[scene.id];

  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) closeTour();
      }}
      labelledBy={titleId}
      describedBy={bodyId}
      dialogClassName="w-[min(46rem,calc(100vw-2rem))] max-w-[calc(100vw-2rem)]"
      onKeyDown={onKeyDown}
      data-testid="tour"
    >
      <div className="p-3 pb-0">
        <div className="overflow-hidden rounded-control border border-line">
          <TourScene index={step} from={from} frame={frame} arrival={arrival} />
        </div>
      </div>
      <DialogHeader className="pt-4">
        <p className="numeric text-[12px] text-faint" aria-live="polite">
          {t("tour.count", { n: String(step + 1), total: String(SCENES.length) })}
        </p>
        <DialogTitle id={titleId}>{t(copy.title)}</DialogTitle>
        <DialogDescription id={bodyId} className="text-[13.5px]">
          {t(copy.body)}
        </DialogDescription>
      </DialogHeader>
      <DialogFooter className="mt-2 justify-between">
        <div className="mr-auto flex items-center gap-1.5">
          {SCENES.map((s, i) => (
            <button
              key={s.id}
              type="button"
              aria-label={t("tour.step", { n: String(i + 1) })}
              aria-current={i === step ? "step" : undefined}
              onClick={() => go(i)}
              className={cn(
                "size-2 rounded-full transition-colors duration-150",
                "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent",
                i === step ? "bg-accent" : "bg-line hover:bg-faint",
              )}
            />
          ))}
        </div>
        {!last && (
          <Button variant="ghost" size="sm" onClick={closeTour}>
            {t("tour.skip")}
          </Button>
        )}
        {step > 0 && (
          <Button variant="outline" size="sm" onClick={() => go(step - 1)}>
            {t("tour.back")}
          </Button>
        )}
        {last ? (
          <Button data-tour-primary size="sm" onClick={closeTour}>
            {t("tour.done")}
          </Button>
        ) : (
          <Button data-tour-primary size="sm" onClick={() => go(step + 1)}>
            {t("tour.next")}
          </Button>
        )}
      </DialogFooter>
    </Dialog>
  );
}
