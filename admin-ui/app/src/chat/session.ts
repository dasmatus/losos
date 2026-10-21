/* Signing this page in to chat, and out again.
 *
 * Nobody types a Matrix password. The person signed in with LosOS Login, and
 * POST /api/chat/session hands this page a device on the box's homeserver
 * for that person (backend/src/chat.rs): lososd is the homeserver's
 * application service and signs the device in for them. Every request the
 * page then makes to /_matrix/ is asked about at nginx again: the LosOS
 * Login cookie must still be valid and name the same person as the token, so
 * signing out of LosOS Login, or being removed from the box, ends chat too.
 *
 * The device ID is kept in localStorage per person, so a reload signs the
 * same device back in and the end-to-end keys in IndexedDB stay its keys. A
 * device the homeserver no longer has keys for (signed out everywhere,
 * removed and added again) starts over as a new device. */

import {
  ClientEvent,
  createClient,
  HttpApiEvent,
  type MatrixClient,
  SyncState,
} from "matrix-js-sdk";
import { decodeRecoveryKey } from "matrix-js-sdk/lib/crypto-api";
import { signInUrl, signOutTo } from "@/lib/api";
import { boxNames } from "./model";

export interface ChatView {
  available: boolean;
  reason: "up" | "starting" | "off";
  serverName?: string;
  federation?: boolean;
}

interface SessionReply {
  userId: string;
  accessToken: string;
  deviceId: string;
  homeserver: string;
}

interface MeReply {
  name: string | null;
  displayName: string | null;
}

/** Why the page cannot open chat. */
export type Refusal = "signIn" | "off" | "starting" | "notForYou" | "broken";

export class ChatRefused extends Error {
  readonly why: Refusal;
  constructor(why: Refusal, message: string = why) {
    super(message);
    this.why = why;
  }
}

async function getJson<T>(path: string, init: RequestInit = {}): Promise<T> {
  const response = await fetch(path, { cache: "no-store", credentials: "same-origin", ...init });
  if (response.status === 401) throw new ChatRefused("signIn");
  if (response.status === 404) throw new ChatRefused("off");
  if (response.status === 403) throw new ChatRefused("notForYou", await errorOf(response));
  if (response.status === 503) throw new ChatRefused("starting");
  if (!response.ok) throw new ChatRefused("broken", await errorOf(response));
  return (await response.json()) as T;
}

async function errorOf(response: Response): Promise<string> {
  try {
    const body = (await response.json()) as { error?: unknown };
    if (typeof body.error === "string") return body.error;
  } catch {
    /* not JSON */
  }
  return `HTTP ${response.status}`;
}

const deviceKey = (name: string): string => `losos-chat-device:${name}`;

function storedDevice(name: string): string | null {
  try {
    return localStorage.getItem(deviceKey(name));
  } catch {
    return null;
  }
}

function storeDevice(name: string, device: string | null): void {
  try {
    if (device === null) localStorage.removeItem(deviceKey(name));
    else localStorage.setItem(deviceKey(name), device);
  } catch {
    /* a private window: the next load is a new device */
  }
}

/** The IndexedDB prefix of one device's end-to-end keys. */
const cryptoPrefix = (userId: string, deviceId: string): string => `losos-chat:${userId}:${deviceId}`;

async function dropCryptoStore(prefix: string): Promise<void> {
  if (typeof indexedDB.databases !== "function") return;
  const all = await indexedDB.databases();
  await Promise.all(
    all
      .map((db) => db.name ?? "")
      .filter((name) => name.startsWith(prefix))
      .map(
        (name) =>
          new Promise<void>((resolve) => {
            const ask = indexedDB.deleteDatabase(name);
            ask.onsuccess = ask.onerror = ask.onblocked = () => resolve();
          }),
      ),
  );
}

/** Whether the homeserver still holds this device's identity keys. */
async function deviceHasKeys(session: SessionReply): Promise<boolean> {
  const response = await fetch("/_matrix/client/v3/keys/query", {
    method: "POST",
    headers: { Authorization: `Bearer ${session.accessToken}`, "Content-Type": "application/json" },
    body: JSON.stringify({ device_keys: { [session.userId]: [session.deviceId] } }),
  });
  if (!response.ok) return false;
  const body = (await response.json()) as { device_keys?: Record<string, Record<string, unknown>> };
  return body.device_keys?.[session.userId]?.[session.deviceId] !== undefined;
}

function deviceName(): string {
  const agent = navigator.userAgent;
  const browser = /Firefox\//.test(agent)
    ? "Firefox"
    : /Edg\//.test(agent)
      ? "Edge"
      : /Chrome\//.test(agent)
        ? "Chrome"
        : /Safari\//.test(agent)
          ? "Safari"
          : "Browser";
  return `LosOS chat, ${browser}`;
}

async function askSession(device: string | null): Promise<SessionReply> {
  return getJson<SessionReply>("/api/chat/session", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ ...(device !== null ? { device } : {}), deviceName: deviceName() }),
  });
}

export interface Signed {
  client: MatrixClient;
  view: ChatView;
  name: string;
  displayName: string;
  /** The secret storage key the person typed or was given, held for the
   *  session only, so the crypto can read secrets without asking twice. */
  keys: SecretKeys;
}

/** The recovery key, once the person has given it on this page load. */
export class SecretKeys {
  private cached = new Map<string, Uint8Array<ArrayBuffer>>();
  private asker: ((keyIds: string[]) => Promise<[string, Uint8Array<ArrayBuffer>] | null>) | null = null;

