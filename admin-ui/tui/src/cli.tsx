/* The commands. `losos --help` lists them; docs/terminal-access.md and
 * docs/edge-terminal.md are the contract behind each one. */

import { spawn, spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import * as React from "react";
import { render } from "ink";
import * as openpgp from "openpgp";
import { envelopeTransport } from "./api";
import { call, errorText, EnvelopeError, hostKeyUrl, type Session } from "./envelope";
import { KeychainUnavailable, SecretToolKeychain, type Keychain } from "./keychain";
import { detectLocale, setLocale, t, type Locale } from "./lang";
import {
  fingerprintOf,
  groupFingerprint,
  KeyProblem,
  needsPassphrase,
  readHostKey,
  readUserKey,
  sameFingerprint,
  unlock,
  userIdsOf,
  type PrivateKey,
} from "./pgp";
import { Cancelled, readHidden, readSecret } from "./prompt";
import { detectMode, detectTruecolor, makeTheme, type Theme } from "./theme";
import { BoxApp, EdgeApp } from "./tui/app";
import { VERSION } from "./tui/box-panes";
import { Login, Picker, type LoginAnswer } from "./tui/login";
import { normalizeUrl, Vault, type HostKind, type HostRecord } from "./vault";

// ── Arguments ────────────────────────────────────────────────────────────

export interface Parsed {
  positionals: string[];
  flags: Map<string, string | true>;
}

const VALUE_FLAGS = new Set(["lang", "theme", "key", "fingerprint", "gpg", "host", "repo", "edge", "token"]);

export function parseArgs(argv: readonly string[]): Parsed {
  const positionals: string[] = [];
  const flags = new Map<string, string | true>();
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i]!;
    if (arg === "--") {
      positionals.push(...argv.slice(i + 1));
      break;
    }
    if (arg.startsWith("--")) {
      const eq = arg.indexOf("=");
      const name = eq < 0 ? arg.slice(2) : arg.slice(2, eq);
      if (eq >= 0) flags.set(name, arg.slice(eq + 1));
      else if (VALUE_FLAGS.has(name) && i + 1 < argv.length) flags.set(name, argv[++i]!);
      else flags.set(name, true);
    } else if (arg === "-h") flags.set("help", true);
    else if (arg === "-V" || arg === "-v") flags.set("version", true);
    else positionals.push(arg);
  }
  return { positionals, flags };
}

function flag(parsed: Parsed, name: string): string | undefined {
  const value = parsed.flags.get(name);
  return typeof value === "string" ? value : undefined;
}

// ── Output ───────────────────────────────────────────────────────────────

/** A failure the user can act on: printed as one sentence, exit status 1. */
export class UserError extends Error {
  constructor(
    message: string,
    readonly code = 1,
  ) {
    super(message);
    this.name = "UserError";
  }
}

export interface Io {
  out: (text: string) => void;
  err: (text: string) => void;
  env: NodeJS.ProcessEnv;
  keychain: () => Keychain;
  fetch: typeof fetch;
}

export const defaultIo: Io = {
  out: (text) => process.stdout.write(text),
  err: (text) => process.stderr.write(text),
  env: process.env,
  keychain: () => new SecretToolKeychain(process.env["LOSOS_SECRET_TOOL"] ?? "secret-tool"),
  fetch: (...args) => fetch(...args),
};

function line(io: Io, text = ""): void {
  io.out(`${text}\n`);
}

// ── Help ─────────────────────────────────────────────────────────────────

