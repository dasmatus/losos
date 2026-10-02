/* The five widgets that ship with the box.
 *
 * Each one is the scripting API in its full form:
 *
 *   const widget: WidgetFn = async (losos) => ({ type, title, data, foot });
 *
 * They take the sandbox object and nothing else. No import of the API client,
 * no fetch, no storage — not because they could not (they are ordinary
 * modules), but because the moment one of them reaches around `losos.metric`
 * the allow-list stops describing what a widget can see, and the next person
 * reading this file would be reading a lie.
 *
 * Text goes through t() at RUN time, so a tile shows the language that was on
 * screen when it last ran; tile.tsx re-runs every tile when the language
 * changes.
 *
 * Vocabulary: these strings are read by the owner of a mini-PC in their
 * hallway. "this box", "the mesh", "an app". Never a scheduler, a runtime or
 * a unit of either.
 */

import { t } from "@/lib/i18n";
import type { WidgetFn } from "./types";

// ── Uptime ────────────────────────────────────────────────────────────────

/* A year of days, and a summary line that says where the reading came from.
 *
 * The source line is not decoration. This journal is kept by the browser
 * looking at the box — the box does not report its own history — so a day
 * nobody had this page open is a day with no reading, drawn as a gap rather
 * than as an outage. Saying so on the tile is the difference between a figure
 * the owner can rely on and one they only think they can. */
export const uptimeWidget: WidgetFn = async (losos) => {
  const m = await losos.metric("uptime.days", { days: 371 });

  const parts: string[] = [];
  if (m.observedDays === 0) {
    parts.push(t("widgets.builtin.uptime.nothingYet"));
  } else {
    parts.push(t("widgets.builtin.uptime.answered", { pct: losos.fmt.percent(m.upRatio) }));
    parts.push(
      m.outages === 0
        ? t("widgets.builtin.uptime.noQuiet")
        : t("widgets.builtin.uptime.quietSpells", { count: m.outages }),
    );
    if (m.longestOutageSeconds > 0) {
      parts.push(
        t("widgets.builtin.uptime.longest", {
          duration: losos.fmt.duration(m.longestOutageSeconds),
        }),
      );
    }
  }

  const watched =
    m.observedDays === 0
      ? t("widgets.builtin.uptime.notWatched")
      : t("widgets.builtin.uptime.watched", { count: m.observedDays });

  return {
    type: "heatmap",
    title: t("widgets.builtin.uptime.title"),
    data: {
      days: m.days,
      lowLabel: t("widgets.builtin.uptime.low"),
      highLabel: t("widgets.builtin.uptime.title"),
    },
    foot: `${parts.join(" · ")} · ${watched}`,
  };
};

// ── Disk ──────────────────────────────────────────────────────────────────

/* Room on this box.
 *
 * The bar is two segments of the SAME accent: filled for what is in use,
 * hatched for the space the installer held back so the box can grow without
 * being opened. One hue; the texture carries the distinction, the way it does
 * everywhere else in this app.
 *
 * Every figure here comes from a real measurement or is absent. The box has
 * no route that reports its disk; POST /api/grow returns sizes, and the
 * Storage screen hands them over after a claim (metrics.ts, recordGrow). Until
 * that has happened this tile draws an em-dash and says why. It does NOT
 * estimate, and it does not fall back to a browser storage quota, which is a
 * number about this tab and not about the box. */
export const diskWidget: WidgetFn = async (losos) => {
  const m = await losos.metric("storage.bytes");

  const total = m.totalBytes;
  const used = m.usedBytes;
  const reserve = m.reserveBytes ?? 0;

  if (total === null) {
    return {
      type: "bar",
      title: t("widgets.builtin.disk.title"),
      data: {
        value: null,
        max: 0,
        format: "bytes",
        caption: t("widgets.builtin.disk.notMeasured"),
      },
      foot: t("widgets.builtin.disk.reportsSize"),
    };
  }

  const ratio = used === null ? null : losos.stat.ratio(used, total);
  const tight = ratio !== null && ratio >= 0.8;

  return {
    type: "bar",
    title: t("widgets.builtin.disk.title"),
    data: {
      value: used,
      reserve,
      max: total + reserve,
      format: "bytes",
      fillLabel: t("widgets.builtin.disk.inUse"),
      reserveLabel: t("widgets.builtin.disk.heldBack"),
      ...(tight
        ? { chip: { text: t("widgets.builtin.disk.fillingUp"), tone: "warn" as const } }
        : {}),
      /* The headline reads "328 GB of 484 GB", where 484 is the whole disk —
       * the filesystem plus what is still unclaimed. The caption has to
       * reconcile that with the smaller figure the owner sees in their file
       * manager, or the two numbers look like a contradiction. */
      caption:
        used === null
          ? t("widgets.builtin.disk.formatted", {
              total: losos.fmt.bytes(total),
              reserve: losos.fmt.bytes(reserve),
            })
          : reserve > 0
            ? t("widgets.builtin.disk.freeMore", {
                free: losos.fmt.bytes(total - used),
                reserve: losos.fmt.bytes(reserve),
              })
            : t("widgets.builtin.disk.free", { free: losos.fmt.bytes(total - used) }),
    },
    foot:
      reserve > 0
        ? t("widgets.builtin.disk.footReserve", { reserve: losos.fmt.bytes(reserve) })
        : t("widgets.builtin.disk.footAll"),
  };
};

