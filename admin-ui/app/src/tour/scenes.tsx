/* The tour's scenes: short loops, each a pure function of its frame.
 *
 * Nothing here keeps state or reads the clock. A scene is handed a frame
 * between 0 and its `length` and draws that moment, so frame 0 is also the
 * last frame plus one: each loop starts composed, does its one thing, and
 * comes back to where it started. `still` is the frame shown with
 * prefers-reduced-motion, the moment that says what the scene is about.
 *
 * Timing is on a 120 BPM grid at 60 fps (motion.ts): a beat is 30 frames,
 * so the numbers below are beats and half-beats from the scene's start.
 * Content is drawn in the miniature's content box (mini-page.tsx,
 * CONTENT: 460 by 304), with real labels from the catalogue so each scene
 * reads in the language on screen, and the numbers are the same box the
 * handbook's pictures show: mattbox, 480 GB, 131 GB of its own.
 *
 * Every part is a plain SVG attribute or a palette class. See mini-page.tsx
 * for why there is no style attribute anywhere. */

import * as React from "react";
import {
  CloudIcon,
  File01Icon,
  Folder01Icon,
  GitBranchIcon,
  MusicNote01Icon,
  Search01Icon,
} from "@hugeicons/core-free-icons";
import { INTL_TAG, type MessageKey } from "@/lib/i18n";
import { useLocale, useT } from "@/lib/i18n-react";
import { CATALOGUE, type AppId } from "@/lib/apps";
import { logoFor } from "@/lib/logo";
import { describeWindow } from "@/screens/settings/window";
import { BAR, BEAT, E, clamp, fd, hitPulse, hold, lmix, mix, prog } from "./motion";
import { Glyph, Hatch, useSvgId, type MiniEntry } from "./mini-page";
import type { BoxFeatures, Feature } from "./features";
import { BuilderScene, ChatScene, LendScene, NameScene, ProfileScene, TogetherScene } from "./more-scenes";
import { Caption, Heading, W, wrap, type SceneProps } from "./parts";

export type SceneId =
  | "overview"
  | "together"
  | "apps"
  | "storage"
  | "mesh"
  | "lend"
  | "chat"
  | "byName"
  | "builder"
  | "profile"
  | "history"
  | "about";

export interface SceneDef {
  id: SceneId;
  entry: MiniEntry;
  /** Frames in one loop: whole bars. */
  length: number;
  /** The frame shown when motion is reduced. */
  still: number;
  Scene: (props: SceneProps) => React.ReactNode;
  /** Only on a box that has this feature (features.ts). */
  needs?: Feature;
}

/* ── 1. Overview: an app opens from its tile and folds back into it ─────── */

/* LosOS Cloud's six apps, as the Overview's tiles. The Board is the
 * catalogue's kanban app under the name LosOS Cloud gives it. */
const OVERVIEW_APPS: readonly { id: AppId; name?: MessageKey }[] = [
  { id: "files" },
  { id: "calendar" },
  { id: "contacts" },
  { id: "tasks" },
  { id: "notes" },
  { id: "deck", name: "tour.scene.board" },
];
const TILE = { x: 24, y: 60, size: 44, step: 72 } as const;

