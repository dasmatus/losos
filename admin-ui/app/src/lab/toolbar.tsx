/* The chrome around the canvas, laid out the way LosOS Mail is: a button
 * for a new diagram beside the pane's heading, the three views as a row of
 * buttons with the setup picker at its end, and inside the card one row
 * of tools above the canvas (the file menu, undo and redo, the network's
 * two modes, who else is here, the engine, and the details on a narrow
 * screen). */

import * as React from "react";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import {
  ArrowDown01Icon,
  ArrowTurnBackwardIcon,
  ArrowTurnForwardIcon,
  FlowchartIcon,
  InformationCircleIcon,
  NeuralNetworkIcon,
  PencilEdit02Icon,
  ServerStack01Icon,
  ShoppingCart01Icon,
} from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { ButtonGroup } from "@/components/ui/button-group";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuLabel, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { StatusDot } from "@/components/ui/badge";
import { useT } from "@/lib/i18n-react";
import type { MessageKey } from "@/lib/i18n";
import { cn } from "@/lib/utils";
import { actions } from "./actions";
import { CloudDialog } from "./canvas/cloud-dialog";
import { LivePeople } from "./canvas/people";
import { useEngine } from "./engine-hook";
import { OrderDialog } from "./order-dialog";
import { useLab, type Workspace } from "./store";

/* What runs the guests, in the order the engine tries them. */
function emulatedTip(kinds: string[]): string {
  return (
    (kinds.includes("libvirt")
      ? "Guests run under libvirt on this computer through losos-registrar lab, one transient domain per powered device. "
      : "") +
    (kinds.includes("virt-rpc")
      ? kinds.includes("libvirt")
        ? "This page's own libvirt client, compiled to WebAssembly, starts any guest the helper's virsh cannot, through the helper's relay. "
        : "This page's own libvirt client, compiled to WebAssembly, runs the guests under libvirt on this computer through losos-registrar lab's relay, one transient domain per powered device. "
      : "") +
    (kinds.includes("qemu-wasm")
      ? kinds.length > 1
        ? "qemu-system-x86_64 compiled to WebAssembly (ktock/qemu-wasm) runs any guest libvirt cannot. "
        : "qemu-system-x86_64 compiled to WebAssembly (ktock/qemu-wasm), one instance per powered device. "
      : "") +
    "LosOS devices boot the LosOS stand-in guest; routers, switches and access points boot Netzgeräte Betriebssystem."
  );
}

/** What the heading says under the name: where the open diagram lives. */
export function useSummary(): string {
  const s = useLab();
  const t = useT();
  const from = s.live?.from;
  if (from) return t("dg.summary.shared", { name: from.name, owner: from.owner, box: from.box });
  if (s.cloudFile) return t("dg.summary.cloud", { path: s.cloudFile.path });
  return t("dg.summary.local");
}

