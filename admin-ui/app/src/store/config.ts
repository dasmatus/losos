/* The config store: the port of BoxStore.kt.
 *
 * One BoxConfig - where the box lives, the access token that links to it,
 * whether demo mode is on and which theme mode the owner picked - persisted
 * so the app opens where the owner left it. The Compose app kept it in
 * SharedPreferences; React Native's shape of that is a key-value store.
 *
 * MMKV vs AsyncStorage: MMKV loses. The config is four fields written on a
 * button press and read a handful of times on launch; nothing here wants
 * MMKV's mmap-backed synchronous reads, and its native module has a habit
 * of trailing every React Native major by a few weeks. AsyncStorage is on
 * the Expo-maintained track and stays out of the way - so the token rides
 * AsyncStorage, unencrypted, in the app's own sandbox, which is the same
 * protection level the SharedPreferences port had. A phone-level secure
 * store (keychain-backed) is the next step if a threat review asks for one.
 *
 * The API client (src/api/api.ts) and the panes read this through
 * useBox(); a 401 eviction writes token: "" and the connect screen takes
 * over, the way MainActivity's needsConnect did.
 */

import AsyncStorage from "@react-native-async-storage/async-storage";
import { useSyncExternalStore } from "react";

import type { ThemeMode } from "../mobile/theme/tokens";

/** Where the box lives, and the access token that links us to it
 *  (BoxStore.kt BoxConfig). */
export interface BoxConfig {
  host: string;
  token: string;
  demo: boolean;
  themeMode: ThemeMode;
}

export const DEFAULT_CONFIG: BoxConfig = {
  host: "",
  token: "",
  demo: true,
  themeMode: "system",
};

/** How the box's pages are reached from an address field (BoxConfig.base):
 *  `http://` addresses are taken as typed, everything else is the LAN's
 *  https. The trailing slash goes the way BoxConfig.base did, so the path
 *  appends clean (`https://box.local` + `/api/…`, never `//api/…`). */
export function boxBase(host: string): string {
  const trimmed = host.trim().replace(/\/+$/, "");
  return trimmed.startsWith("http") ? trimmed : `https://${trimmed}`;
}

/* ── The store ──────────────────────────────────────────────────────────── */

const KEY = "losos-companion/config";

let current: BoxConfig | null = null; // null until load() finished
const listeners = new Set<() => void>();

function emit(): void {
  for (const fn of listeners) fn();
}

export async function loadConfig(): Promise<void> {
  let parsed: Partial<BoxConfig> = {};
  try {
    const raw = await AsyncStorage.getItem(KEY);
    if (raw !== null) parsed = JSON.parse(raw) as Partial<BoxConfig>;
  } catch {
    /* a corrupt or blocked store reads as first-run; demo mode is on */
  }
  const themeMode = (["system", "light", "dark"] as const).find(
    (m) => m === parsed.themeMode,
  );
  current = {
    host: typeof parsed.host === "string" ? parsed.host : "",
    token: typeof parsed.token === "string" ? parsed.token : "",
    demo: typeof parsed.demo === "boolean" ? parsed.demo : true,
    themeMode: themeMode ?? "system",
  };
  emit();
}

/** Persist `config` (BoxStore.kt save): the in-memory copy swaps at once;
 *  the write is awaited by nobody and two writes race, which the last one
 *  wins - orders of magnitude out of harm's way for this UI. */
export function saveConfig(config: BoxConfig): void {
  current = config;
  emit();
  void AsyncStorage.setItem(KEY, JSON.stringify(config)).catch(() => {
    /* storage blocked; the session stays in memory, like the SPA's catch */
  });
}

/** MainActivity's update: replace the config and persist it. */
export function updateConfig(patch: Partial<BoxConfig>): void {
  if (current === null) return;
  saveConfig({ ...current, ...patch });
}

/** Subscribe for useSyncExternalStore; getSnapshot mirrors MainActivity. */
export function subscribeConfig(onChange: () => void): () => void {
  listeners.add(onChange);
  return () => {
    listeners.delete(onChange);
  };
}

export { current as getCurrentConfig };

/* ── React views ────────────────────────────────────────────────────────── */

/** The config, or null before loadConfig() has answered (the app shell
 *  renders nothing then, one frame, like a splash). */
export function useBoxConfig(): BoxConfig | null {
  return useSyncExternalStore(subscribeConfig, () => current);
}

/** MainActivity's needsConnect: the connect screen stays until the demo is
 *  on or a box is linked. A link is an address *and* a token; when lososd
 *  throws the token back (a 401, evicted in src/api/api.ts) the token goes
 *  and the owner is asked again. */
export function needsConnect(config: BoxConfig): boolean {
  if (config.demo) return config.host.trim().length === 0;
  return config.host.trim().length === 0 || config.token.trim().length === 0;
}

/* ── Probe (BoxStore.kt BoxProbe) ───────────────────────────────────────── */

/** A probe round trip to the box's admin pages (BoxProbe.probe). */
export type ProbeResult =
  | { kind: "probing" }
  | { kind: "reachable"; ms: number }
  | { kind: "unreachable"; reason: string };

/** Demo data stand-ins while no real box is linked (BoxProbe.demoStorage):
 *  every invented number a card shows wears the demo chip while it is up. */
export const DEMO_STORAGE = {
  usedGb: 184,
  totalGb: 512,
  label: "1 TB, encrypted /persist",
  get fraction(): number {
    return this.usedGb / this.totalGb;
  },
  get phrase(): string {
    return `${this.usedGb} GB used of ${this.totalGb} GB`;
  },
} as const;
