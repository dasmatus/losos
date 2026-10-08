/* LosOS Lab: boot (the core, the engine probe, "This box", the last setup),
 * the frame loop, the keyboard, and the workbench layout: the top bar, the
 * canvas with the traffic panel under it, the inspector docked right (a
 * sheet on a phone) and the parts tray along the bottom. */

import * as React from "react";
import { Confirmations } from "@/components/ui/toaster";
import { Sheet, SheetContent, SheetTitle } from "@/components/ui/sheet";
import { TooltipProvider } from "@/components/ui/tooltip";
import { confirm, dismissConfirmation } from "@/components/ui/use-toast";
import { t as translate } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { Actions, actions, setActions } from "./actions";
import { Canvas } from "./canvas";
import { Core, CoreError } from "./core";
import { Engine } from "./engine";
import { setEngine } from "./engine-hook";
import { LibvirtBackend } from "./engine/libvirt";
import { QemuBackend } from "./engine/qemu";
import { VirtRpcBackend } from "./engine/virt-rpc";
import { readLast } from "./files";
import { plateUrl } from "./icons";
import { Inspector } from "./inspector";
import { BOX_COPY } from "./shape";
import { Splash, type SplashStep } from "./splash";
import { LabStore, setStore, store, useLab } from "./store";
import { readThisBox } from "./thisbox";
import { TopBar } from "./topbar";
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
  core.setPlate(plateUrl);
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
  await Promise.allSettled([probe, box]);

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
}

function usePhone(): boolean {
  const query = "(max-width: 900px)";
  return React.useSyncExternalStore(
    (fn) => {
      const m = window.matchMedia(query);
      m.addEventListener("change", fn);
      return () => m.removeEventListener("change", fn);
    },
    () => window.matchMedia(query).matches,
  );
}

function Workbench() {
  const s = useLab();
  const t = useT();
  const phone = usePhone();
  s.phone = phone;

  React.useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const el = e.target as Element | null;
      if (el?.closest?.("input, textarea, select, [contenteditable], .lab-term, .xterm, [role=dialog]")) return;
      if (e.key === "Escape") actions().cancelTool();
      if ((e.key === "Delete" || e.key === "Backspace") && store().ui.sel) {
        actions().removeSelection();
        e.preventDefault();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const cols =
    s.ui.wide === "console"
      ? "min-[901px]:grid-cols-[minmax(0,1fr)_min(560px,44vw)]"
      : s.ui.wide === "desk"
        ? "min-[901px]:grid-cols-[minmax(0,1fr)_min(860px,62vw)]"
        : "min-[901px]:grid-cols-[minmax(0,1fr)_380px]";

  return (
    <div
      className={cn(
        "grid min-h-full grid-cols-1 grid-rows-[auto_62vh_auto] [grid-template-areas:'top'_'work'_'tray']",
        "min-[901px]:h-full min-[901px]:grid-rows-[auto_1fr_auto] min-[901px]:[grid-template-areas:'top_top'_'work_side'_'tray_side']",
        cols,
      )}
      data-testid="lab"
    >
      <TopBar phone={phone} onDetails={() => s.setUi({ sheet: true })} />
      <main className="relative grid min-h-0 min-w-0 grid-cols-1 grid-rows-[minmax(0,1fr)_auto] [grid-area:work]">
        <Canvas />
        <section
          aria-label={t("lab.sim.label")}
          className={cn("flex min-h-0 flex-col border-t border-line bg-surface", s.ui.folded ? "h-auto" : "h-[210px]")}
        >
          <SimBar />
          <EventList />
        </section>
      </main>
      {phone ? (
        <Sheet open={s.ui.sheet} onOpenChange={(open) => s.setUi({ sheet: open })}>
          <SheetContent side="right" closeLabel={t("lab.sheet.close")} className="w-full max-w-[440px] gap-0 bg-surface p-0 pt-10" data-testid="inspector-sheet">
            <SheetTitle className="sr-only">{t("lab.sheet.title")}</SheetTitle>
            <div className="flex min-h-0 flex-1 flex-col overflow-auto">
              <Inspector />
            </div>
          </SheetContent>
        </Sheet>
      ) : (
        <aside aria-label={t("lab.inspector.label")} className="flex min-h-0 min-w-0 flex-col border-l border-line bg-surface [grid-area:side]">
          <Inspector />
        </aside>
      )}
      <Tray />
    </div>
  );
}

export function LabRoot() {
  const [step, setStep] = React.useState<SplashStep>("canvas");
  const [ready, setReady] = React.useState(false);
  const [gone, setGone] = React.useState(false);
  const [removed, setRemoved] = React.useState(false);
  const [error, setError] = React.useState<string>();

  React.useEffect(() => {
    const shown = performance.now();
    let cancelled = false;
    boot(setStep).then(
      () => {
        if (cancelled) return;
        setReady(true);
        window.setTimeout(
          () => {
            setGone(true);
            window.setTimeout(() => setRemoved(true), 500);
          },
          Math.max(0, SPLASH_MIN_MS - (performance.now() - shown)),
        );
      },
      (e: unknown) => {
        console.error(e);
        setStep("failed");
        setError(e instanceof CoreError ? translate("lab.splash.coreBroken", { detail: e.message }) : translate("lab.splash.failed", { detail: String(e) }));
      },
    );
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <TooltipProvider>
      {ready && <Workbench />}
      {!removed && <Splash step={step} gone={gone} error={error} />}
      <Confirmations />
    </TooltipProvider>
  );
}