export function helpText(): string {
  const rows: [string, string][] = [
    ["losos [open] [url]", t("tui.help.cmd.open")],
    ["losos login <url>", t("tui.help.cmd.login")],
    ["losos logout <url>", t("tui.help.cmd.logout")],
    ["losos hosts", t("tui.help.cmd.hosts")],
    ["losos key import <file.asc>", t("tui.help.cmd.keyImport")],
    ["losos key import --gpg <id>", t("tui.help.cmd.keyImportGpg")],
    ["losos key list", t("tui.help.cmd.keyList")],
    ["losos key remove <fingerprint>", t("tui.help.cmd.keyRemove")],
    ["losos call <METHOD> <path> [json]", t("tui.help.cmd.call")],
    ["losos edge secret list", t("tui.help.cmd.secretList")],
    ["losos edge secret set <name>", t("tui.help.cmd.secretSet")],
    ["losos edge reconcile", t("tui.help.cmd.reconcile")],
    ["losos ci setup --edge <url>", t("tui.help.cmd.ciSetup")],
  ];
  const opts: [string, string][] = [
    ["--host <url>", t("tui.help.opt.host")],
    ["--lang en|sk|de", t("tui.help.opt.lang")],
    ["--theme dark|light", t("tui.help.opt.theme")],
    ["--key <fingerprint>", t("tui.help.opt.key")],
    ["--fingerprint <fpr>", t("tui.help.opt.fingerprint")],
    ["--repo <owner/name>", t("tui.help.opt.repo")],
    ["--print", t("tui.help.opt.print")],
    ["--version", t("tui.help.opt.version")],
  ];
  const width = 36;
  return [
    `losos ${VERSION}`,
    t("tui.help.what"),
    "",
    t("tui.help.commands"),
    ...rows.map(([c, d]) => `  ${c.padEnd(width)}${d}`),
    "",
    t("tui.help.options"),
    ...opts.map(([c, d]) => `  ${c.padEnd(width)}${d}`),
    "",
    t("tui.help.keychain"),
    t("tui.help.ci"),
    "",
  ].join("\n");
}

// ── Keys ─────────────────────────────────────────────────────────────────

async function importKey(io: Io, vault: Vault, armored: string): Promise<void> {
  const key = await readUserKey(armored);
  const fingerprint = fingerprintOf(key);
  const label = `losos gpg-key ${userIdsOf(key)[0] ?? fingerprint}`;
  await vault.putKey(fingerprint, key.armor(), label);
  line(io, t("tui.key.imported", { fingerprint: groupFingerprint(fingerprint) }));
  for (const id of userIdsOf(key)) line(io, `  ${id}`);
  if (needsPassphrase(key)) line(io, t("tui.key.protected"));
}

function exportFromGpg(id: string): string {
  const run = spawnSync("gpg", ["--export-secret-keys", "--armor", id], { encoding: "utf8", stdio: ["inherit", "pipe", "pipe"] });
  if (run.error !== undefined) throw new UserError(t("tui.key.noGpg"));
  if (run.status !== 0 || run.stdout.trim().length === 0) {
    throw new UserError(t("tui.key.gpgFailed", { id, detail: run.stderr.trim() || `exit ${run.status ?? "?"}` }));
  }
  return run.stdout;
}

interface KeyInfo {
  key: PrivateKey;
  fingerprint: string;
}

async function storedKeys(vault: Vault): Promise<KeyInfo[]> {
  const out: KeyInfo[] = [];
  for (const armored of await vault.listKeys()) {
    try {
      const key = await readUserKey(armored);
      out.push({ key, fingerprint: fingerprintOf(key) });
    } catch {
      /* not a usable key: list skips it */
    }
  }
  return out;
}

// ── Hosts ────────────────────────────────────────────────────────────────

async function fetchHostKey(io: Io, url: string): Promise<{ kind: HostKind; fingerprint: string; publicKey: string }> {
  const tries: HostKind[] = ["box", "edge"];
  let last = "";
  for (const kind of tries) {
    let response: Response;
    try {
      response = await io.fetch(hostKeyUrl(url, kind), { headers: { Accept: "application/json" } });
    } catch (error) {
      const cause = (error as { cause?: { message?: string } }).cause?.message;
      throw new UserError(t("tui.login.unreachable", { url, detail: cause ?? (error as Error).message }));
    }
    if (response.status === 200) {
      let doc: { fingerprint?: unknown; publicKey?: unknown };
      try {
        doc = (await response.json()) as typeof doc;
      } catch {
        last = `${hostKeyUrl(url, kind)}: not JSON`;
        continue;
      }
      if (typeof doc.publicKey !== "string") {
        last = `${hostKeyUrl(url, kind)}: no publicKey`;
        continue;
      }
      const key = await readHostKey(doc.publicKey);
      const fingerprint = fingerprintOf(key);
      // The served fingerprint must be the key's own; a mismatch is not a host key.
      if (typeof doc.fingerprint === "string" && !sameFingerprint(doc.fingerprint, fingerprint)) {
        throw new UserError(t("tui.login.inconsistent"));
      }
      return { kind, fingerprint, publicKey: doc.publicKey };
    }
    last = `${hostKeyUrl(url, kind)}: HTTP ${response.status}`;
  }
  throw new UserError(t("tui.login.noHostKey", { url, detail: last }));
}

