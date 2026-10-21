import * as React from "react";
import { kb, type Site } from "./api";

/* The person's knowledge bases and whether the box offers them, loaded
 * once and kept for the menus that ask synchronously (a folder's right
 * click). Any change made here loads them again. */

interface State {
  available: boolean;
  sites: Site[];
  loaded: boolean;
}

let state: State = { available: false, sites: [], loaded: false };
const listeners = new Set<() => void>();
let loading: Promise<void> | null = null;

function set(next: State): void {
  state = next;
  for (const l of listeners) l();
}

export function reloadSites(): Promise<void> {
  loading = kb.list().then(
    (r) => set({ available: r.available, sites: r.sites, loaded: true }),
    () => set({ ...state, loaded: true }),
  );
  return loading;
}

export function sites(): State {
  if (loading === null) void reloadSites();
  return state;
}

export function siteFor(folderId: number): Site | undefined {
  return state.sites.find((s) => s.folderId === folderId);
}

export function useSites(): State {
  const [, force] = React.useReducer((n: number) => n + 1, 0);
  React.useEffect(() => {
    listeners.add(force);
    if (loading === null) void reloadSites();
    return () => {
      listeners.delete(force);
    };
  }, []);
  return state;
}
