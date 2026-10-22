/* Pictures that arrive in Chat and Mail: whether one shows at once or
 * waits blurred until the person presses it.
 *
 * The person's settings and each sender's answer come from lososd
 * (`/api/pictures`, backend/src/pictures.rs) and are kept for the page's
 * life; a change here drops them so every picture on screen asks again.
 * When lososd does not answer (an older box, the spare key) every picture
 * stays blurred: the safe way to be wrong. */

import * as React from "react";
import {
  getPictureSender,
  getPictures,
  postPictureTrust,
  postPictures,
  type PictureBlur,
  type PictureSender,
  type PictureSettings,
} from "./api";

const FALLBACK: PictureSettings = {
  blur: "everyone",
  trusted: [],
  nudityCheck: false,
  age: null,
  child: false,
  mayShowAll: false,
};

let settings: PictureSettings | null = null;
let loading: Promise<void> | null = null;
const senders = new Map<string, PictureSender | "asking">();
/** Pictures shown this page load, by the caller's key (an event ID). */
const revealed = new Set<string>();
let version = 0;
const listeners = new Set<() => void>();

function changed(): void {
  version += 1;
  for (const l of listeners) l();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

const snapshot = (): number => version;

function load(): void {
  if (settings !== null || loading !== null) return;
  loading = getPictures()
    .then((s) => {
      settings = s;
    })
    .catch(() => {
      settings = FALLBACK;
    })
    .finally(() => {
      loading = null;
      changed();
    });
}

function forgetAnswers(next: PictureSettings): void {
  settings = next;
  senders.clear();
  changed();
}

/** The person's settings, null until they arrive. */
export function usePictureSettings(): PictureSettings | null {
  React.useSyncExternalStore(subscribe, snapshot, snapshot);
  React.useEffect(load, []);
  return settings;
}

function ask(id: string): void {
  if (senders.has(id)) return;
  senders.set(id, "asking");
  getPictureSender(id)
    .then((answer) => senders.set(id, answer))
    .catch(() => senders.set(id, { shown: false, because: null, nudityCheck: settings?.nudityCheck ?? false }))
    .finally(changed);
}

/** Whether `sender`'s pictures show at once: null while lososd is asked. */
export function useSenderShown(sender: string | null): PictureSender | null {
  const seen = React.useSyncExternalStore(subscribe, snapshot, snapshot);
  const id = sender?.trim().toLowerCase() ?? "";
  // `seen` moves when a change drops the answers, so this asks again.
  React.useEffect(() => {
    if (id !== "") ask(id);
  }, [id, seen]);
  if (id === "") return { shown: false, because: null, nudityCheck: settings?.nudityCheck ?? false };
  const answer = senders.get(id);
  return answer === undefined || answer === "asking" ? null : answer;
}

/** Whether the person pressed this picture to see it, and the press. */
export function useRevealed(key: string): [boolean, () => void] {
  React.useSyncExternalStore(subscribe, snapshot, snapshot);
  const reveal = React.useCallback(() => {
    revealed.add(key);
    changed();
  }, [key]);
  return [revealed.has(key), reveal];
}

/** Always show `sender`'s pictures at once, or stop. */
export async function trustSender(sender: string, trusted = true): Promise<void> {
  forgetAnswers(await postPictureTrust(sender, trusted));
}

/** Change the person's own settings. Throws the ApiError lososd answers,
 *  whose `code` is `adultRequired` or `childAccount`. */
export async function savePictureSettings(change: { blur?: PictureBlur; nudityCheck?: boolean }): Promise<PictureSettings> {
  const next = await postPictures(change);
  forgetAnswers(next);
  return next;
}
