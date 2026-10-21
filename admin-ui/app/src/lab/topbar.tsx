/* The top bar: back to the admin page, the diagram and the network's two
 * views, the file menu (new, open, save, LosOS Cloud, pictures), undo and
 * redo, the setup picker, the two modes (network views only), and the
 * engine badge. */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  ArrowDown01Icon,
  ArrowLeft01Icon,
  ArrowTurnBackwardIcon,
  ArrowTurnForwardIcon,
  InformationCircleIcon,
  ShoppingCart01Icon,
} from "@hugeicons/core-free-icons";
import { Button, buttonVariants } from "@/components/ui/button";
import { ButtonGroup } from "@/components/ui/button-group";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuLabel, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { StatusDot } from "@/components/ui/badge";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { actions } from "./actions";
import { CloudDialog } from "./canvas/cloud-dialog";
import { useEngine } from "./engine-hook";
import { logoUrl } from "./icons";
import { OrderDialog } from "./order-dialog";
import { BOX_COPY } from "./shape";
import { useLab } from "./store";

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

export function TopBar({ phone, onDetails }: { phone: boolean; onDetails: () => void }) {
  const s = useLab();
  const engine = useEngine();
  const t = useT();
  const file = React.useRef<HTMLInputElement>(null);
  const [ordering, setOrdering] = React.useState(false);
  const [cloudMode, setCloudMode] = React.useState<"open" | "save" | null>(null);
  const a = actions();
  const scenario = s.snap.scenario;
  const known = new Set([...s.scenarios.map((x) => x.key), ...s.extras.map((x) => x.key)]);

  // Which canvas draws: the GPU one once it has drawn, SVG before or instead.
  const gpu = s.gpu;
  const canvasLine =
    gpu.state === "gpu"
      ? t("lab.canvas.gpu", { backend: gpu.backend === "webgpu" ? "WebGPU" : "WebGL2" })
      : gpu.state === "loading" || gpu.state === "starting"
        ? t("lab.canvas.loading")
        : t("lab.canvas.svg");
  const badge = (
    <span
      className="inline-flex max-w-full min-w-0 items-center gap-1.5 truncate rounded-full border border-line bg-sunk px-2.5 py-1 text-[12px] text-muted"
      data-testid="engine-badge"
    >
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
  );

  return (
    <header className="flex flex-wrap items-center gap-3 border-b border-line bg-surface px-4 py-2 pt-[calc(8px+env(safe-area-inset-top,0px))] min-[901px]:flex-nowrap [grid-area:top]">
      {BOX_COPY && (
        <a
          href="/"
          className={cn(buttonVariants({ variant: "secondary", size: "sm" }), "gap-1 pl-1.5 no-underline")}
        >
          <HugeiconsIcon icon={ArrowLeft01Icon} size={14} strokeWidth={1.8} color="currentColor" className="text-muted" />
          {t("lab.top.admin")}
        </a>
      )}
      <div className="flex shrink-0 items-center gap-2 text-[17px] font-semibold whitespace-nowrap">
        <img src={logoUrl} alt="" width={26} height={26} className="size-[26px]" />
        LosOS Diagrams
      </div>
      <ToggleGroup
        aria-label={t("lab.top.workspace")}
        value={s.ui.view}
        onValueChange={(v) => a.setView(v === "physical" ? "physical" : v === "logical" ? "logical" : "canvas")}
        className="shrink-0"
      >
        <ToggleGroupItem value="canvas" data-testid="view-canvas">
          {t("dg.view.canvas")}
        </ToggleGroupItem>
        <ToggleGroupItem value="logical">{t("lab.view.logical")}</ToggleGroupItem>
        <ToggleGroupItem value="physical">{t("lab.view.physical")}</ToggleGroupItem>
      </ToggleGroup>
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
      <ButtonGroup aria-label={t("dg.history")} className="shrink-0">
        <Button variant="secondary" size="icon-sm" aria-label={t("dg.undo")} title={t("dg.undo")} disabled={!s.canvas.canUndo} onClick={() => a.canvas.undo()} data-testid="undo">
          <HugeiconsIcon icon={ArrowTurnBackwardIcon} size={15} strokeWidth={1.8} color="currentColor" />
        </Button>
        <Button variant="secondary" size="icon-sm" aria-label={t("dg.redo")} title={t("dg.redo")} disabled={!s.canvas.canRedo} onClick={() => a.canvas.redo()} data-testid="redo">
          <HugeiconsIcon icon={ArrowTurnForwardIcon} size={15} strokeWidth={1.8} color="currentColor" />
        </Button>
      </ButtonGroup>
      <NativeSelect
        aria-label={t("lab.top.setup")}
        className="max-w-full min-w-0 shrink min-[901px]:w-[280px]"
        value={known.has(scenario) ? scenario : ""}
        onChange={(e) => a.pick(e.target.value)}
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
      <span className="hidden flex-1 min-[901px]:block" />
      {s.ordering?.enabled && (
        <>
          <Button variant="secondary" size="sm" title={t("lab.order.buttonTip")} onClick={() => setOrdering(true)} data-testid="order-open">
            <HugeiconsIcon icon={ShoppingCart01Icon} size={15} strokeWidth={1.8} color="currentColor" />
            {t("lab.order.button")}
          </Button>
          <OrderDialog open={ordering} onOpenChange={setOrdering} />
        </>
      )}
      <Tooltip>
        <TooltipTrigger
          render={<span tabIndex={0} />}
          className="min-w-0 rounded-full outline-none focus-visible:ring-2 focus-visible:ring-accent/40"
          aria-label={engine.available ? `${t("lab.engine.emulated")}: ${emulatedTip(engine.kinds)}` : `${t("lab.engine.simulated")}: ${engine.probe.reason}`}
        >
          {badge}
        </TooltipTrigger>
        <TooltipContent side="bottom" align="end" className="max-w-[360px]">
          {engine.available ? emulatedTip(engine.kinds) : engine.probe.reason}
          <br />
          {canvasLine}
        </TooltipContent>
      </Tooltip>
      {phone && (
        <Button variant="secondary" size="sm" onClick={onDetails} className="ml-auto">
          <HugeiconsIcon icon={InformationCircleIcon} size={15} strokeWidth={1.8} color="currentColor" />
          {t("lab.top.details")}
        </Button>
      )}
    </header>
  );
}
