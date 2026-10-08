/* The top bar: back to the admin page, the two views, the setup picker with
 * its files, the two modes, and the engine badge. */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { ArrowLeft01Icon, InformationCircleIcon, ShoppingCart01Icon } from "@hugeicons/core-free-icons";
import { Button, buttonVariants } from "@/components/ui/button";
import { ButtonGroup } from "@/components/ui/button-group";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { StatusDot } from "@/components/ui/badge";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { actions } from "./actions";
import { useEngine } from "./engine-hook";
import { plateUrl } from "./icons";
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
  const a = actions();
  const scenario = s.snap.scenario;
  const known = new Set([...s.scenarios.map((x) => x.key), ...s.extras.map((x) => x.key)]);

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
        <img src={plateUrl} alt="" width={26} height={26} className="size-[26px]" />
        LosOS Lab
      </div>
      <ToggleGroup
        aria-label={t("lab.top.workspace")}
        value={s.ui.view}
        onValueChange={(v) => a.setView(v === "physical" ? "physical" : "logical")}
        className="shrink-0"
      >
        <ToggleGroupItem value="logical">{t("lab.view.logical")}</ToggleGroupItem>
        <ToggleGroupItem value="physical">{t("lab.view.physical")}</ToggleGroupItem>
      </ToggleGroup>
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
      <ButtonGroup aria-label={t("lab.top.files")} className="shrink-0">
        <Button variant="secondary" size="sm" title={t("lab.top.saveTip")} onClick={() => void a.save()}>
          {t("lab.top.save")}
        </Button>
        <Button variant="secondary" size="sm" title={t("lab.top.openTip")} onClick={() => file.current?.click()}>
          {t("lab.top.open")}
        </Button>
      </ButtonGroup>
      <input
        ref={file}
        type="file"
        accept=".llf,.json"
        hidden
        data-testid="open-file"
        onChange={(e) => {
          const f = e.target.files?.[0];
          e.target.value = "";
          if (f) void a.open(f);
        }}
      />
      <ToggleGroup
        aria-label={t("lab.top.mode")}
        value={s.snap.sim.mode}
        onValueChange={(v) => a.setMode(v === "simulation" ? "simulation" : "realtime")}
        className="shrink-0"
      >
        <ToggleGroupItem value="realtime">{t("lab.mode.realtime")}</ToggleGroupItem>
        <ToggleGroupItem value="simulation">{t("lab.mode.simulation")}</ToggleGroupItem>
      </ToggleGroup>
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