async function pickHost(vault: Vault, parsed: Parsed, explicit?: string, kind?: HostKind): Promise<HostRecord> {
  const wanted = explicit ?? flag(parsed, "host");
  if (wanted !== undefined) {
    const url = normalizeUrl(wanted);
    const host = await vault.getHost(url);
    if (host === null) throw new UserError(t("tui.host.unknown", { url }));
    return host;
  }
  const hosts = (await vault.listHosts()).filter((h) => kind === undefined || h.kind === kind);
  if (hosts.length === 0) throw new UserError(t(kind === "edge" ? "tui.host.noEdge" : "tui.host.none"));
  if (hosts.length > 1) throw new UserError(t("tui.host.several", { hosts: hosts.map((h) => h.url).join(", ") }));
  return hosts[0]!;
}

async function passphraseFor(io: Io, key: PrivateKey): Promise<PrivateKey> {
  if (!needsPassphrase(key)) return key;
  const fromEnv = io.env["LOSOS_PASSPHRASE"];
  if (fromEnv !== undefined) return unlock(key, fromEnv);
  if (!process.stdin.isTTY) throw new UserError(t("tui.key.needsPassphrase"));
  const passphrase = await readHidden(t("tui.key.passphrase", { fingerprint: groupFingerprint(fingerprintOf(key)).slice(-14) }));
  return unlock(key, passphrase);
}

async function sessionFor(io: Io, vault: Vault, host: HostRecord): Promise<Session> {
  const armored = await vault.getKey(host.keyFingerprint);
  if (armored === null) throw new UserError(t("tui.host.keyGone", { fingerprint: groupFingerprint(host.keyFingerprint) }));
  const key = await passphraseFor(io, await readUserKey(armored));
  const hostKey = await readHostKey(host.publicKey);
  if (!sameFingerprint(fingerprintOf(hostKey), host.fingerprint)) throw new UserError(t("tui.host.pinBroken"));
  return { url: host.url, kind: host.kind, hostKey, hostFingerprint: host.fingerprint, token: host.token, userKey: key };
}

/* CI mode (LOSOS_CI=1): no Secret Service on a runner, so the key, token, URL
 * and pinned host key come from four variables `losos ci setup` put into the
 * repository's Actions settings. The keychain is not touched. */
export function ciMode(io: Io): boolean {
  return io.env["LOSOS_CI"] === "1";
}

async function ciSession(io: Io): Promise<Session> {
  const need = (name: string): string => {
    const value = io.env[name];
    if (value === undefined || value.trim() === "") throw new UserError(t("tui.ci.missingVar", { name }));
    return value;
  };
  const url = normalizeUrl(need("LOSOS_EDGE_URL"));
  const userKey = await readUserKey(need("LOSOS_CI_GPG_KEY"));
  if (needsPassphrase(userKey)) throw new UserError(t("tui.ci.keyProtected"));
  const hostKey = await readHostKey(need("LOSOS_EDGE_HOST_KEY"));
  return {
    url,
    kind: "edge",
    hostKey,
    hostFingerprint: fingerprintOf(hostKey),
    token: need("LOSOS_EDGE_TOKEN").trim(),
    userKey,
  };
}

// ── The full-screen view ─────────────────────────────────────────────────

function screenSize(): { cols: number; rows: number } {
  return { cols: process.stdout.columns ?? 120, rows: process.stdout.rows ?? 36 };
}

