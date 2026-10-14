/* Everything this client keeps lives in the desktop's Secret Service, reached
 * through libsecret's `secret-tool`. Nothing is written to a plain file, and
 * when no Secret Service answers nothing is kept at all.
 *
 * Items carry `service=losos` and a `kind` (docs/terminal-access.md):
 *
 *   kind=gpg-key  fingerprint=<40 hex>  secret: the armoured secret key
 *   kind=host     url=<base url>        secret: JSON, see HostRecord
 *
 * `Keychain` is the narrow interface the rest of the client uses; tests swap
 * in MemoryKeychain, and the suite also runs SecretToolKeychain against a
 * fake `secret-tool` on PATH so this file's own code path is exercised. */

import { spawn } from "node:child_process";
import { t } from "./lang";

export type Attributes = Record<string, string>;

export interface KeychainItem {
  label: string;
  secret: string;
}

export interface Keychain {
  store(attributes: Attributes, label: string, secret: string): Promise<void>;
  /** The secret of the first item matching every attribute, or null. */
  lookup(attributes: Attributes): Promise<string | null>;
  /** Every item matching every attribute. */
  search(attributes: Attributes): Promise<KeychainItem[]>;
  clear(attributes: Attributes): Promise<void>;
}

/** No Secret Service, or no `secret-tool`: the client keeps nothing. */
export class KeychainUnavailable extends Error {
  constructor(detail: string) {
    super(detail);
    this.name = "KeychainUnavailable";
  }
}

interface Run {
  code: number;
  stdout: string;
  stderr: string;
}

function flatten(attributes: Attributes): string[] {
  return Object.entries(attributes).flatMap(([name, value]) => [name, value]);
}

export class SecretToolKeychain implements Keychain {
  constructor(private readonly command = "secret-tool") {}

  private run(args: string[], input?: string): Promise<Run> {
    return new Promise((resolve, reject) => {
      let child;
      try {
        child = spawn(this.command, args, { stdio: ["pipe", "pipe", "pipe"] });
      } catch (error) {
        reject(this.missing(error));
        return;
      }
      const out: Buffer[] = [];
      const err: Buffer[] = [];
      child.stdout.on("data", (chunk: Buffer) => out.push(chunk));
      child.stderr.on("data", (chunk: Buffer) => err.push(chunk));
      child.on("error", (error) => reject(this.missing(error)));
      child.on("close", (code) =>
        resolve({
          code: code ?? 1,
          stdout: Buffer.concat(out).toString("utf8"),
          stderr: Buffer.concat(err).toString("utf8"),
        }),
      );
      child.stdin.on("error", () => {
        /* the process went away before reading; its exit status says why */
      });
      child.stdin.end(input ?? "");
    });
  }

  private missing(error: unknown): Error {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") {
      return new KeychainUnavailable(t("tui.keychain.noTool"));
    }
    return error instanceof Error ? error : new Error(String(error));
  }

  /* secret-tool answers "nothing found" with status 1 and nothing on stderr,
   * and a missing or locked-out Secret Service with status 1 and a message
   * on stderr. The message is what tells the two apart. */
  private fail(run: Run): never {
    const detail = run.stderr.trim();
    throw new KeychainUnavailable(
      detail.length > 0 ? t("tui.keychain.noService", { detail }) : t("tui.keychain.failed"),
    );
  }

  async store(attributes: Attributes, label: string, secret: string): Promise<void> {
    const run = await this.run(["store", `--label=${label}`, ...flatten(attributes)], secret);
    if (run.code !== 0) this.fail(run);
  }

  async lookup(attributes: Attributes): Promise<string | null> {
    const run = await this.run(["lookup", ...flatten(attributes)]);
    if (run.code === 0) return run.stdout;
    if (run.stderr.trim().length === 0) return null;
    this.fail(run);
  }

  async search(attributes: Attributes): Promise<KeychainItem[]> {
    const run = await this.run(["search", "--all", "--unlock", ...flatten(attributes)]);
    if (run.code !== 0) {
      if (run.stderr.trim().length === 0 || /no matching|not found/i.test(run.stderr)) return [];
      this.fail(run);
    }
    return parseSearch(run.stdout);
  }

  async clear(attributes: Attributes): Promise<void> {
    const run = await this.run(["clear", ...flatten(attributes)]);
    if (run.code !== 0 && run.stderr.trim().length > 0) this.fail(run);
  }

  /** Ask the Secret Service something harmless, so a missing one is reported
   *  before the user has typed a token. */
  async probe(): Promise<void> {
    await this.search({ service: "losos", kind: "probe" });
  }
}

/* `secret-tool search` prints one block per item:
 *
 *   [/org/freedesktop/secrets/collection/login/12]
 *   label = losos host https://box.local
 *   secret = <the secret, which may span lines>
 *   created = 2026-10-14 09:00:00
 *   modified = 2026-10-14 09:00:00
 *
 * followed by `schema = ` and `attribute.<name> = ` lines, which libsecret
 * writes to stderr. An armoured key spans many lines, so a secret runs until
 * the `created = ` line that follows it. */
export function parseSearch(stdout: string): KeychainItem[] {
  const items: KeychainItem[] = [];
  const lines = stdout.split("\n");
  let label = "";
  let secret: string[] | null = null;
  let inItem = false;
  const flush = (): void => {
    if (inItem && secret !== null) items.push({ label, secret: secret.join("\n") });
    label = "";
    secret = null;
  };
  for (const line of lines) {
    if (/^\[\/.*\]$/.test(line)) {
      flush();
      inItem = true;
      continue;
    }
    if (!inItem) continue;
    if (secret !== null && !/^(created|modified|schema|attribute\.[^ ]+) = /.test(line)) {
      secret.push(line);
      continue;
    }
    if (line.startsWith("label = ")) label = line.slice("label = ".length);
    else if (line.startsWith("secret = ")) secret = [line.slice("secret = ".length)];
    else if (secret !== null) {
      /* created/modified/attributes: the secret has ended */
      items.push({ label, secret: secret.join("\n") });
      secret = null;
      inItem = false;
    }
  }
  flush();
  return items;
}

/** In memory, for tests and for the pane renderer's fixtures. */
export class MemoryKeychain implements Keychain {
  readonly items: { attributes: Attributes; label: string; secret: string }[] = [];

  private matches(item: { attributes: Attributes }, attributes: Attributes): boolean {
    return Object.entries(attributes).every(([k, v]) => item.attributes[k] === v);
  }

  async store(attributes: Attributes, label: string, secret: string): Promise<void> {
    await this.clear(attributes);
    this.items.push({ attributes: { ...attributes }, label, secret });
  }

  async lookup(attributes: Attributes): Promise<string | null> {
    return this.items.find((item) => this.matches(item, attributes))?.secret ?? null;
  }

  async search(attributes: Attributes): Promise<KeychainItem[]> {
    return this.items
      .filter((item) => this.matches(item, attributes))
      .map(({ label, secret }) => ({ label, secret }));
  }

  async clear(attributes: Attributes): Promise<void> {
    for (let i = this.items.length - 1; i >= 0; i -= 1) {
      const item = this.items[i];
      if (item !== undefined && this.matches(item, attributes)) this.items.splice(i, 1);
    }
  }
}
