/* The others on a diagram edited together (shared.ts): their faces in the
 * top bar, and on the canvas their pointers with their names and an
 * outline round what they have selected, each in their own colour. The
 * colour is `data-accent`, which sets `--person` (styles/index.css): the
 * page's policy allows no style attribute, so no colour is written here. */

import { Faces } from "@/cloud/collab/faces";
import { StatusDot } from "@/components/ui/badge";
import { useT } from "@/lib/i18n-react";
import { useLab } from "../store";

/** Who has the diagram open, and whether the room is reachable. */
export function LivePeople() {
  const s = useLab("people");
  const t = useT();
  const l = s.live;
  if (!l) return null;
  return (
    <div className="flex shrink-0 items-center gap-2 text-[12px] text-muted" data-testid="live-people">
      {l.state !== "live" && (
        <span className="inline-flex items-center gap-1.5" role="status">
          <StatusDot state={l.state === "offline" ? "warn" : "pending"} />
          {l.state === "offline" ? t("dg.live.offline") : t("dg.live.connecting")}
        </span>
      )}
      {l.readOnly && <span>{t("dg.live.viewOnlyShort")}</span>}
      {l.people.length > 0 && <Faces people={l.people} me={l.myAddress} />}
    </div>
  );
}

/** Inside the canvas's transformed group: `k` is the camera's zoom, so
 *  pointers and names stay one size on screen. */
export function PeerLayer({ k }: { k: number }) {
  const s = useLab("people");
  const l = s.live;
  if (!l) return null;
  const nodes = new Map(s.canvas.nodes.map((n) => [n.id, n]));
  // The same person in two tabs is two pointers; this tab's own is not drawn.
  const others = l.people.filter((p) => p.client !== l.me);
  return (
    <g data-testid="peers" pointerEvents="none">
      {others.map((p) => (
        <g key={p.client} data-accent={p.user.accent} data-peer={p.user.address}>
          {p.sel.map((id) => {
            const n = nodes.get(id);
            return n ? (
              <rect key={id} x={n.x - 5 / k} y={n.y - 5 / k} width={n.width + 10 / k} height={n.height + 10 / k} rx={13} className="dg-peer-sel" strokeWidth={2 / k} />
            ) : null;
          })}
          {p.pointer && (
            <g transform={`translate(${p.pointer.x} ${p.pointer.y}) scale(${1 / k})`} className="dg-peer">
              <path d="M0 0L0 16L4.4 12.2L7.4 19L10 17.9L7.1 11.2L12.6 11.2Z" className="dg-peer-arrow" />
              <rect x={11} y={17} width={p.user.name.length * 6.6 + 12} height={19} rx={5} className="dg-peer-tag" />
              <text x={17} y={30.5} className="dg-peer-name">
                {p.user.name}
              </text>
            </g>
          )}
        </g>
      ))}
    </g>
  );
}
