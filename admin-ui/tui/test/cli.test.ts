/* The bundled CLI, end to end: `node dist/losos.mjs` in a child process, with
 * the fake secret-tool and the fake gh first on PATH and the mock box and
 * edge in this process. `npm test` builds the bundle first. */

import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { delimiter, join } from "node:path";
import { after, before, describe, it } from "node:test";
import { fileURLToPath } from "node:url";
import * as openpgp from "openpgp";
import { generateLegacyKey, startMock, type Mock } from "./mock-server";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const BIN = join(ROOT, "dist", "losos.mjs");
const FAKE_BIN = join(ROOT, "test", "fake-bin");

const FULL = `losos_pat_0a1b2c3d_${"a".repeat(40)}`;
const EDGE_TOKEN = `losos_pat_e0e1e2e3_${"e".repeat(40)}`;
const CI_TOKEN = `losos_pat_c1c1c1c1_${"c".repeat(40)}`;

interface Result {
  code: number;
  stdout: string;
  stderr: string;
}

function group(fpr: string): string {
  return fpr.match(/.{4}/g)!.join(" ");
}

describe("the losos command", () => {
  let dir: string;
  let store: string;
  let secretLog: string;
  let ghLog: string;
  let box: Mock;
  let edge: Mock;
  let owner: Awaited<ReturnType<typeof generateLegacyKey>>;
  let ownerFpr: string;
  let ownerFile: string;

  const env = (extra: Record<string, string | undefined> = {}): NodeJS.ProcessEnv => {
    const base: NodeJS.ProcessEnv = {
      PATH: `${FAKE_BIN}${delimiter}${process.env["PATH"] ?? ""}`,
      HOME: dir,
      LANG: "C",
      FAKE_SECRET_STORE: store,
      FAKE_SECRET_LOG: secretLog,
      FAKE_GH_LOG: ghLog,
    };
    for (const [k, v] of Object.entries(extra)) {
      if (v === undefined) delete base[k];
      else base[k] = v;
    }
    return base;
  };

  /* Async on purpose: the mock servers live in this process, so a spawnSync
   * would block the very event loop that has to answer the child. */
  const losos = (args: string[], options: { input?: string; env?: Record<string, string | undefined> } = {}): Promise<Result> =>
    new Promise((resolve) => {
      const child = spawn(process.execPath, [BIN, "--lang", "en", ...args], { env: env(options.env), stdio: ["pipe", "pipe", "pipe"] });
      let stdout = "";
      let stderr = "";
      child.stdout.on("data", (c: Buffer) => (stdout += c.toString("utf8")));
      child.stderr.on("data", (c: Buffer) => (stderr += c.toString("utf8")));
      child.on("close", (code) => resolve({ code: code ?? 1, stdout, stderr }));
      child.stdin.end(options.input ?? "");
    });

  const items = (): { label: string; attributes: Record<string, string>; secret: string }[] =>
    existsSync(store) ? JSON.parse(readFileSync(store, "utf8")) : [];
  const secretCalls = (): number => (existsSync(secretLog) ? readFileSync(secretLog, "utf8").trim().split("\n").filter(Boolean).length : 0);
  const ghCalls = (): { argv: string[]; stdin: string }[] =>
    existsSync(ghLog) ? readFileSync(ghLog, "utf8").trim().split("\n").filter(Boolean).map((l) => JSON.parse(l)) : [];

  before(async () => {
    assert.ok(existsSync(BIN), "dist/losos.mjs is missing: run npm run build");
    dir = mkdtempSync(join(tmpdir(), "losos-cli-"));
    store = join(dir, "store.json");
    secretLog = join(dir, "secret-tool.log");
    ghLog = join(dir, "gh.log");
    owner = await generateLegacyKey("Owner");
    ownerFpr = (await openpgp.readKey({ armoredKey: owner.publicKey })).getFingerprint().toUpperCase();
    ownerFile = join(dir, "owner.asc");
    writeFileSync(ownerFile, owner.privateKey);
    box = await startMock({ kind: "box", seeded: [owner.publicKey], tokens: [{ token: FULL, fingerprint: ownerFpr, scope: "full", expires: null }] });
    edge = await startMock({
      kind: "edge",
      seeded: [owner.publicKey],
      tokens: [{ token: EDGE_TOKEN, fingerprint: ownerFpr, scope: "full", expires: null }],
    });
  });
  after(async () => {
    await box.close();
    await edge.close();
    rmSync(dir, { recursive: true, force: true });
  });

  it("prints its version and its commands", async () => {
    const version = await losos(["--version"]);
    assert.equal(version.code, 0);
    assert.match(version.stdout, /^losos \S+\n$/);
    const help = await losos(["--help"]);
    assert.equal(help.code, 0);
    for (const cmd of ["login <url>", "key import --gpg", "call <METHOD>", "edge secret set", "edge reconcile", "ci setup --edge", "LOSOS_CI=1"]) {
      assert.ok(help.stdout.includes(cmd), `--help names ${cmd}`);
    }
  });

  it("says so in one sentence when there is no Secret Service", async () => {
    const down = await losos(["hosts"], { env: { FAKE_SECRET_DOWN: "1" } });
    assert.equal(down.code, 1);
    assert.match(down.stderr, /^losos: .*Secret Service.*\n$/);
    const missing = await losos(["key", "list"], { env: { PATH: "/nonexistent" } });
    assert.equal(missing.code, 1);
    assert.match(missing.stderr, /secret-tool/);
    assert.equal(existsSync(store), false, "nothing was written anywhere");
  });

  it("imports a key from a file, lists it and refuses a public key", async () => {
    const imported = await losos(["key", "import", ownerFile]);
    assert.equal(imported.code, 0, imported.stderr);
    assert.ok(imported.stdout.includes(group(ownerFpr)));
    const stored = items().filter((i) => i.attributes["kind"] === "gpg-key");
    assert.equal(stored.length, 1);
    assert.equal(stored[0]!.attributes["service"], "losos");
    assert.equal(stored[0]!.attributes["fingerprint"], ownerFpr);
    const list = await losos(["key", "list"]);
    assert.equal(list.code, 0);
    assert.ok(list.stdout.includes(group(ownerFpr)));
    assert.ok(list.stdout.includes("Owner"));
    const pub = join(dir, "public.asc");
    writeFileSync(pub, owner.publicKey);
    const refused = await losos(["key", "import", pub]);
    assert.equal(refused.code, 1);
    assert.match(refused.stderr, /public key/);
  });

  it("imports a key from gpg", { skip: spawnSync("gpg", ["--version"]).status !== 0 ? "no gpg" : false }, async () => {
    const home = mkdtempSync(join(dir, "gnupg-"));
    const other = await generateLegacyKey("From gpg");
    const fpr = (await openpgp.readKey({ armoredKey: other.publicKey })).getFingerprint().toUpperCase();
    const imported = spawnSync("gpg", ["--batch", "--import"], { input: other.privateKey, env: { ...process.env, GNUPGHOME: home } });
    assert.equal(imported.status, 0, imported.stderr.toString());
    const result = await losos(["key", "import", "--gpg", fpr], { env: { GNUPGHOME: home } });
    spawnSync("gpgconf", ["--kill", "gpg-agent"], { env: { ...process.env, GNUPGHOME: home } });
    assert.equal(result.code, 0, result.stderr);
    assert.ok(result.stdout.includes(group(fpr)));
    const removed = await losos(["key", "remove", fpr]);
    assert.equal(removed.code, 0, removed.stderr);
    assert.equal(items().filter((i) => i.attributes["kind"] === "gpg-key").length, 1);
  });

  it("logs in to a box from a script, with the fingerprint on the command line", async () => {
    const unconfirmed = await losos(["login", box.url], { input: `${FULL}\n` });
    assert.equal(unconfirmed.code, 1);
    assert.ok(unconfirmed.stderr.includes(group(box.hostFingerprint)), "the refusal shows the fingerprint to compare");
    const wrong = await losos(["login", box.url, "--fingerprint", "0".repeat(40)], { input: `${FULL}\n` });
    assert.equal(wrong.code, 2);
    const badToken = await losos(["login", box.url, "--fingerprint", box.hostFingerprint], { input: "not-a-token\n" });
    assert.equal(badToken.code, 1);
    assert.equal(items().filter((i) => i.attributes["kind"] === "host").length, 0);

    const ok = await losos(["login", box.url, "--fingerprint", group(box.hostFingerprint)], { input: `${FULL}\n` });
    assert.equal(ok.code, 0, ok.stderr);
    const host = items().find((i) => i.attributes["kind"] === "host")!;
    assert.equal(host.attributes["url"], box.url);
    const record = JSON.parse(host.secret) as Record<string, string>;
    assert.equal(record["kind"], "box");
    assert.equal(record["fingerprint"], box.hostFingerprint);
    assert.equal(record["keyFingerprint"], ownerFpr);
    assert.equal(record["token"], FULL);
  });

  it("calls the box through the envelope and exits on the inner status", async () => {
    const ok = await losos(["call", "GET", "/api/state"]);
    assert.equal(ok.code, 0, ok.stderr);
    assert.deepEqual(JSON.parse(ok.stdout), { mode: "local", sharing: false });
    assert.equal(ok.stderr.trim(), "200");
    const missing = await losos(["call", "GET", "/api/nothing"]);
    assert.equal(missing.code, 1);
    assert.match(missing.stderr, /^404/);
    const posted = await losos(["call", "POST", "/api/grow", "{}"]);
    assert.equal(posted.code, 0);
    assert.deepEqual(box.calls.at(-1), { method: "POST", path: "/api/grow", body: {} });
    const badJson = await losos(["call", "POST", "/api/grow", "{nope"]);
    assert.equal(badJson.code, 1);
  });

  it("refuses loudly when a pinned host shows a different key", async () => {
    const all = items();
    const host = all.find((i) => i.attributes["kind"] === "host")!;
    const record = JSON.parse(host.secret) as Record<string, string>;
    const real = record["fingerprint"]!;
    record["fingerprint"] = "1234".repeat(10);
    host.secret = JSON.stringify(record);
    writeFileSync(store, JSON.stringify(all));
    const before = host.secret;
    const result = await losos(["login", box.url, "--fingerprint", real], { input: `${FULL}\n` });
    assert.equal(result.code, 2);
    assert.match(result.stderr, /THE HOST KEY HAS CHANGED/);
    assert.ok(result.stderr.includes("1234 1234 1234"));
    assert.ok(result.stderr.includes(group(real)));
    assert.match(result.stderr, /Refused/);
    assert.equal(items().find((i) => i.attributes["kind"] === "host")!.secret, before, "the pin was not replaced");
    // A broken pin is refused for calls too.
    const call = await losos(["call", "GET", "/api/state"]);
    assert.equal(call.code, 1);
    // Put it back for the tests that follow.
    record["fingerprint"] = real;
    host.secret = JSON.stringify(record);
    writeFileSync(store, JSON.stringify(all));
  });

  it("logs in to an edge by the edge path and runs the edge commands", async () => {
    const login = await losos(["login", edge.url, "--fingerprint", edge.hostFingerprint], { input: `${EDGE_TOKEN}\n` });
    assert.equal(login.code, 0, login.stderr);
    assert.match(login.stdout, /edge/);
    const hosts = await losos(["hosts"]);
    assert.ok(hosts.stdout.includes(box.url) && hosts.stdout.includes(edge.url));

    const list = await losos(["edge", "secret", "list"]);
    assert.equal(list.code, 0, list.stderr);
    assert.match(list.stdout, /stripe-secret-key\s+Sealed/);
    assert.match(list.stdout, /claude-api-key\s+not set/i);

    const set = await losos(["edge", "secret", "set", "stripe-secret-key"], { input: "sk_test_123\n" });
    assert.equal(set.code, 0, set.stderr);
    assert.deepEqual(edge.calls.at(-1), { method: "POST", path: "/secrets", body: { name: "stripe-secret-key", value: "sk_test_123" } });
    assert.ok(!set.stdout.includes("sk_test_123"), "the value is not echoed");

    const unknown = await losos(["edge", "secret", "set", "NOPE"], { input: "x\n" });
    assert.equal(unknown.code, 1);
    const empty = await losos(["edge", "secret", "set", "stripe-secret-key"], { input: "" });
    assert.equal(empty.code, 1);

    const reconcile = await losos(["edge", "reconcile"]);
    assert.equal(reconcile.code, 0, reconcile.stderr);
    assert.deepEqual(edge.calls.at(-1), { method: "POST", path: "/reconcile", body: null });
  });

  it("sets up CI through gh, with every value on stdin", async () => {
    const before = ghCalls().length;
    const keysBefore = items().filter((i) => i.attributes["kind"] === "gpg-key").length;
    const notLoggedIn = await losos(["ci", "setup", "--edge", "https://elsewhere.example"], { input: `${CI_TOKEN}\n` });
    assert.equal(notLoggedIn.code, 1);
    assert.match(notLoggedIn.stderr, /losos login https:\/\/elsewhere\.example/);

    const result = await losos(["ci", "setup", "--edge", edge.url, "--repo", "someone/thing"], { input: `${CI_TOKEN}\n` });
    assert.equal(result.code, 0, result.stderr);
    assert.match(result.stdout, /BEGIN PGP PUBLIC KEY BLOCK/);
    assert.ok(!result.stdout.includes("PRIVATE KEY"), "the secret half is not printed");
    assert.ok(!result.stdout.includes(CI_TOKEN), "the token is not printed");
    assert.match(result.stdout, /losos\.edge\.access\.gpgKeys/);
    assert.match(result.stdout, /losos-registrar provision token create --edge \S+ --name ci --key [0-9A-F]{40} --scope full/);

    const calls = ghCalls().slice(before);
    assert.deepEqual(
      calls.map((c) => c.argv),
      [
        ["secret", "set", "LOSOS_CI_GPG_KEY", "--repo", "someone/thing"],
        ["secret", "set", "LOSOS_EDGE_TOKEN", "--repo", "someone/thing"],
        ["variable", "set", "LOSOS_EDGE_URL", "--repo", "someone/thing"],
        ["variable", "set", "LOSOS_EDGE_HOST_KEY", "--repo", "someone/thing"],
      ],
    );
    for (const c of calls) {
      assert.ok(!c.argv.some((a) => a.includes("BEGIN PGP") || a.includes(CI_TOKEN) || a.startsWith("http")), `no value in argv: ${c.argv.join(" ")}`);
    }
    const [gpgKey, token, url, hostKey] = calls.map((c) => c.stdin);
    assert.match(gpgKey!, /BEGIN PGP PRIVATE KEY BLOCK/);
    assert.equal(token, CI_TOKEN);
    assert.equal(url, edge.url);
    assert.equal((await openpgp.readKey({ armoredKey: hostKey! })).getFingerprint().toUpperCase(), edge.hostFingerprint);

    // A new key, not the owner's, with the user id the contract names, kept
    // in the keychain like any imported key.
    const ci = await openpgp.readPrivateKey({ armoredKey: gpgKey! });
    const ciFpr = ci.getFingerprint().toUpperCase();
    assert.notEqual(ciFpr, ownerFpr);
    assert.equal(ci.isDecrypted(), true);
    assert.deepEqual(ci.getUserIDs(), ["LosOS CI <someone/thing>"]);
    assert.ok(result.stdout.includes(group(ciFpr)));
    const keys = items().filter((i) => i.attributes["kind"] === "gpg-key");
    assert.equal(keys.length, keysBefore + 1);
    assert.ok(keys.some((k) => k.attributes["fingerprint"] === ciFpr));

    // CI mode: the four values from the environment, the keychain untouched.
    await edge.seed(ci.toPublic().armor());
    edge.addToken({ token: CI_TOKEN, fingerprint: ciFpr, scope: "full", expires: null });
    const ciEnv = {
      LOSOS_CI: "1",
      LOSOS_CI_GPG_KEY: gpgKey,
      LOSOS_EDGE_TOKEN: token,
      LOSOS_EDGE_URL: url,
      LOSOS_EDGE_HOST_KEY: hostKey,
      FAKE_SECRET_DOWN: "1",
    };
    const logged = secretCalls();
    const list = await losos(["edge", "secret", "list"], { env: ciEnv });
    assert.equal(list.code, 0, list.stderr);
    assert.match(list.stdout, /stripe-webhook-secret/);
    const set = await losos(["edge", "secret", "set", "claude-api-key"], { env: ciEnv, input: "sk-ant-xyz\n" });
    assert.equal(set.code, 0, set.stderr);
    assert.deepEqual(edge.calls.at(-1), { method: "POST", path: "/secrets", body: { name: "claude-api-key", value: "sk-ant-xyz" } });
    const reconcile = await losos(["edge", "reconcile"], { env: ciEnv });
    assert.equal(reconcile.code, 0, reconcile.stderr);
    const called = await losos(["call", "GET", "/overview"], { env: ciEnv });
    assert.equal(called.code, 0, called.stderr);
    assert.equal(secretCalls(), logged, "CI mode never ran secret-tool");

    const missing = await losos(["edge", "reconcile"], { env: { ...ciEnv, LOSOS_EDGE_TOKEN: undefined } });
    assert.equal(missing.code, 1);
    assert.match(missing.stderr, /LOSOS_EDGE_TOKEN/);

    // Outside LOSOS_CI=1 the four variables mean nothing: the keychain is asked.
    const ignored = await losos(["edge", "reconcile"], { env: { ...ciEnv, LOSOS_CI: undefined } });
    assert.equal(ignored.code, 1);
    assert.match(ignored.stderr, /Secret Service/);
  });

  it("prints the four names and no secret when gh is signed out", async () => {
    const before = ghCalls().length;
    const quiet = await losos(["ci", "setup", "--edge", edge.url], { input: `${CI_TOKEN}\n`, env: { FAKE_GH_SIGNED_OUT: "1" } });
    assert.equal(quiet.code, 0, quiet.stderr);
    for (const name of ["LOSOS_CI_GPG_KEY", "LOSOS_EDGE_TOKEN", "LOSOS_EDGE_URL", "LOSOS_EDGE_HOST_KEY"]) assert.ok(quiet.stdout.includes(name));
    assert.match(quiet.stdout, /dasmatus\/losos/);
    assert.ok(!quiet.stdout.includes("PRIVATE KEY"));
    assert.ok(!quiet.stdout.includes(CI_TOKEN));
    assert.match(quiet.stdout, /--print/);

    const printed = await losos(["ci", "setup", "--edge", edge.url, "--print"], { input: `${CI_TOKEN}\n`, env: { FAKE_GH_SIGNED_OUT: "1" } });
    assert.equal(printed.code, 0, printed.stderr);
    assert.match(printed.stdout, /LOSOS_CI_GPG_KEY:\n-----BEGIN PGP PRIVATE KEY BLOCK/);
    assert.ok(printed.stdout.includes(`LOSOS_EDGE_TOKEN:\n${CI_TOKEN}`));
    assert.equal(ghCalls().length, before, "a signed-out gh set nothing");

    const noGh = await losos(["ci", "setup", "--edge", edge.url], { input: `${CI_TOKEN}\n`, env: { LOSOS_GH: join(dir, "no-gh") } });
    assert.equal(noGh.code, 0, noGh.stderr);
    assert.ok(noGh.stdout.includes("LOSOS_EDGE_HOST_KEY"));
  });

  it("logs out and forgets a key", async () => {
    const out = await losos(["logout", box.url]);
    assert.equal(out.code, 0, out.stderr);
    assert.equal(items().some((i) => i.attributes["kind"] === "host" && i.attributes["url"] === box.url), false);
    const again = await losos(["logout", box.url]);
    assert.equal(again.code, 1);
    const removed = await losos(["key", "remove", ownerFpr]);
    assert.equal(removed.code, 0, removed.stderr);
    assert.ok(removed.stdout.includes(edge.url), "names the host that still points at it");
    const unknown = await losos(["frobnicate"]);
    assert.equal(unknown.code, 1);
  });
});
