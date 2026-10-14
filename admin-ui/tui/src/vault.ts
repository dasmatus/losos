/* The two kinds of item this client keeps, over the Keychain interface.
 * docs/terminal-access.md, "What the client keeps, and where". */

import type { Keychain } from "./keychain";

export type HostKind = "box" | "edge";

/** The secret of a `kind=host` item. */
export interface HostRecord {
  url: string;
  kind: HostKind;
  /** The host key's fingerprint, 40 hex, upper case. Pinned on first login. */
  fingerprint: string;
  /** The host key's armoured public half, as served at login. */
  publicKey: string;
  token: string;
  /** Which of the user's keys (`kind=gpg-key`) signs for this host. */
  keyFingerprint: string;
}

const SERVICE = "losos";

/** One spelling per host: scheme and host lower case, no trailing slash. */
export function normalizeUrl(raw: string): string {
  const trimmed = raw.trim();
  const withScheme = /^[a-z][a-z0-9+.-]*:\/\//i.test(trimmed) ? trimmed : `https://${trimmed}`;
  const url = new URL(withScheme);
  if (url.protocol !== "https:" && url.protocol !== "http:") {
    throw new Error(`${url.protocol} is not http or https`);
  }
  if (url.username !== "" || url.password !== "") throw new Error("a URL with a user name or password");
  const path = url.pathname.replace(/\/+$/, "");
  return `${url.protocol}//${url.host}${path}`;
}

function isHostRecord(value: unknown): value is HostRecord {
  if (typeof value !== "object" || value === null) return false;
  const v = value as Record<string, unknown>;
  return (
    typeof v["url"] === "string" &&
    (v["kind"] === "box" || v["kind"] === "edge") &&
    typeof v["fingerprint"] === "string" &&
    typeof v["publicKey"] === "string" &&
    typeof v["token"] === "string" &&
    typeof v["keyFingerprint"] === "string"
  );
}

export class Vault {
  constructor(readonly keychain: Keychain) {}

  async putKey(fingerprint: string, armored: string, label: string): Promise<void> {
    await this.keychain.store({ service: SERVICE, kind: "gpg-key", fingerprint }, label, armored);
  }

  async getKey(fingerprint: string): Promise<string | null> {
    return this.keychain.lookup({ service: SERVICE, kind: "gpg-key", fingerprint });
  }

  /** Every stored key, armoured. */
  async listKeys(): Promise<string[]> {
    const items = await this.keychain.search({ service: SERVICE, kind: "gpg-key" });
    return items.map((item) => item.secret).filter((secret) => secret.includes("BEGIN PGP PRIVATE KEY BLOCK"));
  }

  async removeKey(fingerprint: string): Promise<void> {
    await this.keychain.clear({ service: SERVICE, kind: "gpg-key", fingerprint });
  }

  async putHost(record: HostRecord): Promise<void> {
    await this.keychain.store(
      { service: SERVICE, kind: "host", url: record.url },
      `losos ${record.kind} ${record.url}`,
      JSON.stringify(record),
    );
  }

  async getHost(url: string): Promise<HostRecord | null> {
    const secret = await this.keychain.lookup({ service: SERVICE, kind: "host", url });
    if (secret === null) return null;
    try {
      const parsed: unknown = JSON.parse(secret);
      return isHostRecord(parsed) ? parsed : null;
    } catch {
      return null;
    }
  }

  async listHosts(): Promise<HostRecord[]> {
    const items = await this.keychain.search({ service: SERVICE, kind: "host" });
    const hosts: HostRecord[] = [];
    for (const item of items) {
      try {
        const parsed: unknown = JSON.parse(item.secret);
        if (isHostRecord(parsed)) hosts.push(parsed);
      } catch {
        /* not ours to read; skip it */
      }
    }
    return hosts.sort((a, b) => a.url.localeCompare(b.url));
  }

  async removeHost(url: string): Promise<void> {
    await this.keychain.clear({ service: SERVICE, kind: "host", url });
  }
}
