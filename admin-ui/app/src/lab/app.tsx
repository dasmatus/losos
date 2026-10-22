/* LosOS Diagrams: boot (the core, the engine probe, "This box", the last
 * diagram, LosOS Cloud and the machines), the frame loop, the keyboard, and
 * the layout. On the box it is a screen of the admin UI at /diagrams, inside
 * its top bar and sidebar the way LosOS Mail is: the heading with a button
 * for a new diagram, the three views as a row of buttons with the setup
 * picker at its end, and one card holding the tools, the canvas (with the
 * traffic panel under it in the network views), the tray along its bottom
 * and the inspector on its right (a sheet on a narrower screen). The hosted
 * copy (main.tsx) is the same screen on a page of its own. A `?file=`
 * address opens that diagram from LosOS Cloud.
 *
 * Boot runs once per page. Leaving the screen and coming back finds the
 * core, the engine and the open diagram as they were; the consoles keep
 * running in between. */

import * as React from "react";
import { Card } from "@/components/ui/card";
import { Spinner } from "@/components/ui/spinner";
import { Confirmations } from "@/components/ui/toaster";
import { PaneHeader } from "@/screens/settings/rows";
import { Sheet, SheetContent, SheetTitle } from "@/components/ui/sheet";
import { TooltipProvider } from "@/components/ui/tooltip";
import { confirm, dismissConfirmation } from "@/components/ui/use-toast";
import { t as translate } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import "./lab.css";
import { Actions, actions, setActions } from "./actions";
import { Canvas } from "./canvas";
import { probeCloud } from "./canvas/cloud";
import { DiagramInspector } from "./canvas/inspector";
import { watchMachines } from "./canvas/machines";
import { Core, CoreError } from "./core";
import { Engine } from "./engine";
import { setEngine } from "./engine-hook";
import { LibvirtBackend } from "./engine/libvirt";
import { QemuBackend } from "./engine/qemu";
import { VirtRpcBackend } from "./engine/virt-rpc";
import { readLast } from "./files";
import { logoUrl } from "./icons";
import { gpuFailed, startGpu } from "./render";
import { Inspector } from "./inspector";
import { BOX_COPY } from "./shape";
import { Splash, type SplashStep } from "./splash";
import { LabStore, setStore, store, useLab } from "./store";
import { readOrdering } from "./order";
import { readThisBox } from "./thisbox";
import { NewDiagramButton, SetupPicker, Toolbar, useSummary, ViewTabs } from "./toolbar";
import { EventList, SimBar } from "./traffic";
import { Tray } from "./tray";

const SPLASH_MIN_MS = 1400;

/* A toast; the same words again replace the one still showing instead of
 * stacking (Step pressed five times on an empty queue said it five times). */
const showing = new Map<string, string>();
function say(text: string, ms = 3600): void {
  const prev = showing.get(text);
  if (prev) dismissConfirmation(prev);
  if (showing.size > 64) showing.clear();
  showing.set(text, confirm({ tone: "done", title: text, duration: ms }));
}

let booting: Promise<void> | null = null;

/* Once per page, whatever React does with effects. */
function boot(step: (s: SplashStep) => void): Promise<void> {
  booting ??= bootOnce(step);
  return booting;
}

