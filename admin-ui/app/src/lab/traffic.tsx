/* The traffic panel under the canvas: the sim bar (mode, lab clock, Play /
 * Step / Reset and speed in Simulation; the clock speeds, the skip to the
 * next timer and Clear in Realtime; the protocol filters) and the event
 * list, which folds away. Ported from side.js's renderSimBar / renderEvents. */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { ArrowDown01Icon, ArrowRight01Icon, ArrowUp01Icon, NextIcon, PauseIcon, PlayIcon } from "@hugeicons/core-free-icons";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { actions } from "./actions";
import { useLab } from "./store";

/** A protocol's colour: the house palette, picked in lab.css by data-proto. */
function Swatch({ proto, className }: { proto: string; className?: string }) {
  return (
    <i
      data-proto={proto}
      className={cn("lab-proto inline-block size-2.5 flex-none rounded-[3px]", className)}
      aria-hidden="true"
    />
  );
}

function speedLabel(v: number): string {
  return v === 3600 ? "1 h/s" : v === 600 ? "10 min/s" : `${v}×`;
}

function Live() {
  const s = useLab("frame");
  return (
    <>
      <span className="rounded-full border border-hair bg-sunk px-2 py-0.5 font-mono text-[12px] text-ink" data-testid="lab-clock">
        {s.frame.clockText || s.snap.clock.text}
      </span>
    </>
  );
}

function Status() {
  const s = useLab("frame");
  return (
    <span className="font-mono text-[12px] text-muted" data-testid="sim-status">
      {s.frame.status}
    </span>
  );
}

export function SimBar() {
  const s = useLab();
  const t = useT();
  const a = actions();
  const sim = s.snap.sim.mode === "simulation";
  const folded = s.ui.folded;
  const next = s.snap.clock.next;
  return (
    <div
      className="flex flex-none items-center gap-x-2 gap-y-1.5 border-b border-hair px-3 py-1.5 max-sm:overflow-x-auto sm:flex-wrap [&>*:not([role=group])]:flex-none"
      data-testid="sim-bar"
    >
      <Button
        variant="secondary"
        size="icon-xs"
        aria-label={folded ? t("lab.sim.showEvents") : t("lab.sim.hideEvents")}
        aria-expanded={!folded}
        onClick={() => {
          s.setUi({ folded: !folded });
          requestAnimationFrame(() => s.fit());
        }}
      >
        <HugeiconsIcon icon={folded ? ArrowUp01Icon : ArrowDown01Icon} size={14} strokeWidth={1.8} color="currentColor" />
      </Button>
      <span className="mr-1.5 text-[13px] font-semibold">{sim ? t("lab.mode.simulation") : t("lab.mode.realtime")}</span>
      <span title={t("lab.sim.clockTip")}>
        <Live />
      </span>
      {sim ? (
        <>
          <Button variant="secondary" size="xs" onClick={() => a.play(!s.snap.sim.playing)} data-testid="sim-play">
            <HugeiconsIcon icon={s.snap.sim.playing ? PauseIcon : PlayIcon} size={13} strokeWidth={1.8} color="currentColor" />
            {s.snap.sim.playing ? t("lab.sim.pause") : t("lab.sim.play")}
          </Button>
          <Button variant="secondary" size="xs" onClick={() => a.step()} data-testid="sim-step">
            {t("lab.sim.step")}
            <HugeiconsIcon icon={ArrowRight01Icon} size={13} strokeWidth={1.8} color="currentColor" />
          </Button>
          <Button variant="secondary" size="xs" onClick={() => a.clear()}>
            {t("lab.sim.reset")}
          </Button>
          <label className="inline-flex items-center gap-1.5 text-[12px] text-muted">
            {t("lab.sim.speed")}
            <input
              type="range"
              min={0.25}
              max={4}
              step={0.25}
              value={s.snap.sim.speed}
              onChange={(e) => a.setSimSpeed(Number(e.target.value))}
              className="w-28 accent-[var(--accent)]"
            />
          </label>
        </>
      ) : (
        <>
          <ToggleGroup
            size="sm"
            aria-label={t("lab.sim.clockSpeed")}
            value={String(s.snap.clock.speed)}
            onValueChange={(v) => a.setClockSpeed(Number(v))}
          >
            {s.catalog.speeds.map((v) => (
              <ToggleGroupItem key={v} value={String(v)}>
                {speedLabel(v)}
              </ToggleGroupItem>
            ))}
          </ToggleGroup>
          <Button variant="secondary" size="xs" title={t("lab.sim.skipTip", { title: next.title, at: next.at })} onClick={() => a.skip()}>
            <HugeiconsIcon icon={NextIcon} size={13} strokeWidth={1.8} color="currentColor" />
            {next.at}
          </Button>
          <Button variant="secondary" size="xs" onClick={() => a.clear()}>
            {t("lab.sim.clear")}
          </Button>
        </>
      )}
      <Status />
      <span className="flex-1" />
      <div className="flex min-w-0 flex-[0_1_auto] justify-end gap-1 max-sm:flex-none sm:flex-wrap" role="group" aria-label={t("lab.sim.filters")}>
        {Object.entries(s.catalog.proto).map(([k, v]) => {
          const on = s.snap.sim.filters[k] !== false;
          return (
            <button
              key={k}
              type="button"
              aria-pressed={on}
              data-proto={k}
              onClick={() => a.setFilter(k, !on)}
              className={cn(
                "inline-flex items-center gap-1.5 rounded-full border border-line bg-surface px-2 py-px text-[11.5px] text-ink",
                "focus-visible:ring-2 focus-visible:ring-accent/40 focus-visible:outline-none",
                !on && "opacity-45",
              )}
            >
              <Swatch proto={k} className="size-[9px]" />
              {v.label}
            </button>
          );
        })}
      </div>
    </div>
  );
}