function OverviewScene({ frame: f }: SceneProps) {
  const t = useT();
  const apps = OVERVIEW_APPS.flatMap(({ id, name }) => {
    const app = CATALOGUE.find((entry) => entry.id === id);
    return app === undefined ? [] : [{ ...app, name: name ?? app.name }];
  });

  // Beat 1 the focus lands on Files, beat 2 it is pressed, then the app
  // grows out of its tile to fill the card, holds, and folds back in.
  const focus = hold(f, BEAT - 6, 10 * BEAT, 14, E.ui);
  const press = hitPulse(f - 2 * BEAT, 2, 5);
  const grow = prog(f, 2 * BEAT + 4, 3 * BEAT + 22, E.inOut) * (1 - prog(f, 8 * BEAT - 4, 9 * BEAT + 26, E.inOut));
  const inside = hold(f, 3 * BEAT + 10, 7 * BEAT + 18, 14);
  // While it is open, the pointer walks down the files, one a beat.
  const hover = hold(f, 4 * BEAT + 12, 7 * BEAT + 12, 12);
  const hoverRow = prog(f, 5 * BEAT, 5 * BEAT + 18, E.glide) + prog(f, 6 * BEAT, 6 * BEAT + 18, E.glide);

  const from = { x: TILE.x, y: TILE.y, w: TILE.size, h: TILE.size };
  const to = { x: 12, y: 50, w: W - 24, h: 132 };
  const win = {
    x: mix(from.x, to.x, grow),
    y: mix(from.y, to.y, grow),
    w: lmix(from.w, to.w, grow),
    h: lmix(from.h, to.h, grow),
  };

  // Where "Room left" starts: after "On this box 141 GB", by an estimate of its width.
  const roomX = 35 + t("home.storage.onThisBox").length * 5 + 5 + 6 * 5.4 + 20;

  const files: { icon: typeof Folder01Icon; name: string; size: string }[] = [
    { icon: Folder01Icon, name: t("tour.scene.documents"), size: "38 MB" },
    { icon: Folder01Icon, name: t("apps.photos.name"), size: "2.1 GB" },
    { icon: File01Icon, name: `${t("apps.notes.name")}.md`, size: "64 KB" },
  ];

  return (
    <>
      <Heading mono>mattbox</Heading>
      <circle cx="4" cy="26" r="2.6" className="fill-ok" />
      <text x="11" y="29" fontSize="9" className="fill-muted">
        {t("home.header.answering")}
      </text>

      <rect x="0.5" y="40.5" width={W - 1} height="152" rx="8" className="fill-surface stroke-line" />
      {apps.map((app, i) => {
        const x = TILE.x + i * TILE.step;
        const c = TILE.size / 2;
        const s = i === 0 ? 1 - 0.06 * press : 1;
        return (
          <g key={app.id} transform={`translate(${x + c} ${TILE.y + c}) scale(${s}) translate(${-c} ${-c})`}>
            <rect x="0.5" y="0.5" width={TILE.size - 1} height={TILE.size - 1} rx="9" className="fill-surface stroke-line" />
            <g className="text-ink">
              <Glyph icon={app.icon} x={12} y={12} size={20} />
            </g>
            <text x={c} y={TILE.size + 14} fontSize="9" textAnchor="middle" className="fill-ink">
              {t(app.name)}
            </text>
          </g>
        );
      })}
      <rect
        x={TILE.x - 3.5}
        y={TILE.y - 3.5}
        width={TILE.size + 7}
        height={TILE.size + 7}
        rx="11"
        fill="none"
        strokeWidth="1.5"
        className="stroke-accent"
        opacity={focus * (1 - grow)}
      />

      {/* The room the box has, as the Overview's own bar shows it. */}
      <rect x="24" y="146" width="412" height="6" rx="3" className="fill-sunk" />
      <rect x="24" y="146" width={412 * (141 / 516)} height="6" rx="3" className="fill-accent" />
      <rect x="24" y="164" width="7" height="7" rx="2" className="fill-accent" />
      <text x="35" y="170.5" fontSize="9" className="fill-ink">
        {t("home.storage.onThisBox")}
        <tspan dx="5" className="fill-muted font-mono">
          141 GB
        </tspan>
      </text>
      <rect x={roomX + 0.5} y="164.5" width="6" height="6" rx="2" fill="none" className="stroke-line" />
      <text x={roomX + 11} y="170.5" fontSize="9" className="fill-ink">
        {t("home.storage.roomLeft")}
        <tspan dx="5" className="fill-muted font-mono">
          375 GB
        </tspan>
      </text>

      <text x="2" y="220" fontSize="11.5" fontWeight="600" className="fill-ink">
        {t("widgets.board.heading")}
      </text>
      <rect x={W - 92.5} y="207.5" width="92" height="20" rx="5" className="fill-surface stroke-line" />
      <text x={W - 46.5} y="220.5" fontSize="9" textAnchor="middle" className="fill-ink">
        {t("widgets.board.add")}
      </text>
      {/* The first widget, running off the bottom edge as the page does. */}
      <rect x="0.5" y="236.5" width={W - 1} height="100" rx="8" className="fill-surface stroke-line" />
      <text x="16" y="256" fontSize="9" fontWeight="600" className="fill-muted">
        {t("widgets.catalogue.uptime.name").toUpperCase()}
      </text>
      {Array.from({ length: 52 }, (_, i) => (
        <rect key={i} x={16 + i * 8.2} y="266" width="6.4" height="6.4" rx="1.5" className="fill-accent" opacity={[7, 19, 33, 44].includes(i) ? 0.35 : 1} />
      ))}

      {/* The Files app, grown out of its tile. Its fill is clear while it is
          small, so the tiles and the bar under it come and go gradually. */}
      {grow > 0.001 && (
        <g>
          <rect
            x={win.x + 0.5}
            y={win.y + 0.5}
            width={Math.max(0, win.w - 1)}
            height={Math.max(0, win.h - 1)}
            rx={mix(9, 8, grow)}
            className="fill-surface stroke-line"
            fillOpacity={clamp((grow - 0.2) / 0.5, 0, 1)}
          />
          <g opacity={inside}>
            <g className="text-accent">
              <Glyph icon={Folder01Icon} x={to.x + 16} y={to.y + 12} size={14} />
            </g>
            <text x={to.x + 36} y={to.y + 23} fontSize="11" fontWeight="600" className="fill-ink">
              {t("apps.files.name")}
            </text>
            <line x1={to.x + 1} y1={to.y + 36.5} x2={to.x + to.w - 1} y2={to.y + 36.5} className="stroke-hair" />
            <rect x={to.x + 8} y={to.y + 40 + hoverRow * 29} width={to.w - 16} height="27" rx="5" className="fill-sunk" opacity={hover} />
            {files.map((file, i) => {
              const at = 3 * BEAT + 16 + i * 6;
              const o = prog(f, at, at + 16, E.out);
              const y = to.y + 40 + i * 29 + mix(6, 0, o);
              return (
                <g key={file.name} opacity={o}>
                  <g className="text-muted">
                    <Glyph icon={file.icon} x={to.x + 16} y={y + 7} size={13} />
                  </g>
                  <text x={to.x + 38} y={y + 17} fontSize="10" className="fill-ink">
                    {file.name}
                  </text>
                  <text x={to.x + to.w - 16} y={y + 17} fontSize="9" textAnchor="end" className="fill-muted font-mono">
                    {file.size}
                  </text>
                  {i < files.length - 1 && (
                    <line x1={to.x + 16} y1={y + 27.5} x2={to.x + to.w - 16} y2={y + 27.5} className="stroke-hair" />
                  )}
                </g>
              );
            })}
          </g>
        </g>
      )}
    </>
  );
}