async function openTui(io: Io, vault: Vault, parsed: Parsed, theme: Theme, locale: Locale): Promise<void> {
  if (!process.stdout.isTTY || !process.stdin.isTTY) throw new UserError(t("tui.open.needsTerminal"));
  let host: HostRecord;
  const named = flag(parsed, "host") ?? parsed.positionals[1] ?? (parsed.positionals[0] !== "open" ? parsed.positionals[0] : undefined);
  if (named !== undefined) host = await pickHost(vault, parsed, named);
  else {
    const hosts = await vault.listHosts();
    if (hosts.length === 0) throw new UserError(t("tui.host.none"));
    if (hosts.length === 1) host = hosts[0]!;
    else {
      let picked: string | null = null;
      const { cols } = screenSize();
      const app = render(<Picker theme={theme} hosts={hosts} cols={cols} onPick={(url) => (picked = url)} />, inkOptions());
      await app.waitUntilExit();
      if (picked === null) return;
      host = hosts.find((h) => h.url === picked)!;
    }
  }
  const session = await sessionFor(io, vault, host);
  const transport = envelopeTransport(session);
  const Sized = () => {
    const [size, setSize] = React.useState(screenSize());
    React.useEffect(() => {
      const onResize = (): void => setSize(screenSize());
      process.stdout.on("resize", onResize);
      return () => {
        process.stdout.off("resize", onResize);
      };
    }, []);
    const props = { host, transport, theme, locale, cols: size.cols, rows: size.rows };
    return host.kind === "edge" ? <EdgeApp {...props} /> : <BoxApp {...props} />;
  };
  const app = render(<Sized />, { ...inkOptions(), alternateScreen: true, exitOnCtrlC: true });
  await app.waitUntilExit();
}

/** Ink draws only its last frame when `CI` is set, which leaves a person at
 * a real terminal (a dev container, a CI debug shell) looking at nothing.
 * The terminal decides, not the environment. */
function inkOptions(): { interactive: boolean } {
  return { interactive: Boolean(process.stdout.isTTY) };
}

// ── Commands ─────────────────────────────────────────────────────────────

async function login(io: Io, vault: Vault, parsed: Parsed, theme: Theme): Promise<void> {
  const raw = parsed.positionals[1];
  if (raw === undefined) throw new UserError(t("tui.login.usage"));
  let url: string;
  try {
    url = normalizeUrl(raw);
  } catch {
    throw new UserError(t("tui.login.badUrl", { url: raw }));
  }
  const served = await fetchHostKey(io, url);
  const pinned = await vault.getHost(url);
  if (pinned !== null && !sameFingerprint(pinned.fingerprint, served.fingerprint)) {
    io.err(
      [
        "",
        t("tui.login.changed1"),
        t("tui.login.changed2", { url }),
        "",
        `  ${t("tui.login.pinned")}  ${groupFingerprint(pinned.fingerprint)}`,
        `  ${t("tui.login.served")}  ${groupFingerprint(served.fingerprint)}`,
        "",
        t("tui.login.changed3"),
        t("tui.login.changed4", { url }),
        "",
      ].join("\n"),
    );
    throw new UserError(t("tui.login.refused"), 2);
  }

  const keys = await storedKeys(vault);
  if (keys.length === 0) throw new UserError(t("tui.login.noKeys"));
  const wantedKey = flag(parsed, "key");
  let keyFingerprint: string | undefined;
  if (wantedKey !== undefined) {
    const match = keys.find((k) => sameFingerprint(k.fingerprint, wantedKey) || k.fingerprint.endsWith(wantedKey.replace(/\s+/g, "").toUpperCase()));
    if (match === undefined) throw new UserError(t("tui.login.keyNotStored", { fingerprint: wantedKey }));
    keyFingerprint = match.fingerprint;
  }
  const expected = flag(parsed, "fingerprint");
  if (expected !== undefined && !sameFingerprint(expected, served.fingerprint)) {
    throw new UserError(t("tui.login.flagMismatch", { served: groupFingerprint(served.fingerprint) }), 2);
  }
  const confirmed = expected !== undefined || (pinned !== null && sameFingerprint(pinned.fingerprint, served.fingerprint));

  let answer: LoginAnswer | null = null;
  if (process.stdin.isTTY && process.stdout.isTTY) {
    const { cols } = screenSize();
    const app = render(
      <Login
        theme={theme}
        url={url}
        kind={served.kind}
        fingerprint={served.fingerprint}
        keys={keys.map((k) => ({ fingerprint: k.fingerprint, userIds: userIdsOf(k.key) }))}
        confirmed={confirmed}
        {...(keyFingerprint === undefined ? {} : { keyFingerprint })}
        cols={cols}
        onDone={(a) => (answer = a)}
      />,
      inkOptions(),
    );
    await app.waitUntilExit();
    if (answer === null) throw new UserError(t("tui.login.cancelled"));
  } else {
    // A script: the fingerprint must be confirmed on the command line, the
    // token comes on stdin, the key is named or is the only one.
    if (!confirmed) throw new UserError(t("tui.login.needsFingerprint", { served: groupFingerprint(served.fingerprint) }));
    const token = (await readSecret("")).trim();
    if (keyFingerprint === undefined) {
      if (keys.length > 1) throw new UserError(t("tui.login.whichKeyFlag"));
      keyFingerprint = keys[0]!.fingerprint;
    }
    answer = { token, keyFingerprint };
  }
  const given = answer as LoginAnswer;
  if (!/^losos_pat_[0-9a-f]{8}_[0-9a-f]{40}$/.test(given.token.trim())) throw new UserError(t("tui.login.badToken"));
  await vault.putHost({
    url,
    kind: served.kind,
    fingerprint: served.fingerprint,
    publicKey: served.publicKey,
    token: given.token.trim(),
    keyFingerprint: given.keyFingerprint,
  });
  line(io, t("tui.login.done", { url, kind: t(served.kind === "edge" ? "tui.kind.edge" : "tui.kind.box") }));
}