export function NewDiagramButton() {
  const t = useT();
  return (
    <Button onClick={() => actions().canvas.newDiagram()} className="shrink-0" data-testid="new-diagram">
      <HugeiconsIcon icon={PencilEdit02Icon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      {t("dg.file.new")}
    </Button>
  );
}

const VIEWS: readonly { view: Workspace; label: MessageKey; icon: IconSvgElement; testid?: string }[] = [
  { view: "canvas", label: "dg.view.canvas", icon: FlowchartIcon, testid: "view-canvas" },
  { view: "logical", label: "lab.view.logical", icon: NeuralNetworkIcon, testid: "view-logical" },
  { view: "physical", label: "lab.view.physical", icon: ServerStack01Icon, testid: "view-physical" },
];

/** The three views, a row of buttons as Mail's folders are. */
export function ViewTabs() {
  const s = useLab();
  const t = useT();
  return (
    <nav aria-label={t("lab.top.workspace")} className="-mx-1 max-w-full min-w-0 overflow-x-auto px-1">
      <ul className="flex gap-1">
        {VIEWS.map((v) => {
          const current = s.ui.view === v.view;
          return (
            <li key={v.view} className="shrink-0">
              <Button
                variant={current ? "secondary" : "ghost"}
                size="sm"
                aria-current={current ? "page" : undefined}
                onClick={() => actions().setView(v.view)}
                className={cn("gap-1.5", current && "text-ink")}
                data-testid={v.testid}
              >
                <HugeiconsIcon icon={v.icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                {t(v.label)}
              </Button>
            </li>
          );
        })}
      </ul>
    </nav>
  );
}

/** The setup picker: the examples, this box, the last diagram, a file. */
export function SetupPicker({ className }: { className?: string }) {
  const s = useLab();
  const t = useT();
  const scenario = s.snap.scenario;
  const known = new Set([...s.scenarios.map((x) => x.key), ...s.extras.map((x) => x.key)]);
  return (
    <NativeSelect
      aria-label={t("lab.top.setup")}
      className={className}
      value={known.has(scenario) ? scenario : ""}
      onChange={(e) => actions().pick(e.target.value)}
      data-testid="setup-picker"
    >
      {!known.has(scenario) && <NativeSelectOption value="">{t("lab.top.mySetup")}</NativeSelectOption>}
      {s.thisBoxProblem && (
        <NativeSelectOption value="this-box-problem" disabled>
          {s.thisBoxProblem === "signedOut" ? t("lab.thisBox.signInShort") : t("lab.thisBox.noAnswerShort")}
        </NativeSelectOption>
      )}
      {s.scenarios.map((x) => (
        <NativeSelectOption key={x.key} value={x.key}>
          {x.name}
        </NativeSelectOption>
      ))}
      {s.extras.map((x) => (
        <NativeSelectOption key={x.key} value={x.key}>
          {(x.key === "last" ? t("lab.top.last") : t("lab.top.file")) + ": " + x.name}
        </NativeSelectOption>
      ))}
    </NativeSelect>
  );
}

/** The row of tools along the top of the card. `details` is there when the
 *  inspector is a sheet (a narrow screen) and opens it. */
export function Toolbar({ details }: { details?: () => void }) {
  const s = useLab();
  const engine = useEngine();
  const t = useT();
  const file = React.useRef<HTMLInputElement>(null);
  const [ordering, setOrdering] = React.useState(false);
  const [cloudMode, setCloudMode] = React.useState<"open" | "save" | null>(null);
  const a = actions();

  // Which canvas draws: the GPU one once it has drawn, SVG before or instead.
  const gpu = s.gpu;
  const canvasLine =
    gpu.state === "gpu"
      ? t("lab.canvas.gpu", { backend: gpu.backend === "webgpu" ? "WebGPU" : "WebGL2" })
      : gpu.state === "loading" || gpu.state === "starting"
        ? t("lab.canvas.loading")
        : t("lab.canvas.svg");

  return (
    <div className="flex flex-wrap items-center gap-2 border-b border-hair px-3 py-2" data-testid="lab-toolbar">
      <DropdownMenu>
        <DropdownMenuTrigger
          render={<Button variant="secondary" size="sm" className="shrink-0 gap-1 pr-2" data-testid="file-menu" />}
        >
          {t("dg.file.menu")}
          <HugeiconsIcon icon={ArrowDown01Icon} size={14} strokeWidth={1.8} color="currentColor" className="text-muted" />
        </DropdownMenuTrigger>
        <DropdownMenuContent>
          <DropdownMenuItem onClick={() => a.canvas.newDiagram()} data-testid="file-new">
            {t("dg.file.new")}
          </DropdownMenuItem>
          <DropdownMenuItem onClick={() => file.current?.click()} data-testid="file-open">
            {t("dg.file.open")}
          </DropdownMenuItem>
          <DropdownMenuItem onClick={() => void a.save()} data-testid="file-download">
            {t("dg.file.download")}
          </DropdownMenuItem>
          {s.cloud && (
            <>
              <DropdownMenuSeparator />
              <DropdownMenuLabel>LosOS Cloud</DropdownMenuLabel>
              <DropdownMenuItem onClick={() => setCloudMode("open")} data-testid="cloud-open">
                {t("dg.file.cloudOpen")}
              </DropdownMenuItem>
              <DropdownMenuItem onClick={() => (s.cloudFile ? void a.saveQuick() : setCloudMode("save"))} data-testid="cloud-save-menu">
                {t("dg.file.cloudSave")}
              </DropdownMenuItem>
              {s.cloudFile && (
                <DropdownMenuItem onClick={() => setCloudMode("save")}>{t("dg.file.cloudSaveAs")}</DropdownMenuItem>
              )}
            </>
          )}
          <DropdownMenuSeparator />
          <DropdownMenuItem onClick={() => void a.exportPicture("svg")} data-testid="export-svg">
            {t("dg.file.svg")}
          </DropdownMenuItem>
          <DropdownMenuItem onClick={() => void a.exportPicture("png")} data-testid="export-png">
            {t("dg.file.png")}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      <CloudDialog mode={cloudMode} onClose={() => setCloudMode(null)} />
      <input
        ref={file}
        type="file"
        accept=".canvas,.llf,.json"
        hidden
        data-testid="open-file"
        onChange={(e) => {
          const f = e.target.files?.[0];
          e.target.value = "";
          if (f) void a.open(f);
        }}
      />
      <ButtonGroup aria-label={t("dg.history")} className="shrink-0">
        <Button variant="ghost" size="icon-sm" aria-label={t("dg.undo")} title={t("dg.undo")} disabled={s.live ? !s.live.canUndo : !s.canvas.canUndo} onClick={() => a.canvas.undo()} data-testid="undo">
          <HugeiconsIcon icon={ArrowTurnBackwardIcon} size={16} strokeWidth={1.5} color="currentColor" />
        </Button>
        <Button variant="ghost" size="icon-sm" aria-label={t("dg.redo")} title={t("dg.redo")} disabled={s.live ? !s.live.canRedo : !s.canvas.canRedo} onClick={() => a.canvas.redo()} data-testid="redo">
          <HugeiconsIcon icon={ArrowTurnForwardIcon} size={16} strokeWidth={1.5} color="currentColor" />
        </Button>
      </ButtonGroup>
      {s.ui.view !== "canvas" && (
        <ToggleGroup
          aria-label={t("lab.top.mode")}
          value={s.snap.sim.mode}
          onValueChange={(v) => a.setMode(v === "simulation" ? "simulation" : "realtime")}
          className="shrink-0"
        >
          <ToggleGroupItem value="realtime">{t("lab.mode.realtime")}</ToggleGroupItem>
          <ToggleGroupItem value="simulation">{t("lab.mode.simulation")}</ToggleGroupItem>
        </ToggleGroup>
      )}
      <span className="flex-1" />
      <LivePeople />
      {s.ordering?.enabled && (
        <>
          <Button variant="ghost" size="sm" title={t("lab.order.buttonTip")} onClick={() => setOrdering(true)} data-testid="order-open">
            <HugeiconsIcon icon={ShoppingCart01Icon} size={16} strokeWidth={1.5} color="currentColor" />
            {t("lab.order.button")}
          </Button>
          <OrderDialog open={ordering} onOpenChange={setOrdering} />
        </>
      )}
      <Tooltip>
        <TooltipTrigger
          render={<span tabIndex={0} />}
          className="min-w-0 rounded-control outline-none focus-visible:ring-2 focus-visible:ring-accent/40"
          aria-label={engine.available ? `${t("lab.engine.emulated")}: ${emulatedTip(engine.kinds)}` : `${t("lab.engine.simulated")}: ${engine.probe.reason}`}
        >
          <span className="inline-flex max-w-full min-w-0 items-center gap-1.5 truncate text-[12px] text-muted" data-testid="engine-badge">
            <StatusDot state={engine.available ? "ok" : "warn"} />
            {engine.available ? (
              <>
                <b className="font-semibold text-ink">{t("lab.engine.emulated")}</b>
                <span className="truncate">· {engine.probe.label}</span>
              </>
            ) : (
              <b className="font-semibold text-ink">{t("lab.engine.simulated")}</b>
            )}
          </span>
        </TooltipTrigger>
        <TooltipContent side="bottom" align="end" className="max-w-[360px]">
          {engine.available ? emulatedTip(engine.kinds) : engine.probe.reason}
          <br />
          {canvasLine}
        </TooltipContent>
      </Tooltip>
      {details && (
        <Button variant="ghost" size="sm" onClick={details} data-testid="details-open">
          <HugeiconsIcon icon={InformationCircleIcon} size={16} strokeWidth={1.5} color="currentColor" />
          {t("lab.top.details")}
        </Button>
      )}
    </div>
  );
}