/* ── 2. Apps: search, install, and the app joins the ones on this box ───── */

const QUERY = "jellyfin";

function AppsScene({ frame: f }: SceneProps) {
  const t = useT();
  const slot = (i: number) => ({ x: 22 + i * 92, y: 56 });

  // Typing on beats 1 to 2.5, deleted again at the end of the loop.
  const typed = f < 9 * BEAT ? clamp(fd((f - 20) / 6), 0, QUERY.length) : clamp(QUERY.length - fd((f - 9 * BEAT - 6) / 3), 0, QUERY.length);
  const caretOn = f >= 14 && f < 10 * BEAT + 10 && fd(f / 16) % 2 === 0;
  const result = hold(f, 2 * BEAT + 16, 6 * BEAT + 24, 14);
  const resultRise = mix(6, 0, prog(f, 2 * BEAT + 16, 3 * BEAT, E.out));
  const press = hitPulse(f - 4 * BEAT, 2, 5);
  const installing = prog(f, 4 * BEAT + 4, 6 * BEAT + 16, E.ui);
  // The new app's icon flies from the result row to the free slot.
  const fly = prog(f, 6 * BEAT + 20, 8 * BEAT, E.glide);
  const name = hold(f, 7 * BEAT + 26, 10 * BEAT + 4, 14);
  const fade = 1 - prog(f, 10 * BEAT + 4, 11 * BEAT, E.smooth);

  const rowIcon = { x: 12, y: 196, size: 26 };
  const target = slot(2);
  const flyer = {
    x: mix(rowIcon.x, target.x, fly),
    y: mix(rowIcon.y, target.y, fly),
    size: lmix(rowIcon.size, 36, fly),
  };

  const mine = [
    { icon: CloudIcon, name: "LosOS Cloud" },
    { icon: GitBranchIcon, name: "LosOS Git" },
  ];

  return (
    <>
      <Heading>{t("shell.nav.apps")}</Heading>
      <Caption y={34}>{t("panes.apps.onThisBox")}</Caption>
      <rect x="0.5" y="40.5" width={W - 1} height="84" rx="8" className="fill-surface stroke-line" />
      {mine.map((app, i) => {
        const { x, y } = slot(i);
        return (
          <g key={app.name}>
            <rect x={x + 0.5} y={y + 0.5} width="35" height="35" rx="8" className="fill-surface stroke-line" />
            <g className="text-ink">
              <Glyph icon={app.icon} x={x + 9} y={y + 9} size={18} />
            </g>
            <text x={x + 18} y={y + 52} fontSize="9" textAnchor="middle" className="fill-ink">
              {app.name}
            </text>
          </g>
        );
      })}
      <rect
        x={target.x + 0.5}
        y={target.y + 0.5}
        width="35"
        height="35"
        rx="8"
        fill="none"
        strokeDasharray="3 3"
        className="stroke-line"
        opacity={1 - fly * fade}
      />
      <text x={target.x + 18} y={target.y + 52} fontSize="9" textAnchor="middle" className="fill-ink" opacity={name * fade}>
        {QUERY}
      </text>

      <Caption y={148}>{t("panes.apps.findMore")}</Caption>
      <rect x="0.5" y="154.5" width={W - 1} height="26" rx="6" className="fill-surface stroke-line" />
      <g className="text-faint">
        <Glyph icon={Search01Icon} x={9} y={162} size={11} />
      </g>
      {typed === 0 ? (
        <text x="26" y="170.5" fontSize="9.5" className="fill-faint">
          {t("panes.apps.searchPlaceholder")}
        </text>
      ) : (
        <text x="26" y="170.5" fontSize="9.5" className="fill-ink font-mono">
          {QUERY.slice(0, typed)}
        </text>
      )}
      {caretOn && <rect x={26 + typed * 5.15 + 0.5} y="162" width="1" height="11" className="fill-ink" />}

      {/* The search's one answer, and its Install button. */}
      <g opacity={result} transform={`translate(0 ${resultRise})`}>
        <rect x="0.5" y="186.5" width={W - 1} height="48" rx="8" className="fill-surface stroke-line" />
        {fly < 0.001 && (
          <>
            <rect x={rowIcon.x + 0.5} y={rowIcon.y + 0.5} width={rowIcon.size - 1} height={rowIcon.size - 1} rx="6" className="fill-surface stroke-line" />
            <g className="text-ink">
              <Glyph icon={MusicNote01Icon} x={rowIcon.x + 6} y={rowIcon.y + 6} size={14} />
            </g>
          </>
        )}
        <text x="48" y="205" fontSize="10" fontWeight="600" className="fill-ink">
          {QUERY}
        </text>
        <text x="48" y="218" fontSize="8.5" className="fill-muted">
          A media server for your films and music.
        </text>
        <g transform={`translate(${W - 46} 210) scale(${1 - 0.06 * press}) translate(${-34} ${-11})`}>
          <rect width="68" height="22" rx="5" className="fill-accent" />
          <text x="34" y="14.2" fontSize="9.5" textAnchor="middle" className="fill-surface">
            {t("install.button")}
          </text>
        </g>
        <rect x="8" y="229" width={(W - 16) * installing} height="2" rx="1" className="fill-accent" opacity={installing > 0 ? 1 : 0} />
      </g>

      {/* The icon on its way to the free slot, then resting in it. */}
      {fly > 0.001 && (
        <g opacity={fade}>
          <rect
            x={flyer.x + 0.5}
            y={flyer.y + 0.5}
            width={flyer.size - 1}
            height={flyer.size - 1}
            rx={mix(6, 8, fly)}
            className="fill-surface stroke-line"
          />
          <g className="text-ink">
            <Glyph icon={MusicNote01Icon} x={flyer.x + flyer.size * 0.25} y={flyer.y + flyer.size * 0.25} size={flyer.size * 0.5} />
          </g>
        </g>
      )}
    </>
  );
}

