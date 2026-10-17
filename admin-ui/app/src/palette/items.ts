import type { IconSvgElement } from "@hugeicons/react";
import {
  BookOpen01Icon,
  ComputerIcon,
  Home01Icon,
  LanguageSquareIcon,
  Logout01Icon,
  Moon02Icon,
  NetworkIcon,
  RoadLocation01Icon,
  Sun03Icon,
} from "@hugeicons/core-free-icons";
import { canUse, signOut } from "@/lib/api";
import { CATALOGUE, recallAvailability, type AppDefinition } from "@/lib/apps";
import { HANDBOOK_BASE } from "@/lib/handbook";
import { LOCALE_NAMES, LOCALES, getLocaleChoice, setLocale, t, template, type MessageKey } from "@/lib/i18n";
import type { Pin, PinView } from "@/lib/pins";
import { paneHref } from "@/lib/routes";
import { getTheme, setTheme, type Theme } from "@/lib/theme";
import { SETTINGS_PANES, paneOpen } from "@/screens/settings/panes";
import { openTour } from "@/tour/store";

/* The palette's own entries: everything it can list without asking the box.
 *
 * Built while the palette renders, so every string is in the language on
 * screen. `words` is what the query is matched against besides the title:
 * the English name too, so an owner reading Slovak can still type "storage",
 * and the same keywords the sidebar's panes carry. */

export type Target =
  /** A route of this page. */
  | { kind: "route"; to: string; state?: unknown }
  /** Another app on this origin (LosOS cloud, LosOS Git, the Lab, the
   *  handbook): a real navigation. */
  | { kind: "href"; href: string; newTab?: boolean }
  | { kind: "run"; run: () => void };

export type GroupId = "pins" | "pages" | "apps" | "files" | "repos" | "text" | "install" | "actions";

export interface Item {
  /** Unique across the whole list; also the option's DOM id suffix. */
  id: string;
  group: GroupId;
  title: string;
  /** A second, quieter line: the path of a file, a repository's description. */
  detail?: string | null;
  /** Beside the title, quieter still: where a text match is. */
  location?: string;
  /** In place of the detail: text with what matched marked, as alternating
   *  plain and matched runs. */
  snippet?: readonly string[];
  icon: IconSvgElement;
  words?: string;
  target: Target;
  /** What its right-click menu pins to the sidebar (components/pin-menu.tsx). */
  pin?: Pin;
}

/** Lower-case and strip diacritics: "Úložisko" -> "ulozisko". */
export function fold(text: string): string {
  return text
    .toLowerCase()
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "");
}

/* How well an item matches, or null for not at all. Every typed word has to
 * appear somewhere (typing a second word narrows), and then a title that
 * starts with the query beats one with a word that starts with it, which
 * beats one that only contains it, which beats a keyword hit. */
export function score(item: Item, query: string): number | null {
  const q = fold(query.trim());
  if (q.length === 0) return 0;
  const terms = q.split(/\s+/);
  const title = fold(item.title);
  const haystack = `${title} ${fold(item.detail ?? "")} ${fold(item.words ?? "")}`;
  if (!terms.every((term) => haystack.includes(term))) return null;
  if (title.startsWith(q)) return 3;
  if (title.split(/[\s/._-]+/).some((word) => word.startsWith(terms[0] ?? q))) return 2;
  if (title.includes(q)) return 1;
  return 0;
}

/** The items that match, best first; ties keep their order. */
export function rank(items: readonly Item[], query: string): Item[] {
  return items
    .map((item, index) => ({ item, index, score: score(item, query) }))
    .filter((entry): entry is { item: Item; index: number; score: number } => entry.score !== null)
    .sort((a, b) => b.score - a.score || a.index - b.index)
    .map((entry) => entry.item);
}

function both(key: MessageKey): string {
  return `${t(key)} ${template(key, undefined, "en")}`;
}

export function pageItems(): Item[] {
  const items: Item[] = [
    {
      id: "page-overview",
      group: "pages",
      title: t("shell.nav.overview"),
      icon: Home01Icon,
      words: `${template("shell.nav.overview", undefined, "en")} home start widgets board dashboard`,
      target: { kind: "route", to: "/" },
    },
  ];
  for (const pane of SETTINGS_PANES) {
    // The panes this person's sections do not open are not offered.
    if (!paneOpen(pane.id)) continue;
    items.push({
      id: `page-${pane.id}`,
      group: "pages",
      title: pane.label,
      detail: pane.summary,
      icon: pane.icon,
      words: [
        template(pane.labelKey, undefined, "en"),
        t(pane.keywordsKey),
        pane.keywords.join(" "),
      ].join(" "),
      target: { kind: "route", to: paneHref(pane.id) },
    });
  }
  if (canUse("lab")) items.push({
    id: "page-lab",
    group: "pages",
    title: t("shell.nav.lab"),
    icon: NetworkIcon,
    words: `${template("shell.nav.lab", undefined, "en")} simulator topology diagram network map`,
    target: { kind: "href", href: "/lab/" },
  });
  return items;
}