function parseBody(raw: string | undefined): unknown {
  if (raw === undefined) return null;
  try {
    return JSON.parse(raw);
  } catch {
    throw new UserError(t("tui.call.badJson"));
  }
}

async function callCommand(io: Io, parsed: Parsed): Promise<number> {
  const [, method, path, body] = parsed.positionals;
  if (method === undefined || path === undefined) throw new UserError(t("tui.call.usage"));
  const session = ciMode(io) ? await ciSession(io) : await sessionFor(io, new Vault(io.keychain()), await pickHost(new Vault(io.keychain()), parsed));
  const answer = await call(session, method, path, parseBody(body), { fetch: io.fetch });
  io.err(`${answer.status}${answer.sealed ? "" : `, ${t("tui.call.notSealed")}`}\n`);
  line(io, typeof answer.body === "string" ? answer.body : JSON.stringify(answer.body, null, 2));
  return answer.status >= 400 ? 1 : 0;
}

async function edgeSession(io: Io, parsed: Parsed): Promise<Session> {
  if (ciMode(io)) return ciSession(io);
  const vault = new Vault(io.keychain());
  const host = await pickHost(vault, parsed, undefined, "edge");
  if (host.kind !== "edge") throw new UserError(t("tui.edge.notEdge", { url: host.url }));
  return sessionFor(io, vault, host);
}

async function edgeCommand(io: Io, parsed: Parsed): Promise<number> {
  const [, what, verb, name] = parsed.positionals;
  if (what === "reconcile") {
    const session = await edgeSession(io, parsed);
    const answer = await call(session, "POST", "/reconcile", null, { fetch: io.fetch });
    if (answer.status >= 400) throw new UserError(errorText(answer));
    line(io, t("tui.edge.reconciled"));
    return 0;
  }
  if (what === "secret" && verb === "list") {
    const session = await edgeSession(io, parsed);
    const answer = await call(session, "GET", "/secrets", null, { fetch: io.fetch });
    if (answer.status >= 400) throw new UserError(errorText(answer));
    const list = Array.isArray(answer.body) ? (answer.body as { name: string; sealed: boolean; updated: unknown }[]) : [];
    for (const s of list) {
      const when = typeof s.updated === "number" ? new Date(s.updated * 1000).toISOString() : typeof s.updated === "string" ? s.updated : "-";
      line(io, `${s.name.padEnd(32)} ${s.sealed ? t("tui.edge.sealed") : t("tui.edge.notSealed")}  ${when}`);
    }
    return 0;
  }
  if (what === "secret" && verb === "set" && name !== undefined) {
    const session = await edgeSession(io, parsed);
    const value = await readSecret(t("tui.edge.valuePrompt", { name }));
    if (value.length === 0) throw new UserError(t("tui.edge.emptyValue"));
    const answer = await call(session, "POST", "/secrets", { name, value }, { fetch: io.fetch });
    if (answer.status >= 400) throw new UserError(errorText(answer));
    line(io, t("tui.edge.secretSet", { name }));
    if (answer.body !== null && answer.body !== undefined && answer.body !== "") {
      line(io, typeof answer.body === "string" ? answer.body : JSON.stringify(answer.body, null, 2));
    }
    return 0;
  }
  throw new UserError(t("tui.edge.usage"));
}

// ── CI setup ─────────────────────────────────────────────────────────────

interface Gh {
  available: boolean;
  run: (args: string[], input: string) => Promise<number>;
}

