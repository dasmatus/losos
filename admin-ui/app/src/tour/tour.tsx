/* The first-run tour: a step per thing the box does, each a looping scene
 * over one sentence.
 *
 * It opens by itself once, after the setup wizard (App.tsx calls
 * startTour() from the wizard's onDone), and again from the About pane.
 * Skipping, Escape, a click on the backdrop and Done all close it the same
 * way: closeTour() marks it seen in this browser (store.ts).
 *
 * Which steps it has depends on the box: chat, mail and calls run as the
 * shared user, so their step is there only while the box shares its storage
 * and has joined a mesh (GET /api/settings). Until the settings answer, and
 * if they never do, the tour goes without it, as the sidebar goes without
 * Chat.
 *
 * The pictures are code, not video. Each scene in scenes.tsx and
 * more-scenes.tsx is a function of a frame number, the clock (clock.ts)
 * supplies that number, and the miniature page around it (mini-page.tsx)
 * carries the sidebar's teal entry from one step's section to the next.
 * That keeps every word in the scenes in the language on screen, every
 * colour on the theme on screen, and the whole tour a few kilobytes in the
 * bundle; there is nothing to re-record when a label changes. The dialog is
 * the app's own native <dialog> (components/ui/dialog.tsx). */

import * as React from "react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import type { MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { getSettings } from "@/lib/api";
import { useLoopFrame, useOneShot } from "./clock";
import { ARRIVAL, MiniPage, VIEW_H, VIEW_W } from "./mini-page";
import { scenesFor, sceneAt, TourLength, type SceneDef, type SceneId } from "./scenes";
import { closeTour } from "./store";

const COPY: Record<SceneId, { title: MessageKey; body: MessageKey }> = {
  overview: { title: "tour.overview.title", body: "tour.overview.body" },
  board: { title: "tour.board.title", body: "tour.board.body" },
  together: { title: "tour.together.title", body: "tour.together.body" },
  apps: { title: "tour.apps.title", body: "tour.apps.body" },
  builder: { title: "tour.builder.title", body: "tour.builder.body" },
  storage: { title: "tour.storage.title", body: "tour.storage.body" },
  mesh: { title: "tour.mesh.title", body: "tour.mesh.body" },
  paid: { title: "tour.paid.title", body: "tour.paid.body" },
  chat: { title: "tour.chat.title", body: "tour.chat.body" },
  profile: { title: "tour.profile.title", body: "tour.profile.body" },
  history: { title: "tour.history.title", body: "tour.history.body" },
  about: { title: "tour.about.title", body: "tour.about.body" },
};

/** Whether this box shares its storage in a mesh: the gate on chat, mail and calls. False until it is known. */
function useSharedSide(): boolean {
  const [shared, setShared] = React.useState(false);
  React.useEffect(() => {
    const controller = new AbortController();
    getSettings({ signal: controller.signal })
      .then((settings) => setShared(settings.sharingMyStorage && settings.clusterEnable))
      .catch(() => {
        // No answer: the tour goes on without the shared steps.
      });
    return () => controller.abort();
  }, []);
  return shared;
}

export interface TourSceneProps {
  scenes: readonly SceneDef[];
  /** The box shares its storage in a mesh, so the miniature's sidebar has Chat. */
  shared: boolean;
  index: number;
  /** The step before, for the sidebar entry's travel. */
  from: number;
  frame: number;
  arrival: number;
  className?: string;
}

/** One step's picture at a given frame: the miniature page with the scene inside. */
export function TourScene({ scenes, shared, index, from, frame, arrival, className }: TourSceneProps) {
  const scene = sceneAt(scenes, index);
  const previous = sceneAt(scenes, from);
  const { Scene } = scene;
  return (
    <svg
      viewBox={`0 0 ${VIEW_W} ${VIEW_H}`}
      className={cn("block h-auto w-full font-sans select-none", className)}
      aria-hidden="true"
      focusable="false"
    >
      <MiniPage entry={scene.entry} from={previous.entry} arrival={arrival} shared={shared}>
        <Scene frame={frame} />
      </MiniPage>
    </svg>
  );
}

export default function Tour() {
  const t = useT();
  // Steps are held by id, so a step that arrives once the settings answer
  // (chat) does not move the owner off the one they are reading.
  const [stepId, setStepId] = React.useState<SceneId>("overview");
  const [fromId, setFromId] = React.useState<SceneId>("overview");
  const titleId = React.useId();
  const bodyId = React.useId();
  const shared = useSharedSide();
  const scenes = React.useMemo(() => scenesFor(shared), [shared]);
  const indexOf = (id: SceneId): number => Math.max(0, scenes.findIndex((s) => s.id === id));
  const step = indexOf(stepId);
  const from = indexOf(fromId);
  const scene = sceneAt(scenes, step);
  const last = step >= scenes.length - 1;

  const frame = useLoopFrame(scene.length, scene.still, true, stepId);
  const arrival = useOneShot(ARRIVAL, stepId);

  // The primary button holds the focus, so Enter walks the tour. showModal()
  // puts the focus on the first control it finds, the first step dot, after
  // React's autoFocus has run, so it is set here once the dialog is open.
  React.useEffect(() => {
    document.querySelector<HTMLButtonElement>("[data-tour-primary]")?.focus();
  }, [stepId]);

  const go = (next: number): void => {
    const target = scenes[next];
    if (target === undefined || next === step) return;
    setFromId(scene.id);
    setStepId(target.id);
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
          <TourLength.Provider value={scenes.length}>
            <TourScene scenes={scenes} shared={shared} index={step} from={from} frame={frame} arrival={arrival} />
          </TourLength.Provider>
        </div>
      </div>
      <DialogHeader className="pt-4">
        <p className="numeric text-[12px] text-faint" aria-live="polite">
          {t("tour.count", { n: String(step + 1), total: String(scenes.length) })}
        </p>
        <DialogTitle id={titleId}>{t(copy.title)}</DialogTitle>
        <DialogDescription id={bodyId} className="text-[13.5px]">
          {t(copy.body)}
        </DialogDescription>
      </DialogHeader>
      <DialogFooter className="mt-2 justify-between">
        <div className="mr-auto flex items-center gap-1.5">
          {scenes.map((s, i) => (
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
