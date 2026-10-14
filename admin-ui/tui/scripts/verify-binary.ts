/* Drives a built losos through a real pseudo-terminal, against the mock box
 * and edge, to show the parts a unit test cannot: Ink's raw-mode input on the
 * login screen and in the full-screen view, the hidden prompt, and the exit
 * statuses. util-linux `script` provides the terminal.
 *
 *   node --import tsx scripts/verify-binary.ts            # dist/losos
 *   node --import tsx scripts/verify-binary.ts --bundle   # node dist/losos.mjs
 */

import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { existsSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { delimiter, join } from "node:path";
import { fileURLToPath } from "node:url";
import * as openpgp from "openpgp";
import { generateLegacyKey, startMock } from "../test/mock-server";

const root = fileURLToPath(new URL("..", import.meta.url));
const bundleMode = process.argv.includes("--bundle");
const command = bundleMode ? `${process.execPath} ${join(root, "dist", "losos.mjs")}` : join(root, "dist", "losos");
if (!bundleMode && !existsSync(command)) {
  console.error("verify-binary: dist/losos is missing. npm run build:binary makes it.");
  process.exit(1);
}

const dir = mkdtempSync(join(tmpdir(), "losos-verify-"));
const env: NodeJS.ProcessEnv = {
  ...process.env,
  PATH: `${join(root, "test", "fake-bin")}${delimiter}${process.env["PATH"] ?? ""}`,
  FAKE_SECRET_STORE: join(dir, "store.json"),
  LANG: "C.UTF-8",
  TERM: "xterm-256color",
  COLORTERM: "truecolor",
};
const strip = (s: string): string => s.replace(/\x1b\[[0-9;?]*[ -/]*[@-~]/g, "").replace(/\x1b[()][0-9A-B]/g, "");

let failures = 0;
function check(ok: boolean, what: string): void {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) failures += 1;
}

/** Runs `losos <args>` with no terminal. */
function plain(args: string[], input = ""): Promise<{ code: number; out: string; err: string }> {
  return new Promise((resolve) => {
    const [exe, ...pre] = command.split(" ");
    const child = spawn(exe!, [...pre, "--lang", "en", ...args], { env });
    let out = "";
    let err = "";
    child.stdout.on("data", (c: Buffer) => (out += c));
    child.stderr.on("data", (c: Buffer) => (err += c));
    child.on("close", (code) => resolve({ code: code ?? 1, out, err }));
    child.stdin.end(input);
  });
}

interface Pty {
  child: ChildProcessWithoutNullStreams;
  output: () => string;
  waitFor: (text: string, ms?: number) => Promise<boolean>;
  send: (keys: string) => Promise<void>;
  exit: Promise<number>;
}

/** Runs `losos <args>` on a 120x36 pseudo-terminal. */
function pty(args: string[]): Pty {
  const quoted = ["--lang", "en", ...args].map((a) => `'${a.replace(/'/g, "'\\''")}'`).join(" ");
  const child = spawn("script", ["-qfec", `stty cols 120 rows 36; exec ${command} ${quoted}`, "/dev/null"], { env });
  let raw = "";
  child.stdout.on("data", (c: Buffer) => (raw += c.toString("utf8")));
  child.stderr.on("data", (c: Buffer) => (raw += c.toString("utf8")));
  const exit = new Promise<number>((resolve) => child.on("close", (code) => resolve(code ?? 1)));
  return {
    child,
    output: () => strip(raw),
    waitFor: async (text, ms = 8000) => {
      const end = Date.now() + ms;
      while (Date.now() < end) {
        if (strip(raw).includes(text)) return true;
        await new Promise((r) => setTimeout(r, 50));
      }
      return false;
    },
    send: async (keys) => {
      for (const ch of keys) {
        child.stdin.write(ch);
        await new Promise((r) => setTimeout(r, 15));
      }
    },
    exit,
  };
}

async function withTimeout<T>(p: Promise<T>, ms: number, fallback: T): Promise<T> {
  return Promise.race([p, new Promise<T>((r) => setTimeout(() => r(fallback), ms))]);
}

const owner = await generateLegacyKey("Owner");
const ownerFpr = (await openpgp.readKey({ armoredKey: owner.publicKey })).getFingerprint().toUpperCase();
const token = `losos_pat_0a1b2c3d_${"a".repeat(40)}`;
const box = await startMock({ kind: "box", seeded: [owner.publicKey], tokens: [{ token, fingerprint: ownerFpr, scope: "full", expires: null }] });
const edge = await startMock({ kind: "edge", seeded: [owner.publicKey], tokens: [{ token, fingerprint: ownerFpr, scope: "full", expires: null }] });
const group = (f: string): string => f.match(/.{4}/g)!.slice(0, 5).join(" ");

try {
  console.log(`verify-binary: ${command}`);
  const version = await plain(["--version"]);
  check(version.code === 0 && /^losos /.test(version.out), `--version: ${version.out.trim()}`);
  const help = await plain(["--help"]);
  check(help.code === 0 && help.out.includes("ci setup --edge"), "--help lists the commands");

  writeFileSync(join(dir, "owner.asc"), owner.privateKey);
  const imported = await plain(["key", "import", join(dir, "owner.asc")]);
  check(imported.code === 0, "key import from a file");

  // The login screen: y to the fingerprint, the token typed without echo.
  const login = pty(["login", box.url]);
  check(await login.waitFor(group(box.hostFingerprint)), "login shows the host key in groups of four");
  await login.send("y");
  check(await login.waitFor("Access token"), "y moves on to the token");
  await login.send(`${token}\r`);
  const loginCode = await withTimeout(login.exit, 8000, -1);
  check(loginCode === 0, `login exits 0 (got ${loginCode})`);
  check(!login.output().includes(token), "the token was not echoed");

  // The full-screen view: arrow keys, Enter, q.
  const tui = pty(["open", box.url]);
  check(await tui.waitFor("jellyfin"), "the overview draws with the box's data");
  await tui.send("j");
  await tui.send("\r");
  const opened = await tui.waitFor("Use reserve");
  check(opened, "j and Enter open Storage");
  if (!opened && process.env["VERIFY_DEBUG"] === "1") console.log(tui.output().slice(-6000));
  await tui.send("\t");
  tui.child.stdin.write("\x1b[B");
  await new Promise((r) => setTimeout(r, 200));
  await tui.send("q");
  const tuiCode = await withTimeout(tui.exit, 8000, -1);
  check(tuiCode === 0, `q leaves the view with status 0 (got ${tuiCode})`);

  // The hidden prompt outside Ink.
  const edgeLogin = await plain(["login", edge.url, "--fingerprint", edge.hostFingerprint], `${token}\n`);
  check(edgeLogin.code === 0, "edge login from a script");
  const set = pty(["edge", "secret", "set", "stripe-secret-key"]);
  check(await set.waitFor("Value for stripe-secret-key"), "edge secret set asks for the value");
  await set.send("sk_test_9\r");
  const setCode = await withTimeout(set.exit, 8000, -1);
  check(setCode === 0, `edge secret set exits 0 (got ${setCode})`);
  const sent = edge.calls.at(-1);
  check(sent?.path === "/secrets" && (sent.body as { value?: string }).value === "sk_test_9", "the value reached the edge");
  check(!set.output().includes("sk_test_9"), "the value was not echoed");

  const called = await plain(["call", "GET", "/api/state", "--host", box.url]);
  check(called.code === 0 && called.out.includes('"mode"'), "call GET /api/state");
  const missing = await plain(["call", "GET", "/api/nothing", "--host", box.url]);
  check(missing.code === 1, "call exits 1 on a 404");
} finally {
  await box.close();
  await edge.close();
  rmSync(dir, { recursive: true, force: true });
}
console.log(failures === 0 ? "verify-binary: all good" : `verify-binary: ${failures} failed`);
process.exit(failures === 0 ? 0 : 1);