export function EventList() {
  const s = useLab("events");
  useLab();
  const t = useT();
  const box = React.useRef<HTMLDivElement>(null);
  const sim = s.snap.sim.mode === "simulation";
  const filters = s.snap.sim.filters;
  const rows = s.events.filter((e) => filters[e.proto] !== false).slice(sim ? -300 : -60);
  React.useEffect(() => {
    const el = box.current;
    if (el) el.scrollTop = el.scrollHeight;
  });
  if (s.ui.folded) return null;
  return (
    <div ref={box} className="min-h-0 flex-1 overflow-auto" data-testid="event-list">
      {rows.length === 0 ? (
        <div className="px-3.5 py-2.5 text-[13px] text-muted">{sim ? t("lab.events.emptySim") : t("lab.events.emptyRealtime")}</div>
      ) : (
        // A bare <table>, not <Table>: its wrapper scrolls sideways, which
        // would stop the header sticking to this box.
        <table className="w-full text-[12px]">
          <TableHeader className="sticky top-0 z-[1] bg-sunk">
            <TableRow className="hover:bg-transparent">
              <TableHead className="h-7 px-2.5 text-[11px] font-semibold tracking-[0.05em] uppercase">{t("lab.events.time")}</TableHead>
              <TableHead className="h-7 px-2.5 text-[11px] font-semibold tracking-[0.05em] uppercase">{t("lab.events.last")}</TableHead>
              <TableHead className="h-7 px-2.5 text-[11px] font-semibold tracking-[0.05em] uppercase">{t("lab.events.at")}</TableHead>
              <TableHead className="h-7 px-2.5 text-[11px] font-semibold tracking-[0.05em] uppercase">{t("lab.events.type")}</TableHead>
              <TableHead className="h-7 px-2.5 text-[11px] font-semibold tracking-[0.05em] uppercase">{t("lab.events.info")}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.map((e, i) => {
              const p = s.catalog.proto[e.proto];
              return (
                <TableRow key={i} className={cn(i === rows.length - 1 && "bg-accent-wash hover:bg-accent-wash")}>
                  <TableCell className="px-2.5 py-1 text-[12px] font-mono whitespace-nowrap">{e.t}</TableCell>
                  <TableCell className="px-2.5 py-1 text-[12px] whitespace-nowrap">{s.name(e.last)}</TableCell>
                  <TableCell className="px-2.5 py-1 text-[12px] whitespace-nowrap">{s.name(e.at)}</TableCell>
                  <TableCell className="px-2.5 py-1 text-[12px] whitespace-nowrap">
                    <span className="inline-flex items-center gap-1.5 text-[11px] font-semibold">
                      <Swatch proto={e.proto} />
                      {p?.label ?? e.proto}
                    </span>
                    {e.real && (
                      <Badge variant="ok" className="ml-1 px-1.5 text-[10.5px] leading-4">{t("lab.events.guestFrame")}</Badge>
                    )}
                  </TableCell>
                  <TableCell className="min-w-[260px] px-2.5 py-1 text-[12px] whitespace-normal">
                    {e.info}
                    {e.fail && <Badge variant="crit" className="ml-1 px-1.5 text-[11px] leading-4">{t("lab.events.dropped")}</Badge>}
                  </TableCell>
                </TableRow>
              );
            })}
          </TableBody>
        </table>
      )}
    </div>
  );
}
