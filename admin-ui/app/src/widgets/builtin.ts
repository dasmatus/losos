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
 * Vocabulary: these strings are read by the owner of a mini-PC in their
 * hallway. "this box", "the mesh", "an app". Never a scheduler, a runtime or
 * a unit of either.
 */

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
    parts.push("Nothing watched yet");
  } else {
    parts.push(`${losos.fmt.percent(m.upRatio)} answered`);
    parts.push(
      m.outages === 0
        ? "no quiet spells"
        : `${m.outages} quiet ${losos.fmt.plural(m.outages, "spell")}`,
    );
    if (m.longestOutageSeconds > 0) {
      parts.push(`longest ${losos.fmt.duration(m.longestOutageSeconds)}`);
    }
  }

  const watched =
    m.observedDays === 0
      ? "This browser has not watched this box yet. Leave this page open and squares will fill in."
      : `Watched from this browser on ${m.observedDays} ${losos.fmt.plural(m.observedDays, "day")}.`;

  return {
    type: "heatmap",
    title: "Answering",
    data: {
      days: m.days,
      lowLabel: "Quiet",
      highLabel: "Answering",
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
      title: "Room on this box",
      data: {
        value: null,
        max: 0,
        format: "bytes",
        caption: "Not measured yet",
      },
      foot: "This box reports its size when you claim space on the Storage page.",
    };
  }

  const ratio = used === null ? null : losos.stat.ratio(used, total);
  const tight = ratio !== null && ratio >= 0.8;

  return {
    type: "bar",
    title: "Room on this box",
    data: {
      value: used,
      reserve,
      max: total + reserve,
      format: "bytes",
      fillLabel: "In use",
      reserveLabel: "Held back to grow into",
      ...(tight ? { chip: { text: "Filling up", tone: "warn" as const } } : {}),
      /* The headline reads "328 GB of 484 GB", where 484 is the whole disk —
       * the filesystem plus what is still unclaimed. The caption has to
       * reconcile that with the smaller figure the owner sees in their file
       * manager, or the two numbers look like a contradiction. */
      caption:
        used === null
          ? `${losos.fmt.bytes(total)} formatted, ${losos.fmt.bytes(reserve)} not yet claimed`
          : `${losos.fmt.bytes(total - used)} free now` +
            (reserve > 0 ? `, ${losos.fmt.bytes(reserve)} more to claim` : ""),
    },
    foot:
      reserve > 0
        ? `${losos.fmt.bytes(reserve)} is held back. You can claim it without opening the box.`
        : "All of the disk is in the filesystem.",
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
      title: "Sharing",
      data: {
        value: 0,
        max: 24,
        format: "duration",
        caption: "This box is on its own",
        fillLabel: "Yours",
      },
      foot: "Join other boxes from the Mesh page to lend and borrow time.",
    };
  }

  if (m.givenSeconds !== null && m.takenSeconds !== null) {
    const total = m.givenSeconds + m.takenSeconds;
    return {
      type: "bar",
      title: "Sharing",
      data: {
        value: m.takenSeconds,
        reserve: m.givenSeconds,
        max: total === 0 ? 1 : total,
        format: "duration",
        fillLabel: "Run for you elsewhere",
        reserveLabel: "Run here for others",
        caption: `${losos.fmt.duration(m.givenSeconds)} given · ${losos.fmt.duration(m.takenSeconds)} taken`,
      },
      foot: "Counted since this box joined.",
    };
  }

  const lent = m.windowHours;
  const kept = 24 - lent;

  return {
    type: "bar",
    title: "Sharing",
    data: {
      value: kept * 3600,
      reserve: lent * 3600,
      max: 24 * 3600,
      format: "duration",
      fillLabel: "Kept for you",
      reserveLabel: "Lent to the mesh",
      caption: m.sharingCompute
        ? `Spare time is lent between ${m.windowStart} and ${m.windowEnd}`
        : "Spare time is not lent right now",
      ...(m.sharingStorage ? { chip: { text: "Storage shared", tone: "neutral" as const } } : {}),
    },
    foot: "How much work each side has actually done is not reported yet.",
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
        ? "Answering"
        : app.reachable === false
          ? "Not answering"
          : "Not checked",
    tone: app.reachable === false ? ("crit" as const) : ("neutral" as const),
    texture: app.onMesh ? ("mesh" as const) : ("local" as const),
    badge: app.onMesh ? "on the mesh" : "on this box",
  }));

  return {
    type: "list",
    title: "Apps",
    data: {
      items,
      empty: "No apps yet. Finish setting the box up and they appear here.",
    },
    foot: `Served by ${m.hostName}.`,
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
        ? "Applying a change"
        : entry.state === "done"
          ? "Change applied"
          : entry.state === "failed"
            ? "Change failed"
            : "Idle",
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
    title: "Changes",
    data: {
      items,
      empty: "Nothing has been changed from this browser.",
    },
    foot: m.busy
      ? "A change is being applied now. The box may be slow for a few minutes."
      : "Only changes this browser was open for.",
  };
};