// ── Mesh ──────────────────────────────────────────────────────────────────

/* Time given to the mesh against time kept for the owner.
 *
 * Given and taken in seconds of work is what this tile wants, and nothing
 * reports either. What the box does state is the window — the hours a day it
 * lends its spare capacity — and that is a real, owner-set figure, so that is
 * what the bar divides: the day, filled for the hours that stay this box's
 * own, hatched for the hours it lends. If seconds ever start being reported,
 * the first branch below uses them instead and the labels stop being about
 * hours in a day. */
export const meshWidget: WidgetFn = async (losos) => {
  const m = await losos.metric("mesh.compute");

  if (!m.joined) {
    return {
      type: "bar",
      title: t("widgets.builtin.mesh.title"),
      data: {
        value: 0,
        max: 24,
        format: "duration",
        caption: t("widgets.builtin.mesh.alone"),
        fillLabel: t("widgets.builtin.mesh.yours"),
      },
      foot: t("widgets.builtin.mesh.joinHint"),
    };
  }

  if (m.givenSeconds !== null && m.takenSeconds !== null) {
    const total = m.givenSeconds + m.takenSeconds;
    return {
      type: "bar",
      title: t("widgets.builtin.mesh.title"),
      data: {
        value: m.takenSeconds,
        reserve: m.givenSeconds,
        max: total === 0 ? 1 : total,
        format: "duration",
        fillLabel: t("widgets.builtin.mesh.runElsewhere"),
        reserveLabel: t("widgets.builtin.mesh.runHere"),
        caption: t("widgets.builtin.mesh.givenTaken", {
          given: losos.fmt.duration(m.givenSeconds),
          taken: losos.fmt.duration(m.takenSeconds),
        }),
      },
      foot: t("widgets.builtin.mesh.counted"),
    };
  }

  const lent = m.windowHours;
  const kept = 24 - lent;

  return {
    type: "bar",
    title: t("widgets.builtin.mesh.title"),
    data: {
      value: kept * 3600,
      reserve: lent * 3600,
      max: 24 * 3600,
      format: "duration",
      fillLabel: t("widgets.builtin.mesh.kept"),
      reserveLabel: t("widgets.builtin.mesh.lent"),
      caption: m.sharingCompute
        ? t("widgets.builtin.mesh.lentBetween", { start: m.windowStart, end: m.windowEnd })
        : t("widgets.builtin.mesh.notLent"),
      ...(m.sharingStorage
        ? { chip: { text: t("widgets.builtin.mesh.storageShared"), tone: "neutral" as const } }
        : {}),
    },
    foot: t("widgets.builtin.mesh.notReported"),
  };
};

// ── Apps ──────────────────────────────────────────────────────────────────

/** Which apps this box serves, and whether each one answered just now. */
export const appsWidget: WidgetFn = async (losos) => {
  const m = await losos.metric("apps.list");

  const items = m.apps.map((app) => ({
    label: app.name,
    detail:
      app.reachable === true
        ? t("widgets.builtin.apps.answering")
        : app.reachable === false
          ? t("widgets.builtin.apps.notAnswering")
          : t("widgets.builtin.apps.notChecked"),
    tone: app.reachable === false ? ("crit" as const) : ("neutral" as const),
    texture: app.onMesh ? ("mesh" as const) : ("local" as const),
    badge: app.onMesh ? t("widgets.builtin.apps.onMesh") : t("widgets.builtin.apps.onBox"),
  }));

  return {
    type: "list",
    title: t("widgets.builtin.apps.title"),
    data: {
      items,
      empty: t("widgets.builtin.apps.empty"),
    },
    foot: t("widgets.builtin.apps.servedBy", { host: m.hostName }),
  };
};

// ── Rebuilds ──────────────────────────────────────────────────────────────

/* What this box has been asked to change, and how it went.
 *
 * Also a browser journal: lososd reports the CURRENT job and forgets the one
 * before it, so this is the history of what this browser was open to see. A
 * change applied from a phone while this laptop was shut does not appear
 * here, and the foot says so. */
export const rebuildsWidget: WidgetFn = async (losos) => {
  const m = await losos.metric("rebuilds.recent", { limit: 8 });

  const items = m.entries.map((entry) => ({
    label:
      entry.state === "building"
        ? t("widgets.builtin.rebuilds.applying")
        : entry.state === "done"
          ? t("widgets.builtin.rebuilds.applied")
          : entry.state === "failed"
            ? t("widgets.builtin.rebuilds.failed")
            : t("widgets.builtin.rebuilds.idle"),
    detail: `${losos.fmt.ago(entry.seenAt)}${entry.message.length > 0 ? ` · ${entry.message}` : ""}`,
    tone:
      entry.state === "failed"
        ? ("crit" as const)
        : entry.state === "done"
          ? ("ok" as const)
          : ("neutral" as const),
  }));

  return {
    type: "list",
    title: t("widgets.builtin.rebuilds.title"),
    data: {
      items,
      empty: t("widgets.builtin.rebuilds.empty"),
    },
    foot: m.busy ? t("widgets.builtin.rebuilds.busy") : t("widgets.builtin.rebuilds.onlyThis"),
  };
};