/* ── 3. Storage: the meter, one part at a time ──────────────────────────── */

const DISK = { total: 480, yours: 131, lent: 96 } as const;

function StorageScene({ frame: f }: SceneProps) {
  const t = useT();
  const hatch = useSvgId("tour-hatch");
  const free = DISK.total - DISK.yours - DISK.lent;
  const track = { x: 16, y: 60, w: W - 32, h: 9 };
  const wYours = (track.w * DISK.yours) / DISK.total;
  const wLent = (track.w * DISK.lent) / DISK.total;

  // A beat in, the meter is read part by part, two beats each: yours, lent, free.
  const part = (i: number) => hold(f, BEAT + i * 2 * BEAT, BEAT + (i + 1) * 2 * BEAT - 4, 12, E.ui);
  const parts: readonly [number, number, number] = [part(0), part(1), part(2)];
  const any = Math.max(...parts);
  const dim = (i: 0 | 1 | 2) => mix(1, 0.3, any - parts[i]);
  const lift = (i: 0 | 1 | 2) => 2.5 * parts[i];

  // Laid out left to right by an estimate of each item's width (a 9.5 px
  // sans averages about 5.3 px a character, the mono figures 5.7).
  const items = [
    { key: "yours", label: t("settings.capacity.yours"), size: DISK.yours },
    { key: "lent", label: t("settings.capacity.lent"), size: DISK.lent },
    { key: "free", label: t("settings.capacity.free"), size: free },
  ];
  let cursor = 16;
  const legend = items.map((item) => {
    const x = cursor;
    cursor += 11 + item.label.length * 5.3 + 5 + `${item.size} GB`.length * 5.7 + 18;
    return { ...item, x };
  });

  return (
    <>
      <defs>
        <Hatch id={hatch} />
      </defs>
      <Heading>{t("shell.nav.storage")}</Heading>
      <Caption y={36}>{t("panes.storage.disk")}</Caption>
      <rect x="0.5" y="42.5" width={W - 1} height="120" rx="8" className="fill-surface stroke-line" />

      <rect x={track.x} y={track.y} width={track.w} height={track.h} rx="4.5" className="fill-sunk" opacity={dim(2)} />
      <rect
        x={track.x + wYours + wLent}
        y={track.y - lift(2)}
        width={track.w - wYours - wLent}
        height={track.h + 2 * lift(2)}
        rx="4.5"
        fill="none"
        strokeWidth="1.2"
        className="stroke-accent"
        opacity={parts[2]}
      />
      <rect x={track.x + wYours - 4} y={track.y - lift(1)} width={wLent + 4} height={track.h + 2 * lift(1)} fill={`url(#${hatch})`} opacity={dim(1)} />
      <rect x={track.x} y={track.y - lift(0)} width={wYours} height={track.h + 2 * lift(0)} rx="4.5" className="fill-accent" opacity={dim(0)} />
      <rect x={track.x + wYours - 4.5} y={track.y - lift(0)} width="4.5" height={track.h + 2 * lift(0)} className="fill-accent" opacity={dim(0)} />

      {legend.map((item, i) => (
        <g key={item.key} opacity={dim(i as 0 | 1 | 2)}>
          {item.key === "yours" && <rect x={item.x} y="85" width="7" height="7" rx="2" className="fill-accent" />}
          {item.key === "lent" && <rect x={item.x} y="85" width="7" height="7" rx="2" fill={`url(#${hatch})`} />}
          {item.key === "free" && <rect x={item.x + 0.5} y="85.5" width="6" height="6" rx="2" className="fill-sunk stroke-line" />}
          <text x={item.x + 11} y="91.5" fontSize="9.5" className="fill-muted">
            {item.label}
            <tspan dx="5" className="fill-ink font-mono">
              {item.size} GB
            </tspan>
          </text>
        </g>
      ))}
      <text x={W - 16} y="91.5" fontSize="9" textAnchor="end" className="fill-muted">
        {t("settings.capacity.total", { total: `${DISK.total} GB` })}
      </text>

      <line x1="0.5" y1="108.5" x2={W - 0.5} y2="108.5" className="stroke-hair" />
      <text x="16" y="139" fontSize="10" className="fill-ink">
        {t("panes.storage.held.title")}
      </text>
      <text x={W - 104} y="139" fontSize="9" textAnchor="end" className="fill-muted font-mono">
        48 GB
      </text>
      <rect x={W - 94.5} y="125.5" width="78" height="20" rx="5" className="fill-surface stroke-line" />
      <text x={W - 55.5} y="138.5" fontSize="9" textAnchor="middle" className="fill-ink">
        {t("panes.storage.useReserve")}
      </text>
      <text x="2" y="182" fontSize="9" className="fill-muted">
        {wrap(t("panes.storage.diskCaption"), 84).map((line, i) => (
          <tspan key={i} x="2" dy={i === 0 ? 0 : 13}>
            {line}
          </tspan>
        ))}
      </text>
    </>
  );
}