async function bootOnce(step: (s: SplashStep) => void): Promise<void> {
  const core = await Core.load();
  core.setPlate(logoUrl);
  const s = new LabStore(core);
  setStore(s);
  const engine = new Engine(s);
  setEngine(engine);
  const a = new Actions(s, engine);
  setActions(a);
  s.onRefresh = () => engine.reconcile();
  s.onToast = (text) => say(text, text.length > 90 ? 6000 : 3600);

  // An ordered probe: libvirt through the helper and its virsh, then the
  // WASM libvirt client through the helper's relay, then qemu-wasm in the tab.
  const probe = engine.init([
    new LibvirtBackend(BOX_COPY),
    new VirtRpcBackend(BOX_COPY),
    new QemuBackend(BOX_COPY, () => step("qemu")),
  ]);
  const box = BOX_COPY ? readThisBox() : Promise.resolve(null);
  const ordering = BOX_COPY ? readOrdering() : Promise.resolve(null);
  const cloud = probeCloud();
  await Promise.allSettled([probe, box, ordering, cloud]);
  s.ordering = await ordering;
  s.cloud = await cloud;
  watchMachines(s);

  const last = readLast();
  if (last !== null) {
    const peek = core.peekSetup(last);
    if (!("error" in peek)) s.extras = [{ key: "last", name: peek.name }];
  }
  step("placing");
  const thisBox = await box;
  let loaded = false;
  if (thisBox && "settings" in thisBox) loaded = a.loadThisBox(thisBox.settings, thisBox.edge);
  if (!loaded) a.pick("two-sites");
  const params = new URLSearchParams(window.location.search);
  const view = params.get("view");
  if (view === "canvas" || view === "logical" || view === "physical") a.setView(view);
  const wanted = params.get("file");
  // A diagram someone on another box shared: it opens in its live room.
  const share = params.get("share");
  const shared = Number(params.get("fileId"));
  if (share && Number.isSafeInteger(shared) && s.cloud) await a.openShared(share, shared);
  else if (wanted && s.cloud) await a.openCloud(wanted);
  if (thisBox && "error" in thisBox) {
    s.thisBoxProblem = thisBox.error;
    s.emit("model");
    say(
      thisBox.error === "signedOut" ? translate("lab.thisBox.signIn") : translate("lab.thisBox.noAnswer", { detail: thisBox.detail }),
      9000,
    );
  }

  // The frame loop: tick() is the lab clock and the packets in one call.
  let lastAt = performance.now();
  const frame = (now: number) => {
    const dt = Math.max(0, now - lastAt);
    lastAt = now;
    s.absorb(core.tick(dt), a.httpDone);
    requestAnimationFrame(frame);
  };
  requestAnimationFrame(frame);
  // A background tab gets no frames; the clock still moves on its interval.
  window.setInterval(() => {
    if (document.hidden) {
      const now = performance.now();
      s.absorb(core.tick(now - lastAt), a.httpDone);
      lastAt = now;
    }
  }, 1000);

  window.setInterval(() => a.keepLast(), 2000);
  window.addEventListener("pagehide", () => a.keepLast());

  // The SVG canvas has painted by the next frame; then try the GPU one.
  requestAnimationFrame(() => {
    startGpu(s).catch((e: unknown) => gpuFailed(s, e instanceof Error ? e.message : String(e)));
  });
}

function useMedia(query: string): boolean {
  return React.useSyncExternalStore(
    (fn) => {
      const m = window.matchMedia(query);
      m.addEventListener("change", fn);
      return () => m.removeEventListener("change", fn);
    },
    () => window.matchMedia(query).matches,
  );
}

/** The card's inside: tools, canvas, tray and inspector. */
function Workbench() {
  const s = useLab();
  const t = useT();
  // The inspector sits beside the canvas once the card is wide enough for
  // both; below that it is a sheet, and a tap on a card opens it.
  const side = useMedia("(min-width: 1280px)");
  s.phone = !side;

  React.useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const el = e.target as Element | null;
      if (el?.closest?.("input, textarea, select, [contenteditable], .lab-term, .xterm, [role=dialog]")) return;
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
        e.preventDefault();
        void actions().saveQuick();
        return;
      }
      // The diagram keeps its own keys (canvas/diagram.tsx).
      if (store().ui.view === "canvas") return;
      if (e.key === "Escape") actions().cancelTool();
      if ((e.key === "Delete" || e.key === "Backspace") && store().ui.sel) {
        actions().removeSelection();
        e.preventDefault();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const width = s.ui.wide === "console" ? "w-[min(34rem,55%)]" : s.ui.wide === "desk" ? "w-[min(46rem,75%)]" : "w-80";
  const inspector = s.ui.view === "canvas" ? <DiagramInspector /> : <Inspector />;

  return (
    <>
      <Toolbar details={side ? undefined : () => s.setUi({ sheet: true })} />
      <div className="flex min-h-0 flex-1">
        <div className="flex min-w-0 flex-1 flex-col">
          <div className="relative grid min-h-0 flex-1 grid-cols-1 grid-rows-[minmax(0,1fr)]">
            <Canvas />
          </div>
          {s.ui.view !== "canvas" && (
            <section
              aria-label={t("lab.sim.label")}
              className={cn("flex min-h-0 flex-col border-t border-line bg-surface", s.ui.folded ? "h-auto" : "h-[190px]")}
            >
              <SimBar />
              <EventList />
            </section>
          )}
          <Tray />
        </div>
        {side && (
          <aside aria-label={t("lab.inspector.label")} className={cn("flex min-h-0 shrink-0 flex-col overflow-y-auto border-l border-line bg-surface", width)}>
            {inspector}
          </aside>
        )}
      </div>
      {!side && (
        <Sheet open={s.ui.sheet} onOpenChange={(open) => s.setUi({ sheet: open })}>
          <SheetContent side="right" closeLabel={t("lab.sheet.close")} className="w-full max-w-[440px] gap-0 bg-surface p-0 pt-10" data-testid="inspector-sheet">
            <SheetTitle className="sr-only">{t("lab.sheet.title")}</SheetTitle>
            <div className="flex min-h-0 flex-1 flex-col overflow-auto">{inspector}</div>
          </SheetContent>
        </Sheet>
      )}
    </>
  );
}

