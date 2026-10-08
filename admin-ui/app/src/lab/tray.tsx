/* The parts tray along the bottom: four categories, and the devices or the
 * cable kinds of the chosen one. Picking one arms the place or connect tool;
 * Esc disarms it. */

import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { actions } from "./actions";
import { useEngine } from "./engine-hook";
import { DeviceIcon } from "./icons";
import { useLab } from "./store";

const CATS = ["losos", "net", "end", "links"] as const;
type Cat = (typeof CATS)[number];
const CAT_KEY = {
  losos: "lab.tray.losos",
  net: "lab.tray.net",
  end: "lab.tray.end",
  links: "lab.tray.links",
} as const;

const item = cn(
  "flex w-[116px] flex-none flex-col items-center gap-0.5 rounded-card border border-line bg-surface px-1.5 pt-1 pb-1.5",
  "text-center text-[11.5px] leading-tight text-ink transition-colors hover:border-accent",
  "aria-pressed:border-accent aria-pressed:bg-accent-wash aria-pressed:shadow-[inset_0_0_0_1px_var(--accent)]",
  "focus-visible:ring-2 focus-visible:ring-accent/40 focus-visible:outline-none",
);

export function Tray() {
  const s = useLab();
  const engine = useEngine();
  const t = useT();
  const a = actions();
  const cat = (CATS as readonly string[]).includes(s.ui.cat) ? (s.ui.cat as Cat) : "losos";
  return (
    <nav
      aria-label={t("lab.tray.label")}
      className="flex min-w-0 flex-col border-t border-line bg-surface pb-[env(safe-area-inset-bottom,0px)] min-[901px]:flex-row [grid-area:tray]"
    >
      <div className="grid grid-cols-2 content-center gap-0.5 border-b border-hair p-1.5 min-[901px]:border-r min-[901px]:border-b-0">
        {CATS.map((k) => (
          <button
            key={k}
            type="button"
            aria-pressed={cat === k}
            onClick={() => s.setUi({ cat: k })}
            className={cn(
              "rounded-control px-2.5 py-0.5 text-left text-[12.5px] whitespace-nowrap text-muted",
              "aria-pressed:bg-accent-wash aria-pressed:font-semibold aria-pressed:text-ink",
              "focus-visible:ring-2 focus-visible:ring-accent/40 focus-visible:outline-none",
            )}
          >
            {t(CAT_KEY[k])}
          </button>
        ))}
      </div>
      <div className="flex min-w-0 flex-1 items-stretch gap-1.5 overflow-x-auto px-3 py-2" data-testid="tray-items">
        {cat === "links" ? (
          <>
            {Object.entries(s.catalog.linkKinds).map(([k, v]) => {
              const stroke = k === "fiber" ? "var(--lab-fiber)" : k === "wifi" ? "var(--lab-wifi)" : k === "auto" ? "var(--ink)" : "var(--lab-copper)";
              return (
                <button
                  key={k}
                  type="button"
                  className={item}
                  aria-pressed={s.ui.tool === "connect" && s.ui.linkKind === k}
                  data-link-kind={k}
                  onClick={() => a.chooseLink(k)}
                >
                  <svg viewBox="0 0 64 56" className="h-[34px] w-10" aria-hidden="true">
                    <path
                      d="M8 44C24 44 18 12 56 12"
                      fill="none"
                      stroke={stroke}
                      strokeWidth="3.5"
                      {...(k === "wifi" ? { strokeDasharray: "2 6", strokeLinecap: "round" as const } : {})}
                    />
                    {k === "auto" && <path d="M44 34l4-8 4 8-4 8z" fill="var(--accent)" />}
                  </svg>
                  <span>{v.label}</span>
                  <small className="text-[10.5px] text-muted [text-wrap:balance]">{v.hint}</small>
                </button>
              );
            })}
            <button type="button" className={item} aria-pressed={s.ui.tool === "delete"} onClick={() => a.toggleDelete()} data-tool="delete">
              <svg viewBox="0 0 64 56" className="h-[34px] w-10" aria-hidden="true">
                <path d="M20 16l24 24M44 16L20 40" stroke="var(--crit)" strokeWidth="4" strokeLinecap="round" />
              </svg>
              <span>{t("lab.tray.delete")}</span>
              <small className="text-[10.5px] text-muted [text-wrap:balance]">{t("lab.tray.deleteHint")}</small>
            </button>
          </>
        ) : (
          Object.entries(s.catalog.types)
            .filter(([, T]) => T.cat === cat)
            .map(([k, T]) => (
              <button
                key={k}
                type="button"
                className={item}
                title={T.blurb}
                aria-pressed={s.ui.tool === "place" && s.ui.placeType === k}
                data-type={k}
                onClick={() => a.choosePlace(k)}
              >
                <DeviceIcon type={k} size={40} />
                <span>{T.short}</span>
                <small className="text-[10.5px] text-muted [text-wrap:balance]">
                  {T.cat === "losos"
                    ? engine.available
                      ? engine.kinds[0] === "libvirt" || engine.kinds[0] === "virt-rpc"
                        ? t("lab.tray.bootsLibvirt")
                        : t("lab.tray.bootsQemu")
                      : t("lab.tray.lososDevice")
                    : T.cat === "end"
                      ? "LosOS Desktop"
                      : t("lab.tray.gear")}
                </small>
              </button>
            ))
        )}
      </div>
    </nav>
  );
}