/* ── 4. Mesh: a day goes by; at night the box lends itself to the edge ──── */

const WINDOW = { start: 23, end: 7 } as const;

function inWindow(hour: number): boolean {
  const h = ((hour % 24) + 24) % 24;
  return WINDOW.start > WINDOW.end ? h >= WINDOW.start || h < WINDOW.end : h >= WINDOW.start && h < WINDOW.end;
}

function MeshScene({ frame: f }: SceneProps) {
  const t = useT();
  const hatch = useSvgId("tour-hatch");
  const clip = useSvgId("tour-strip");
  const length = 4 * BAR;

  // The day drifts past the fixed "now" line: noon at frame 0, noon again
  // at the end of the loop, midnight in the middle.
  const now = 12 + (24 * f) / length;
  const strip = { x: 16, y: 196, w: W - 32, h: 22 };
  const cell = strip.w / 24;
  const shift = (24 * f) / length; // hours the strip has moved
  const marker = strip.x + strip.w / 2;

  // How lent the box is: eased in over the window's first hour, out over the hour after it ends.
  const into = now - (WINDOW.start + (now < WINDOW.start ? -24 : 0));
  const span = (WINDOW.end - WINDOW.start + 24) % 24;
  const lending = E.smooth(clamp(into)) * E.smooth(clamp(span + 1 - into));

  const hh = Math.floor(now) % 24;
  const mm = Math.floor((now % 1) * 6) * 10;
  const clock = `${String(hh).padStart(2, "0")}:${String(mm).padStart(2, "0")}`;

  const box = { x: 40, y: 62, w: 100, h: 40 };
  const edge = { x: W - 156, y: 62, w: 116, h: 40 };
  const lineY = box.y + box.h / 2;
  const x1 = box.x + box.w;
  const x2 = edge.x;
  const gap = 46;
  const travel = (f * 1.1) % gap;

  const cells: React.ReactNode[] = [];
  for (let h = -1; h <= 25; h++) {
    const hour = Math.floor(shift) + h;
    const x = strip.x + (hour - shift) * cell;
    cells.push(
      <rect
        key={hour}
        x={x + 0.6}
        y={strip.y}
        width={cell - 1.2}
        height={strip.h}
        rx="1.5"
        fill={inWindow(hour) ? `url(#${hatch})` : undefined}
        className={inWindow(hour) ? undefined : "fill-sunk"}
      />,
    );
    if (hour % 6 === 0) {
      cells.push(
        <text key={`l${hour}`} x={x + 2} y={strip.y + strip.h + 11} fontSize="8" className="fill-faint font-mono">
          {String(((hour % 24) + 24) % 24).padStart(2, "0")}
        </text>,
      );
    }
  }

  return (
    <>
      <defs>
        <Hatch id={hatch} />
        <clipPath id={clip}>
          <rect x={strip.x} y={strip.y - 2} width={strip.w} height={strip.h + 16} />
        </clipPath>
      </defs>
      <Heading>{t("shell.nav.mesh")}</Heading>
      <Caption y={36}>{t("panes.mesh.edge.title")}</Caption>
      <rect x="0.5" y="42.5" width={W - 1} height="80" rx="8" className="fill-surface stroke-line" />

      <line x1={x1} y1={lineY} x2={x2} y2={lineY} strokeWidth="1.5" className="stroke-line" />
      <line x1={x1} y1={lineY} x2={x2} y2={lineY} strokeWidth="1.5" className="stroke-accent" opacity={lending} />
      <g opacity={lending}>
        {Array.from({ length: Math.ceil((x2 - x1) / gap) + 1 }, (_, i) => {
          const x = x1 + travel + i * gap - gap;
          if (x < x1 + 4 || x > x2 - 16) return null;
          return <rect key={i} x={x} y={lineY - 3.5} width="12" height="7" rx="2" fill={`url(#${hatch})`} />;
        })}
        <text x={(x1 + x2) / 2} y={lineY - 9} fontSize="9" textAnchor="middle" className="fill-accent">
          {t("tour.scene.lending")}
        </text>
      </g>

      <rect x={box.x + 0.5} y={box.y + 0.5} width={box.w - 1} height={box.h - 1} rx="8" className="fill-sunk stroke-line" />
      <text x={box.x + box.w / 2} y={lineY + 3} fontSize="10" textAnchor="middle" className="fill-ink font-mono">
        mattbox
      </text>
      <rect x={edge.x + 0.5} y={edge.y + 0.5} width={edge.w - 1} height={edge.h - 1} rx="8" className="fill-sunk stroke-line" />
      <text x={edge.x + edge.w / 2} y={lineY + 3} fontSize="8.5" textAnchor="middle" className="fill-ink font-mono">
        losos-edge.dasmat.us
      </text>

      <rect x="0.5" y="136.5" width={W - 1} height="136" rx="8" className="fill-surface stroke-line" />
      <text x="16" y="160" fontSize="10" className="fill-ink">
        {t("panes.mesh.lend")}
      </text>
      <rect x={W - 48} y="149" width="32" height="16" rx="8" className="fill-accent" />
      <circle cx={W - 24} cy="157" r="6" className="fill-surface" />
      <line x1="16" y1="176.5" x2={W - 16} y2="176.5" className="stroke-hair" />

      <g clipPath={`url(#${clip})`}>{cells}</g>
      <line x1={marker} y1={strip.y - 6} x2={marker} y2={strip.y + strip.h + 3} strokeWidth="1.2" className="stroke-ink" />
      <text x={marker} y={strip.y - 9} fontSize="8.5" textAnchor="middle" className="fill-ink font-mono">
        {clock}
      </text>
      <text x="16" y={strip.y + strip.h + 34} fontSize="9" className="fill-muted">
        {describeWindow("23:00", "07:00")}
      </text>
    </>
  );
}