  set(keyId: string, key: Uint8Array<ArrayBuffer>): void {
    this.cached.set(keyId, key);
  }

  /** The page's prompt for a recovery key, installed by the security dialog. */
  onAsk(asker: ((keyIds: string[]) => Promise<[string, Uint8Array<ArrayBuffer>] | null>) | null): void {
    this.asker = asker;
  }

  async get(keyIds: string[]): Promise<[string, Uint8Array<ArrayBuffer>] | null> {
    for (const id of keyIds) {
      const key = this.cached.get(id);
      if (key !== undefined) return [id, key];
    }
    if (this.asker === null) return null;
    const answer = await this.asker(keyIds);
    if (answer !== null) this.cached.set(answer[0], answer[1]);
    return answer;
  }
}

/** A typed recovery key, as the secret storage key it encodes. */
export function recoveryKeyBytes(text: string): Uint8Array<ArrayBuffer> | null {
  try {
    return decodeRecoveryKey(text.trim());
  } catch {
    return null;
  }
}

/** Sign this page in: who LosOS Login says this is, a device for them, and
 *  a running client with end-to-end encryption. */
export async function signIn(): Promise<Signed> {
  const me = await getJson<MeReply>("/api/me");
  if (me.name === null) throw new ChatRefused("notForYou", "spare key");
  const view = await getJson<ChatView>("/api/chat");
  if (!view.available) throw new ChatRefused(view.reason === "off" ? "off" : "starting");

  const had = storedDevice(me.name);
  let session = await askSession(had);
  if (had !== null && session.deviceId === had && !(await deviceHasKeys(session))) {
    // The homeserver forgot this device's keys; so does the page.
    await dropCryptoStore(cryptoPrefix(session.userId, had));
    session = await askSession(null);
  }
  storeDevice(me.name, session.deviceId);

  const keys = new SecretKeys();
  const client = createClient({
    baseUrl: window.location.origin,
    accessToken: session.accessToken,
    userId: session.userId,
    deviceId: session.deviceId,
    timelineSupport: true,
    useAuthorizationHeader: true,
    // nginx asks LosOS Login about every homeserver request, so the sign-in
    // cookie has to travel with them; the SDK's own fetch leaves it out.
    fetchFn: (input, init) => fetch(input, { ...init, credentials: "same-origin" }),
    cryptoCallbacks: {
      getSecretStorageKey: ({ keys: wanted }) => keys.get(Object.keys(wanted)),
      cacheSecretStorageKey: (keyId, _info, key) => keys.set(keyId, key),
    },
  });
  // Everyone's name on this box, for people the homeserver names only by ID.
  await boxPeople()
    .then((people) => {
      for (const p of people) boxNames.set(p.userId, p.displayName);
    })
    .catch(() => undefined);
  await client.initRustCrypto({
    useIndexedDB: true,
    cryptoDatabasePrefix: cryptoPrefix(session.userId, session.deviceId),
  });
  return { client, view, name: me.name, displayName: me.displayName ?? me.name, keys };
}

/** Start syncing; resolves once the first sync has landed. */
export function start(client: MatrixClient): Promise<void> {
  return new Promise((resolve, reject) => {
    const onSync = (state: SyncState): void => {
      if (state === SyncState.Prepared || state === SyncState.Syncing) {
        client.off(ClientEvent.Sync, onSync);
        resolve();
      } else if (state === SyncState.Error && client.getSyncState() === SyncState.Error) {
        // Keeps retrying by itself; the page shows the reconnecting state.
      }
    };
    client.on(ClientEvent.Sync, onSync);
    client.startClient({ initialSyncLimit: 30, lazyLoadMembers: true }).catch(reject);
  });
}

/** The homeserver refused the token: a sign-out in LosOS Login, a removed
 *  person, or a device signed out from elsewhere. */
export function onSignedOut(client: MatrixClient, then: () => void): void {
  client.on(HttpApiEvent.SessionLoggedOut, () => {
    client.stopClient();
    then();
  });
}

/** Leave chat and LosOS Login together: the device goes, then its keys on
 *  this browser, then the LosOS Login session. */
/** The chat this page is signed in to, for the top bar's sign-out. */
let current: Signed | null = null;

export function signedHere(signed: Signed | null): void {
  current = signed;
}

/** The top bar's sign-out on the chat page: the device first, when there is
 *  one, then LosOS Login. */
export function signOutHere(): void {
  if (current !== null) void signOut(current);
  else signOutTo("/chat/");
}

export async function signOut(signed: Signed): Promise<void> {
  const { client, name } = signed;
  const userId = client.getSafeUserId();
  const deviceId = client.getDeviceId();
  try {
    await client.logout(true);
  } catch {
    /* already gone */
  }
  if (deviceId !== null) await dropCryptoStore(cryptoPrefix(userId, deviceId));
  storeDevice(name, null);
  signOutTo("/chat/");
}

export interface BoxPerson {
  userId: string;
  displayName: string;
}

/** Everyone on this box chat can reach (GET /api/chat/people). The
 *  homeserver's own directory lists only people who already share a room. */
export async function boxPeople(): Promise<BoxPerson[]> {
  const answer = await getJson<{ people: BoxPerson[] }>("/api/chat/people");
  return answer.people;
}

/** LosOS Login's page, coming back to chat. */
export function goSignIn(): void {
  window.location.assign(signInUrl());
}
