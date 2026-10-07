import { isSettingsPaneId, type SettingsPaneId } from "@/screens/settings/panes";

/* The one place a settings pane's address is decided.
 *
 * Storage, mesh and apps are what an owner opens this page for, so they get a
 * top-level address; network, hardware, security, about and reset sit under
 * /settings. The sidebar and the router both read this, so they cannot
 * disagree about where a pane lives, and a deep link to any of them survives a
 * reload because nginx serves index.html for unknown paths under the admin
 * location. */
const TOP_LEVEL_PANES: readonly SettingsPaneId[] = ["storage", "mesh", "apps"];

/* What /settings opens: the first entry under Settings in the sidebar. A
 * segment that names no pane that can be opened (a typo, or the planned
 * market) lands here too, never on a blank page. */
export const SETTINGS_LANDING: SettingsPaneId = "network";

export function paneHref(pane: SettingsPaneId): string {
  return TOP_LEVEL_PANES.includes(pane) ? `/${pane}` : `/settings/${pane}`;
}

/** The pane a path shows, or null for a path that is not a settings pane. */
export function paneFromPath(pathname: string): SettingsPaneId | null {
  const top = TOP_LEVEL_PANES.find((pane) => pathname === `/${pane}`);
  if (top !== undefined) return top;
  if (pathname === "/settings" || pathname === "/settings/") return SETTINGS_LANDING;
  const match = /^\/settings\/([^/]+)\/?$/.exec(pathname);
  if (match === null) return null;
  const segment = match[1];
  return isSettingsPaneId(segment) ? segment : SETTINGS_LANDING;
}