type Boot = { ready: boolean; step: SplashStep; error?: string };

/* The boot's progress, for whichever screen is showing. */
let progress: Boot = { ready: false, step: "canvas" };
const watchers = new Set<() => void>();
function setProgress(p: Partial<Boot>): void {
  progress = { ...progress, ...p };
  for (const fn of watchers) fn();
}
function startBoot(): void {
  boot((step) => setProgress({ step })).then(
    () => setProgress({ ready: true }),
    (e: unknown) => {
      console.error(e);
      setProgress({
        step: "failed",
        error: e instanceof CoreError ? translate("lab.splash.coreBroken", { detail: e.message }) : translate("lab.splash.failed", { detail: String(e) }),
      });
    },
  );
}
function useBoot(): Boot {
  React.useEffect(startBoot, []);
  return React.useSyncExternalStore(
    (fn) => {
      watchers.add(fn);
      return () => watchers.delete(fn);
    },
    () => progress,
  );
}

function Heading() {
  const t = useT();
  const summary = useSummary();
  return (
    <div className="flex items-start gap-3">
      <div className="min-w-0 flex-1">
        <PaneHeader title={t("dg.title")} summary={summary} />
      </div>
      <NewDiagramButton />
    </div>
  );
}

/** While the core loads: the card with what it is doing, or why it stopped. */
function Loading({ step, error }: { step: SplashStep; error?: string }) {
  const t = useT();
  const msg = step === "failed" ? error : step === "qemu" ? t("lab.splash.qemu") : t("dg.loading");
  return (
    <div role="status" className="m-auto flex max-w-md flex-col items-center gap-3 p-8 text-center text-[13px] text-muted" data-testid="lab-splash">
      {step !== "failed" && <Spinner />}
      <span className={cn(step === "failed" && "text-crit")}>{msg}</span>
    </div>
  );
}

/** The screen: heading, views and picker, the card. */
function Screen({ boot: b }: { boot: Boot }) {
  const t = useT();
  return (
    <TooltipProvider>
      <div className="flex flex-col" data-testid="lab">
        {b.ready ? (
          <>
            <Heading />
            <div className="mb-3 flex flex-wrap items-center gap-2">
              <ViewTabs />
              <SetupPicker className="w-full sm:ml-auto sm:w-64" />
            </div>
          </>
        ) : (
          <PaneHeader title={t("dg.title")} summary={t("dg.summary.local")} />
        )}
        <Card className="flex h-[min(54rem,calc(100dvh-15rem))] min-h-[30rem] flex-col overflow-hidden">
          {b.ready ? <Workbench /> : <Loading step={b.step} error={b.error} />}
        </Card>
      </div>
    </TooltipProvider>
  );
}

/** /diagrams in the admin UI (App.tsx), inside its top bar and sidebar. */
export default function DiagramsScreen() {
  const b = useBoot();
  React.useEffect(() => {
    const before = document.title;
    document.title = "LosOS Diagrams";
    return () => {
      document.title = before;
    };
  }, []);
  return <Screen boot={b} />;
}

/** The hosted copy's page (main.tsx): the same screen under a plain header,
 *  with the splash over it while qemu-wasm loads. */
export function LabRoot() {
  const b = useBoot();
  const [shown] = React.useState(() => performance.now());
  const [gone, setGone] = React.useState(false);
  const [removed, setRemoved] = React.useState(false);
  React.useEffect(() => {
    if (!b.ready) return;
    const out = window.setTimeout(
      () => {
        setGone(true);
        window.setTimeout(() => setRemoved(true), 500);
      },
      Math.max(0, SPLASH_MIN_MS - (performance.now() - shown)),
    );
    return () => window.clearTimeout(out);
  }, [b.ready, shown]);
  return (
    <>
      <header className="sticky top-0 z-40 border-b border-line bg-surface/85 backdrop-blur-sm">
        <div className="mx-auto flex w-full max-w-6xl items-center gap-2 px-4 py-3 text-[17px] font-semibold sm:px-6">
          <img src={logoUrl} alt="" width={26} height={26} className="size-[26px]" />
          LosOS Diagrams
        </div>
      </header>
      <main className="mx-auto w-full max-w-6xl px-4 pt-5 pb-12 sm:px-6">
        <Screen boot={b} />
      </main>
      {!removed && <Splash step={b.step} gone={gone} error={b.error} />}
      <Confirmations className="top-16 max-sm:top-16" />
    </>
  );
}
