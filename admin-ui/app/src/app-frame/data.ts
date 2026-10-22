/* What an app keeps, inside the frame: the two data hooks and `losos`.
 *
 * The frame is an opaque origin with no token and no API, so it holds a copy
 * of the app's data that the admin page sent with the app, changes that copy
 * at once when the app writes, and asks the page to make the same write on
 * the box (`apps/app-frame.tsx`, which calls /api/user-apps/{id}/data). A
 * write the box refuses is reported, and the page sends the box's copy back,
 * so the frame never keeps something the box does not.
 *
 * The hooks forgive the shapes a small model reaches for. `useCollection`
 * answers `notes.items.map`, `notes.map` and `notes.length` alike, and when
 * the app writes `const [cards, setCards] = useCollection("cards")`, as if it
 * were useState, `cards` is the list (with add, update and remove on it) and
 * `setCards` replaces the whole list.
 */

import * as React from "react";

export type Item = Record<string, unknown> & { id: string };
export interface Snapshot {
  collections: Record<string, Item[]>;
  stored: Record<string, unknown>;
}

export type Op =
  | { op: "add"; collection: string; value: Record<string, unknown> }
  | { op: "update"; collection: string; id: string; value: Record<string, unknown> }
  | { op: "remove"; collection: string; id: string }
  | { op: "replace"; collection: string; items: Record<string, unknown>[] }
  | { op: "set"; key: string; value: unknown };

/** Sends one write to the page, which answers with the box's result. */
export type Writer = (op: Op) => Promise<Record<string, unknown>>;

const KEY = /^[A-Za-z0-9_-]{1,40}$/;

let data: Snapshot = { collections: {}, stored: {} };
let version = 0;
const listeners = new Set<() => void>();
let writer: Writer = () => Promise.reject(new Error("the app is not loaded"));
let tempSeq = 0;

function changed(): void {
  version += 1;
  for (const listener of listeners) listener();
}

/** The page's copy of the data: on load, and after a refused write. */
export function setSnapshot(next: Snapshot): void {
  data = {
    collections: typeof next.collections === "object" && next.collections !== null ? next.collections : {},
    stored: typeof next.stored === "object" && next.stored !== null ? next.stored : {},
  };
  changed();
}

/** How many entries each collection holds, and which stored keys are set:
 *  what a try of the app compares before and after. */
export function tally(): Record<string, number> {
  const out: Record<string, number> = {};
  for (const [name, items] of Object.entries(data.collections)) out[name] = Array.isArray(items) ? items.length : 0;
  for (const key of Object.keys(data.stored)) out[`stored ${key}`] = 1;
  return out;
}

export function setWriter(w: Writer): void {
  writer = w;
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function useVersion(): number {
  return React.useSyncExternalStore(subscribe, () => version);
}

function plain(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return { value };
  // Through JSON, so a Date becomes its ISO string the way it will be kept.
  return JSON.parse(JSON.stringify(value)) as Record<string, unknown>;
}

function items(name: string): Item[] {
  return data.collections[name] ?? [];
}

function put(name: string, list: Item[]): void {
  data = { ...data, collections: { ...data.collections, [name]: list } };
  changed();
}

function check(name: string): void {
  if (!KEY.test(name)) throw new Error(`"${name}" cannot name a collection: use letters, digits, - and _`);
}

function collectionApi(name: string) {
  const add = (value: unknown): Promise<Item> => {
    check(name);
    tempSeq += 1;
    const temp = `new-${tempSeq}`;
    const body = plain(value);
    put(name, [...items(name), { ...body, id: temp, createdAt: Date.now() }]);
    return writer({ op: "add", collection: name, value: body }).then((answer) => {
      const item = answer["item"] as Item;
      put(
        name,
        items(name).map((i) => (i.id === temp ? item : i)),
      );
      return item;
    });
  };
  const update = (id: unknown, value: unknown): Promise<Item> => {
    const key = typeof id === "object" && id !== null && "id" in id ? String((id as Item).id) : String(id);
    const body = plain(value);
    delete body["id"];
    put(
      name,
      items(name).map((i) => (i.id === key ? { ...i, ...body } : i)),
    );
    return writer({ op: "update", collection: name, id: key, value: body }).then((answer) => answer["item"] as Item);
  };
  const remove = (id: unknown): Promise<void> => {
    const key = typeof id === "object" && id !== null && "id" in id ? String((id as Item).id) : String(id);
    put(
      name,
      items(name).filter((i) => i.id !== key),
    );
    return writer({ op: "remove", collection: name, id: key }).then(() => undefined);
  };
  const set = (next: unknown): Promise<void> => {
    check(name);
    const list = typeof next === "function" ? (next as (old: Item[]) => unknown)(items(name)) : next;
    const array = Array.isArray(list) ? list.map(plain) : [];
    put(
      name,
      array.map((v, i) => ({ ...v, id: typeof v["id"] === "string" ? v["id"] : `new-list-${i}` }) as Item),
    );
    return writer({ op: "replace", collection: name, items: array }).then((answer) => {
      put(name, answer["items"] as Item[]);
    });
  };
  return { add, update, remove, set };
}

/** A list of objects kept on the box, by name. */
export function useCollection(name: string) {
  const v = useVersion();
  const list = items(name);
  // `v` is in the deps so a change to the list makes a new object.
  return React.useMemo(() => {
    const api = collectionApi(name);
    const array = Object.assign(list.slice(), api, { items: list, loading: false });
    const result = {
      ...api,
      items: list,
      loading: false,
      error: null,
      length: list.length,
      map: list.map.bind(list),
      filter: list.filter.bind(list),
      find: list.find.bind(list),
      forEach: list.forEach.bind(list),
      reduce: list.reduce.bind(list),
      some: list.some.bind(list),
      every: list.every.bind(list),
      slice: list.slice.bind(list),
      *[Symbol.iterator]() {
        yield array;
        yield api.set;
      },
    };
    return result;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [name, v]);
}

/** Like useState, but the value is kept on the box. */
export function useStored<T>(key: string, initial: T): [T, (next: T | ((old: T) => T)) => void] {
  useVersion();
  const value = Object.prototype.hasOwnProperty.call(data.stored, key) ? (data.stored[key] as T) : initial;
  const setValue = React.useCallback(
    (next: T | ((old: T) => T)) => {
      if (!KEY.test(key)) throw new Error(`"${key}" cannot name a value: use letters, digits, - and _`);
      const old = Object.prototype.hasOwnProperty.call(data.stored, key) ? (data.stored[key] as T) : initial;
      const resolved = typeof next === "function" ? (next as (old: T) => T)(old) : next;
      const kept = resolved === undefined ? null : JSON.parse(JSON.stringify(resolved));
      data = { ...data, stored: { ...data.stored, [key]: kept } };
      changed();
      void writer({ op: "set", key, value: kept }).catch(() => undefined);
    },
    // `initial` is read only when nothing is kept yet, like useState's.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [key],
  );
  return [value, setValue];
}