/* The LosOS cloud apps and LosOS Git, as the Overview's tiles have them:
 * only what this browser last saw answering (lib/apps.ts), so the palette
 * does not offer a link the grid would have hidden. Settings is left out;
 * it is a page here already. */
export function appItems(): Item[] {
  const seen = recallAvailability();
  const shown = (app: AppDefinition): boolean => {
    if (app.provider === "box") return false;
    if (app.provider === "code") return seen.code !== "absent";
    if (app.optional === true) return seen.mail === "present";
    return seen.files !== "absent";
  };
  return CATALOGUE.filter(shown).map((app) => ({
    id: `app-${app.id}`,
    group: "apps" as const,
    title: t(app.name),
    detail: t(app.provider === "code" ? "palette.inGit" : "palette.inCloud"),
    icon: app.icon,
    words: `${template(app.name, undefined, "en")} ${both(app.note)}`,
    target: { kind: "href", href: app.provider === "code" ? "/forgejo/" : `/nextcloud${app.path}` },
    pin: { kind: "tile", app: app.id },
  }));
}

/* The owner's pins (lib/pins.ts), in their own order, each opening where
 * its sidebar row does. */
export function pinItems(pinned: readonly { pin: Pin; view: PinView }[]): Item[] {
  return pinned.map(({ pin, view }) => ({
    id: `pin-${view.key}`,
    group: "pins" as const,
    title: view.labelKey !== null ? t(view.labelKey) : view.label,
    detail: view.detail,
    icon: view.icon,
    words: view.labelKey !== null ? template(view.labelKey, undefined, "en") : undefined,
    target: view.inApp
      ? { kind: "route" as const, to: view.href }
      : { kind: "href" as const, href: view.href, newTab: view.newTab },
    pin,
  }));
}

const THEME_ITEMS: Record<Theme, { key: MessageKey; icon: IconSvgElement }> = {
  light: { key: "palette.action.light", icon: Sun03Icon },
  dark: { key: "palette.action.dark", icon: Moon02Icon },
  auto: { key: "palette.action.auto", icon: ComputerIcon },
};

export function actionItems(): Item[] {
  const items: Item[] = [];
  const theme = getTheme();
  for (const [value, { key, icon }] of Object.entries(THEME_ITEMS) as [Theme, (typeof THEME_ITEMS)[Theme]][]) {
    if (value === theme) continue;
    items.push({
      id: `action-theme-${value}`,
      group: "actions",
      title: t(key),
      icon,
      words: `${template(key, undefined, "en")} theme colour color appearance mode`,
      target: { kind: "run", run: () => setTheme(value) },
    });
  }
  const locale = getLocaleChoice();
  for (const next of LOCALES) {
    if (next === locale) continue;
    items.push({
      id: `action-lang-${next}`,
      group: "actions",
      // The language's own name, so it reads right whatever is on screen.
      title: t("palette.action.language", { language: LOCALE_NAMES[next] }),
      icon: LanguageSquareIcon,
      words: `${template("palette.action.language", { language: LOCALE_NAMES[next] }, "en")} language jazyk sprache`,
      target: { kind: "run", run: () => setLocale(next) },
    });
  }
  items.push(
    {
      id: "action-tour",
      group: "actions",
      title: t("palette.action.tour"),
      icon: RoadLocation01Icon,
      words: `${template("palette.action.tour", undefined, "en")} tour welcome guide help`,
      target: { kind: "run", run: openTour },
    },
    {
      id: "action-handbook",
      group: "actions",
      title: t("palette.action.handbook"),
      icon: BookOpen01Icon,
      words: `${template("palette.action.handbook", undefined, "en")} handbook help docs manual`,
      target: { kind: "href", href: HANDBOOK_BASE },
    },
    {
      id: "action-sign-out",
      group: "actions",
      title: t("shell.signOut"),
      icon: Logout01Icon,
      words: `${template("shell.signOut", undefined, "en")} log out lock leave`,
      target: { kind: "run", run: signOut },
    },
  );
  return items;
}