/* ── 5. History: at three in the morning an update lands as a change ────── */

interface Change {
  subject: string;
  at: number;
  sha: string;
}

const CHANGES: readonly Change[] = [
  { subject: "Change hostName, cluster.enable", at: Date.UTC(2026, 9, 13, 21, 40), sha: "b41f07c2" },
  { subject: "Change shareCompute", at: Date.UTC(2026, 9, 12, 18, 2), sha: "9e2a51d0" },
  { subject: "Update to dasmatus/losos main", at: Date.UTC(2026, 9, 12, 3, 0), sha: "a088a5cb" },
];
const NEW_CHANGE: Change = { subject: "Update to dasmatus/losos main", at: Date.UTC(2026, 9, 14, 3, 0), sha: "4c7e19ab" };
const ROW = 36;

function HistoryScene({ frame: f }: SceneProps) {
  const t = useT();
  const locale = useLocale();
  const when = React.useMemo(
    () =>
      new Intl.DateTimeFormat(INTL_TAG[locale], {
        day: "numeric",
        month: "short",
        year: "numeric",
        hour: "2-digit",
        minute: "2-digit",
        timeZone: "UTC",
      }),
    [locale],
  );

  // The clock ticks to 03:00 on beat 1.5; the update lands as a new change
  // on top, arriving in the accent and settling to plain over a beat.
  const clockFade = 1 - hold(f, 9 * BEAT + 26, 10 * BEAT + 26, 14);
  const clock = f < 20 ? "02:58" : f < 42 ? "02:59" : f < 10 * BEAT + 26 ? "03:00" : "02:58";
  const open = prog(f, 44, 84, E.glide) * (1 - prog(f, 9 * BEAT + 26, 11 * BEAT, E.glide));
  const arrive = prog(f, 54, 80, E.smooth) * (1 - prog(f, 9 * BEAT + 16, 9 * BEAT + 34, E.smooth));
  const wash = 1 - prog(f, 4 * BEAT, 5 * BEAT + 10, E.smooth);

  const top = 52;
  const row = (change: Change, y: number, key: string, opacity = 1) => (
    <g key={key} opacity={opacity}>
      <text x="16" y={y + 15} fontSize="10" className="fill-ink">
        {change.subject}
      </text>
      <text x="16" y={y + 27} fontSize="8.5" className="fill-muted">
        {when.format(change.at)}
      </text>
      <text x={W - 16} y={y + 21} fontSize="9" textAnchor="end" className="fill-muted font-mono">
        {change.sha}
      </text>
    </g>
  );

  return (
    <>
      <Heading>{t("settings.panes.history.label")}</Heading>
      <text x={W} y="14" fontSize="12" textAnchor="end" className="fill-muted font-mono" opacity={clockFade}>
        {clock}
      </text>
      <Caption y={44}>{t("history.commits.title")}</Caption>
      <rect x="0.5" y={top - 1.5} width={W - 1} height={CHANGES.length * ROW + ROW * open + 4} rx="8" className="fill-surface stroke-line" />
      <rect x="1" y={top - 1} width={W - 2} height={ROW} rx="7.5" className="fill-accent-wash" opacity={arrive * wash} />
      {row(NEW_CHANGE, top, "new", arrive)}
      {CHANGES.map((change, i) => {
        const y = top + (i + open) * ROW;
        return (
          <g key={change.sha}>
            <line x1="1" y1={y + 0.5} x2={W - 1} y2={y + 0.5} className="stroke-hair" opacity={i === 0 ? open : 1} />
            {row(change, y, change.sha)}
          </g>
        );
      })}

      <rect x="0.5" y="214.5" width={W - 1} height="62" rx="8" className="fill-surface stroke-line" />
      <text x="16" y="236" fontSize="10" className="fill-ink">
        {t("panes.about.updates")}
      </text>
      <text x={W - 16} y="236" fontSize="9" textAnchor="end" className="fill-muted font-mono">
        {t("panes.about.everyNightAt", { time: "03:00" })}
      </text>
      <line x1="16" y1="245.5" x2={W - 16} y2="245.5" className="stroke-hair" />
      <text x="16" y="265" fontSize="10" className="fill-ink">
        {t("panes.about.restarts")}
      </text>
      <text x={W - 16} y="265" fontSize="9" textAnchor="end" className="fill-muted font-mono">
        {t("panes.about.everyNightAt", { time: "00:07" })}
      </text>
    </>
  );
}