function gh(io: Io): Gh {
  const command = io.env["LOSOS_GH"] ?? "gh";
  const status = spawnSync(command, ["auth", "status"], { stdio: "ignore", env: io.env });
  return {
    available: status.error === undefined && status.status === 0,
    run: (args, input) =>
      new Promise((resolve) => {
        const child = spawn(command, args, { stdio: ["pipe", "ignore", "inherit"], env: io.env });
        child.on("error", () => resolve(127));
        child.on("close", (code) => resolve(code ?? 1));
        child.stdin.end(input);
      }),
  };
}

async function ciSetup(io: Io, parsed: Parsed): Promise<number> {
  const edgeFlag = flag(parsed, "edge");
  if (edgeFlag === undefined) throw new UserError(t("tui.ci.usage"));
  const repo = flag(parsed, "repo") ?? "dasmatus/losos";
  const vault = new Vault(io.keychain());
  const url = normalizeUrl(edgeFlag);
  const host = await vault.getHost(url);
  if (host === null) throw new UserError(t("tui.ci.notLoggedIn", { url }));
  if (host.kind !== "edge") throw new UserError(t("tui.edge.notEdge", { url }));

  // A key for CI alone, never the owner's own.
  const generated = await openpgp.generateKey({
    type: "ecc",
    curve: "curve25519Legacy",
    userIDs: [{ name: `LosOS CI <${repo}>` }],
    format: "object",
  });
  const ciKey = generated.privateKey;
  const fingerprint = fingerprintOf(ciKey);
  const secretArmor = ciKey.armor();
  await vault.putKey(fingerprint, secretArmor, `losos gpg-key LosOS CI <${repo}>`);

  line(io, t("tui.ci.generated", { repo }));
  line(io, `  ${groupFingerprint(fingerprint)}`);
  line(io);
  line(io, generated.publicKey.armor().trim());
  line(io);
  line(io, t("tui.ci.seed"));
  line(io, `  losos.edge.access.gpgKeys`);
  line(io, t("tui.ci.token"));
  line(io, `  losos-registrar provision token create --edge ${url} --name ci --key ${fingerprint} --scope full`);
  line(io);

  let token: string;
  try {
    token = (await readSecret(t("tui.ci.tokenPrompt"))).trim();
  } catch (error) {
    if (error instanceof Cancelled) throw new UserError(t("tui.login.cancelled"));
    throw error;
  }
  if (!/^losos_pat_[0-9a-f]{8}_[0-9a-f]{40}$/.test(token)) throw new UserError(t("tui.login.badToken"));

  const values: { name: string; value: string; secret: boolean }[] = [
    { name: "LOSOS_CI_GPG_KEY", value: secretArmor, secret: true },
    { name: "LOSOS_EDGE_TOKEN", value: token, secret: true },
    { name: "LOSOS_EDGE_URL", value: url, secret: false },
    { name: "LOSOS_EDGE_HOST_KEY", value: host.publicKey, secret: false },
  ];
  const tool = gh(io);
  const print = parsed.flags.get("print") === true;
  if (tool.available) {
    for (const v of values) {
      const code = await tool.run([v.secret ? "secret" : "variable", "set", v.name, "--repo", repo], v.value);
      if (code !== 0) throw new UserError(t("tui.ci.ghFailed", { name: v.name, code }));
      line(io, t(v.secret ? "tui.ci.setSecret" : "tui.ci.setVariable", { name: v.name, repo }));
    }
  } else {
    line(io, t("tui.ci.noGh", { repo }));
    for (const v of values) line(io, `  ${v.name}  (${t(v.secret ? "tui.ci.kindSecret" : "tui.ci.kindVariable")})`);
    if (print) {
      for (const v of values) {
        line(io);
        line(io, `${v.name}:`);
        line(io, v.value.trim());
      }
    } else {
      line(io, t("tui.ci.printHint"));
    }
  }
  line(io, t("tui.ci.rotate", { fingerprint: groupFingerprint(fingerprint) }));
  return 0;
}

// ── Entry ────────────────────────────────────────────────────────────────

