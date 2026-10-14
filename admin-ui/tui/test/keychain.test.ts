import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, beforeEach, describe, it } from "node:test";
import { fileURLToPath } from "node:url";
import { KeychainUnavailable, MemoryKeychain, parseSearch, SecretToolKeychain, type Keychain } from "../src/keychain";
import { setLocale } from "../src/lang";
import { normalizeUrl, Vault, type HostRecord } from "../src/vault";

const FAKE = fileURLToPath(new URL("./fake-bin/secret-tool", import.meta.url));
const ARMOR = "-----BEGIN PGP PRIVATE KEY BLOCK-----\n\nxVgEZ0abc\n=AbCd\n-----END PGP PRIVATE KEY BLOCK-----\n";

describe("the keychain over a fake secret-tool", () => {
  let dir: string;
  let store: string;
  let log: string;
  const saved = { ...process.env };

  before(() => {
    setLocale("en");
    dir = mkdtempSync(join(tmpdir(), "losos-keychain-"));
  });
  after(() => {
    rmSync(dir, { recursive: true, force: true });
    process.env = saved;
  });
  beforeEach(() => {
    store = join(dir, `store-${Math.random().toString(16).slice(2)}.json`);
    log = join(dir, `log-${Math.random().toString(16).slice(2)}`);
    process.env["FAKE_SECRET_STORE"] = store;
    process.env["FAKE_SECRET_LOG"] = log;
    delete process.env["FAKE_SECRET_DOWN"];
  });

  it("stores, looks up, searches and clears", async () => {
    const kc = new SecretToolKeychain(FAKE);
    await kc.store({ service: "losos", kind: "gpg-key", fingerprint: "AA" }, "losos gpg-key A", ARMOR);
    await kc.store({ service: "losos", kind: "gpg-key", fingerprint: "BB" }, "losos gpg-key B", `${ARMOR}`);
    await kc.store({ service: "losos", kind: "host", url: "https://box.local" }, "losos box", '{"url":"https://box.local"}');
    assert.equal(await kc.lookup({ service: "losos", kind: "gpg-key", fingerprint: "AA" }), ARMOR);
    assert.equal(await kc.lookup({ service: "losos", kind: "gpg-key", fingerprint: "CC" }), null);
    const keys = await kc.search({ service: "losos", kind: "gpg-key" });
    assert.equal(keys.length, 2);
    for (const item of keys) assert.equal(item.secret.trimEnd(), ARMOR.trimEnd());
    assert.deepEqual(
      keys.map((k) => k.label),
      ["losos gpg-key A", "losos gpg-key B"],
    );
    await kc.clear({ service: "losos", kind: "gpg-key", fingerprint: "AA" });
    assert.equal((await kc.search({ service: "losos", kind: "gpg-key" })).length, 1);
    assert.deepEqual(await kc.search({ service: "losos", kind: "nothing" }), []);
    // The secret travels on stdin, never in argv.
    const calls = readFileSync(log, "utf8").trim().split("\n").map((l) => JSON.parse(l) as string[]);
    assert.ok(calls.every((argv) => !argv.some((a) => a.includes("BEGIN PGP"))));
    assert.ok(calls.some((argv) => argv[0] === "search" && argv.includes("--all")));
  });

  it("replaces an item stored again under the same attributes", async () => {
    const kc = new SecretToolKeychain(FAKE);
    const attrs = { service: "losos", kind: "host", url: "https://box.local" };
    await kc.store(attrs, "one", "1");
    await kc.store(attrs, "two", "2");
    assert.equal(await kc.lookup(attrs), "2");
    assert.equal((await kc.search({ service: "losos" })).length, 1);
  });

  it("says so in a sentence when no Secret Service answers", async () => {
    process.env["FAKE_SECRET_DOWN"] = "1";
    const kc = new SecretToolKeychain(FAKE);
    await assert.rejects(kc.probe(), (error: unknown) => {
      assert.ok(error instanceof KeychainUnavailable);
      assert.match((error as Error).message, /Secret Service/);
      return true;
    });
    await assert.rejects(kc.store({ service: "losos", kind: "probe" }, "x", "y"), KeychainUnavailable);
  });

  it("says so in a sentence when secret-tool is not installed", async () => {
    const kc = new SecretToolKeychain(join(dir, "no-such-secret-tool"));
    await assert.rejects(kc.probe(), (error: unknown) => {
      assert.ok(error instanceof KeychainUnavailable);
      assert.match((error as Error).message, /secret-tool/);
      return true;
    });
  });
});

describe("parseSearch", () => {
  it("reads secrets that span lines and items one after another", () => {
    const out = [
      "[/org/freedesktop/secrets/collection/login/1]",
      "label = one",
      "secret = line 1",
      "line 2",
      "",
      "created = 2026-10-14 09:00:00",
      "modified = 2026-10-14 09:00:00",
      "[/org/freedesktop/secrets/collection/login/2]",
      "label = two",
      "secret = {\"a\":1}",
      "created = 2026-10-14 09:00:00",
      "modified = 2026-10-14 09:00:00",
      "",
    ].join("\n");
    assert.deepEqual(parseSearch(out), [
      { label: "one", secret: "line 1\nline 2\n" },
      { label: "two", secret: '{"a":1}' },
    ]);
  });
});

describe("the vault", () => {
  const record = (url: string, kind: "box" | "edge" = "box"): HostRecord => ({
    url,
    kind,
    fingerprint: "A".repeat(40),
    publicKey: "-----BEGIN PGP PUBLIC KEY BLOCK-----",
    token: `losos_pat_0a1b2c3d_${"a".repeat(40)}`,
    keyFingerprint: "B".repeat(40),
  });

  for (const [name, make] of [
    ["in memory", () => new MemoryKeychain()],
    ["over secret-tool", () => new SecretToolKeychain(FAKE)],
  ] as [string, () => Keychain][]) {
    it(`keeps keys and hosts apart (${name})`, async () => {
      const dir = mkdtempSync(join(tmpdir(), "losos-vault-"));
      process.env["FAKE_SECRET_STORE"] = join(dir, "store.json");
      delete process.env["FAKE_SECRET_DOWN"];
      delete process.env["FAKE_SECRET_LOG"];
      try {
        const vault = new Vault(make());
        await vault.putKey("B".repeat(40), ARMOR, "losos gpg-key B");
        await vault.putHost(record("https://zeta.local"));
        await vault.putHost(record("https://alpha.local", "edge"));
        assert.deepEqual(await vault.listKeys(), [ARMOR]);
        assert.deepEqual(
          (await vault.listHosts()).map((h) => h.url),
          ["https://alpha.local", "https://zeta.local"],
        );
        assert.equal((await vault.getHost("https://alpha.local"))?.kind, "edge");
        assert.equal(await vault.getHost("https://nowhere.local"), null);
        await vault.removeHost("https://alpha.local");
        assert.equal((await vault.listHosts()).length, 1);
        await vault.removeKey("B".repeat(40));
        assert.deepEqual(await vault.listKeys(), []);
      } finally {
        rmSync(dir, { recursive: true, force: true });
      }
    });
  }

  it("spells a URL one way", () => {
    assert.equal(normalizeUrl("Mattbox.local/"), "https://mattbox.local");
    assert.equal(normalizeUrl("http://192.168.1.20:8080//"), "http://192.168.1.20:8080");
    assert.throws(() => normalizeUrl("ftp://box"));
    assert.throws(() => normalizeUrl("https://me:pw@box"));
  });
});