/* ── 6. About: the row that opens the tour, and the tour opening ────────── */

function AboutScene({ frame: f }: SceneProps) {
  const t = useT();
  const steps = React.useContext(TourLength);
  const logo = logoFor();

  // The Tour row is picked out on beat 1, pressed on beat 2, and the tour
  // grows out of it, holds for two bars' worth of beats, and folds back.
  const ring = hold(f, BEAT - 10, 7 * BEAT + 4, 14, E.ui);
  const press = hitPulse(f - 2 * BEAT, 2, 5);
  const grow = prog(f, 2 * BEAT + 4, 3 * BEAT + 14, E.inOut) * (1 - prog(f, 6 * BEAT - 6, 7 * BEAT - 2, E.inOut));
  const inside = hold(f, 3 * BEAT + 4, 5 * BEAT + 22, 12);

  const rows: { title: string; value: string; kind?: "link" | "mono" | "badge" }[] = [
    { title: t("panes.about.called"), value: "mattbox", kind: "mono" },
    { title: t("panes.about.reachedAt"), value: "http://mattbox.local", kind: "mono" },
    { title: t("panes.about.storage"), value: t("panes.about.storageLocal"), kind: "badge" },
  ];
  const tourY = 186;
  const from = { x: 4, y: tourY + 4, w: W - 8, h: 26 };
  const to = { x: 70, y: 28, w: W - 140, h: 232 };
  const box = {
    x: mix(from.x, to.x, grow),
    y: mix(from.y, to.y, grow),
    w: mix(from.w, to.w, grow),
    h: lmix(from.h, to.h, grow),
  };

  return (
    <>
      <Heading>{t("settings.panes.about.label")}</Heading>
      <Caption y={36}>{t("panes.about.thisBox")}</Caption>
      <rect x="0.5" y="42.5" width={W - 1} height={rows.length * 34} rx="8" className="fill-surface stroke-line" />
      {rows.map((row, i) => {
        const y = 43 + i * 34;
        return (
          <g key={row.title}>
            {i > 0 && <line x1="16" y1={y + 0.5} x2={W - 16} y2={y + 0.5} className="stroke-hair" />}
            <text x="16" y={y + 20.5} fontSize="10" className="fill-ink">
              {row.title}
            </text>
            {row.kind === "badge" && (
              <rect x={W - 26 - row.value.length * 5.3} y={y + 8} width={row.value.length * 5.3 + 18} height="18" rx="9" className="fill-accent-wash" />
            )}
            <text
              x={row.kind === "badge" ? W - 17 - 0 : W - 16}
              y={y + 20.5}
              fontSize="9.5"
              textAnchor="end"
              className={row.kind === "badge" ? "fill-accent" : "fill-ink font-mono"}
            >
              {row.value}
            </text>
          </g>
        );
      })}

      <Caption y={tourY - 6}>{t("panes.about.tour")}</Caption>
      <g transform={`translate(${W / 2} ${tourY + 17}) scale(${1 - 0.012 * press}) translate(${-W / 2} ${-(tourY + 17)})`}>
        <rect x="0.5" y={tourY + 0.5} width={W - 1} height="34" rx="8" className="fill-surface stroke-line" />
        <text x="16" y={tourY + 21} fontSize="10" className="fill-ink">
          {t("panes.about.tour")}
        </text>
        <text x={W - 16} y={tourY + 21} fontSize="9.5" textAnchor="end" className="fill-accent font-mono">
          {t("panes.about.tourLink")}
        </text>
      </g>
      <rect
        x="4.5"
        y={tourY + 4.5}
        width={W - 9}
        height="25"
        rx="6"
        fill="none"
        strokeWidth="1.5"
        className="stroke-accent"
        opacity={ring * (1 - grow)}
      />
      <text x="2" y={tourY + 52} fontSize="9" className="fill-muted">
        {t("panes.about.tourCaption")}
      </text>

      {/* The tour itself, opening out of the row: this very dialog, small. */}
      {grow > 0.001 && (
        <g>
          <rect x="-166" y="-42" width="640" height="360" className="fill-black" opacity={0.4 * grow} />
          {/* Its fill comes up over the first frames of the growth, so the row
              under it fades out instead of vanishing in one frame. */}
          <rect
            x={box.x + 0.5}
            y={box.y + 0.5}
            width={Math.max(0, box.w - 1)}
            height={Math.max(0, box.h - 1)}
            rx="8"
            className="fill-surface stroke-line"
            fillOpacity={clamp(grow * 3, 0, 1)}
          />
          <g opacity={inside}>
            <rect x={to.x + 10.5} y={to.y + 10.5} width={to.w - 21} height="118" rx="5" className="fill-sunk stroke-line" />
            <image href={logo.url} x={to.x + to.w / 2 - 22} y={to.y + 47} width="44" height="44" />
            <text x={to.x + 12} y={to.y + 148} fontSize="8.5" className="fill-faint font-mono">
              {t("tour.count", { n: "1", total: String(steps) })}
            </text>
            <text x={to.x + 12} y={to.y + 164} fontSize="11" fontWeight="600" className="fill-ink">
              {t("tour.overview.title")}
            </text>
            <line x1={to.x + 1} y1={to.y + 186.5} x2={to.x + to.w - 1} y2={to.y + 186.5} className="stroke-hair" />
            {Array.from({ length: steps }, (_, i) => (
              <circle key={i} cx={to.x + 16 + i * 9} cy={to.y + 209} r="2.6" className={i === 0 ? "fill-accent" : "fill-line"} />
            ))}
            <rect x={to.x + to.w - 66} y={to.y + 198} width="54" height="22" rx="5" className="fill-accent" />
            <text x={to.x + to.w - 39} y={to.y + 212.5} fontSize="9" textAnchor="middle" className="fill-surface">
              {t("tour.next")}
            </text>
          </g>
        </g>
      )}
    </>
  );
}