export async function run(argv: readonly string[], io: Io = defaultIo): Promise<number> {
  const parsed = parseArgs(argv);
  const locale = detectLocale(io.env, flag(parsed, "lang"));
  setLocale(locale);
  const theme = makeTheme(detectMode(io.env, flag(parsed, "theme")), detectTruecolor(io.env));
  if (parsed.flags.get("version") === true) {
    line(io, `losos ${VERSION}`);
    return 0;
  }
  if (parsed.flags.get("help") === true || parsed.positionals[0] === "help") {
    io.out(helpText());
    return 0;
  }
  const command = parsed.positionals[0];
  try {
    switch (command) {
      case "call":
        return await callCommand(io, parsed);
      case "edge":
        return await edgeCommand(io, parsed);
      default:
        break;
    }
    const keychain = io.keychain();
    if (keychain instanceof SecretToolKeychain) await keychain.probe();
    const vault = new Vault(keychain);
    switch (command) {
      case undefined:
      case "open":
        await openTui(io, vault, parsed, theme, locale);
        return 0;
      case "login":
        await login(io, vault, parsed, theme);
        return 0;
      case "logout": {
        const raw = parsed.positionals[1];
        if (raw === undefined) throw new UserError(t("tui.logout.usage"));
        const url = normalizeUrl(raw);
        if ((await vault.getHost(url)) === null) throw new UserError(t("tui.host.unknown", { url }));
        await vault.removeHost(url);
        line(io, t("tui.logout.done", { url }));
        return 0;
      }
      case "hosts": {
        const hosts = await vault.listHosts();
        if (hosts.length === 0) line(io, t("tui.host.none"));
        for (const h of hosts) {
          line(io, `${h.url}  ${t(h.kind === "edge" ? "tui.kind.edge" : "tui.kind.box")}`);
          line(io, `  ${t("tui.login.hostKey")}: ${groupFingerprint(h.fingerprint)}`);
          line(io, `  ${t("tui.about.userKey")}: ${groupFingerprint(h.keyFingerprint)}`);
        }
        return 0;
      }
      case "key": {
        const verb = parsed.positionals[1];
        if (verb === "import") {
          const gpgId = flag(parsed, "gpg");
          const file = parsed.positionals[2];
          if (gpgId === undefined && file === undefined) throw new UserError(t("tui.key.importUsage"));
          let armored: string;
          if (gpgId !== undefined) armored = exportFromGpg(gpgId);
          else {
            try {
              armored = readFileSync(file!, "utf8");
            } catch {
              throw new UserError(t("tui.key.unreadable", { file: file! }));
            }
          }
          await importKey(io, vault, armored);
          return 0;
        }
        if (verb === "list") {
          const keys = await storedKeys(vault);
          if (keys.length === 0) line(io, t("tui.key.none"));
          for (const k of keys) {
            line(io, groupFingerprint(k.fingerprint));
            for (const id of userIdsOf(k.key)) line(io, `  ${id}`);
          }
          return 0;
        }
        if (verb === "remove") {
          const fpr = parsed.positionals[2];
          if (fpr === undefined) throw new UserError(t("tui.key.removeUsage"));
          const keys = await storedKeys(vault);
          const match = keys.find((k) => sameFingerprint(k.fingerprint, fpr));
          if (match === undefined) throw new UserError(t("tui.login.keyNotStored", { fingerprint: fpr }));
          const users = (await vault.listHosts()).filter((h) => sameFingerprint(h.keyFingerprint, match.fingerprint));
          await vault.removeKey(match.fingerprint);
          line(io, t("tui.key.removed", { fingerprint: groupFingerprint(match.fingerprint) }));
          if (users.length > 0) line(io, t("tui.key.orphans", { hosts: users.map((h) => h.url).join(", ") }));
          return 0;
        }
        throw new UserError(t("tui.key.usage"));
      }
      case "ci":
        if (parsed.positionals[1] === "setup") return await ciSetup(io, parsed);
        throw new UserError(t("tui.ci.usage"));
      default:
        throw new UserError(t("tui.unknownCommand", { command }));
    }
  } catch (error) {
    if (error instanceof UserError) {
      io.err(`losos: ${error.message}\n`);
      return error.code;
    }
    if (error instanceof KeychainUnavailable || error instanceof KeyProblem || error instanceof EnvelopeError) {
      io.err(`losos: ${error.message}\n`);
      return 1;
    }
    if (error instanceof Cancelled) {
      io.err(`losos: ${t("tui.login.cancelled")}\n`);
      return 130;
    }
    io.err(`losos: ${error instanceof Error ? error.message : String(error)}\n`);
    return 1;
  }
}