/* Every step, in the sidebar's order, so the teal entry only ever travels
 * down it. Six are on every box. The rest each need a feature the box
 * reports (features.ts), and each sits by the section it lives in: writing
 * together opens from a document, lending under Market, the name under
 * Network, Build with AI under Look, the profile under Your account and
 * chat under its own entry. */
export const SCENES: readonly SceneDef[] = [
  { id: "overview", entry: "overview", length: 3 * BAR, still: BEAT + 20, Scene: OverviewScene },
  { id: "together", entry: "overview", length: 3 * BAR, still: 6 * BEAT, Scene: TogetherScene, needs: "office" },
  { id: "apps", entry: "apps", length: 3 * BAR, still: 7 * BEAT + 10, Scene: AppsScene },
  { id: "storage", entry: "storage", length: 3 * BAR, still: 4 * BEAT, Scene: StorageScene },
  { id: "mesh", entry: "mesh", length: 4 * BAR, still: 2 * BAR, Scene: MeshScene },
  { id: "lend", entry: "market", length: 3 * BAR, still: 8 * BEAT, Scene: LendScene, needs: "lend" },
  { id: "chat", entry: "chat", length: 3 * BAR, still: 7 * BEAT, Scene: ChatScene, needs: "chat" },
  { id: "byName", entry: "network", length: 3 * BAR, still: 8 * BEAT, Scene: NameScene, needs: "byName" },
  { id: "builder", entry: "look", length: 3 * BAR, still: 7 * BEAT, Scene: BuilderScene, needs: "builder" },
  { id: "profile", entry: "account", length: 2 * BAR, still: 5 * BEAT, Scene: ProfileScene, needs: "profile" },
  { id: "history", entry: "history", length: 3 * BAR, still: 4 * BEAT + 10, Scene: HistoryScene },
  { id: "about", entry: "about", length: 2 * BAR, still: 4 * BEAT + 10, Scene: AboutScene },
];

/** The steps this box gets: the six every box has, and each other one only where its feature is. */
export function scenesFor(features: BoxFeatures): readonly SceneDef[] {
  return SCENES.filter((scene) => scene.needs === undefined || features[scene.needs]);
}

/** How many steps the tour on screen has, for the About scene's picture of the tour itself. */
export const TourLength = React.createContext(SCENES.length);

/** The scene at `index` of `scenes`, clamped into the list. */
export function sceneAt(scenes: readonly SceneDef[], index: number): SceneDef {
  const first = scenes[0] as SceneDef;
  return scenes[Math.min(scenes.length - 1, Math.max(0, index))] ?? first;
}
