import assert from "node:assert/strict";
import { after, before, describe, it } from "node:test";
import { call, EnvelopeError, seal, type Session } from "../src/envelope";
import { readHostKey, readUserKey, unlock } from "../src/pgp";
import { generateLegacyKey, startMock, type Mock } from "./mock-server";

const FULL = `losos_pat_0a1b2c3d_${"a".repeat(40)}`;
const READ = `losos_pat_1a1b2c3d_${"b".repeat(40)}`;
const OTHER = `losos_pat_2a1b2c3d_${"c".repeat(40)}`;

describe("the envelope", () => {
  let mock: Mock;
  let owner: Awaited<ReturnType<typeof generateLegacyKey>>;
  let stranger: Awaited<ReturnType<typeof generateLegacyKey>>;
  let second: Awaited<ReturnType<typeof generateLegacyKey>>;
  let session: (token?: string, key?: string) => Promise<Session>;

  before(async () => {
    owner = await generateLegacyKey("Owner");
    stranger = await generateLegacyKey("Stranger");
    second = await generateLegacyKey("Second");
    const ownerFpr = (await readUserKey(owner.privateKey)).getFingerprint().toUpperCase();
    const secondFpr = (await readUserKey(second.privateKey)).getFingerprint().toUpperCase();
    mock = await startMock({
      kind: "box",
      seeded: [owner.publicKey, second.publicKey],
      tokens: [
        { token: FULL, fingerprint: ownerFpr, scope: "full", expires: null },
        { token: READ, fingerprint: ownerFpr, scope: "read", expires: null },
        { token: OTHER, fingerprint: secondFpr, scope: "full", expires: null },
      ],
    });
    session = async (token = FULL, key = owner.privateKey) => ({
      url: mock.url,
      kind: "box",
      hostKey: await readHostKey(mock.hostPublicKey),
      hostFingerprint: mock.hostFingerprint,
      token,
      userKey: await readUserKey(key),
    });
  });
  after(() => mock.close());

  it("round-trips a GET and checks the answer", async () => {
    const answer = await call(await session(), "GET", "/api/state");
    assert.equal(answer.sealed, true);
    assert.equal(answer.status, 200);
    assert.deepEqual(answer.body, { mode: "local", sharing: false });
  });

  it("carries the plaintext the contract names", async () => {
    const { plaintext } = await seal(await session(), "post", "/api/grow", null);
    assert.deepEqual(Object.keys(plaintext).sort(), ["body", "host", "method", "nonce", "path", "token", "ts", "v"]);
    assert.equal(plaintext.v, 1);
    assert.equal(plaintext.method, "POST");
    assert.match(plaintext.nonce, /^[0-9a-f]{32}$/);
    assert.equal(plaintext.host, mock.hostFingerprint);
  });

  it("passes the inner status through, sealed", async () => {
    const answer = await call(await session(), "GET", "/api/nothing-here");
    assert.equal(answer.sealed, true);
    assert.equal(answer.status, 404);
  });

  it("is refused when signed by a key that was not seeded", async () => {
    const answer = await call(await session(FULL, stranger.privateKey), "GET", "/api/state");
    assert.equal(answer.sealed, false);
    assert.equal(answer.status, 401);
  });

  it("is refused when the token is bound to another seeded key", async () => {
    const answer = await call(await session(OTHER, owner.privateKey), "GET", "/api/state");
    assert.equal(answer.status, 401);
  });

  it("is refused when stale", async () => {
    const answer = await call(await session(), "GET", "/api/state", undefined, { overrides: { ts: Math.floor(Date.now() / 1000) - 301 } });
    assert.equal(answer.status, 401);
    assert.equal(answer.sealed, false);
  });

  it("is refused when its nonce was seen before", async () => {
    const s = await session();
    const { armored } = await seal(s, "GET", "/api/state", null);
    const send = () =>
      fetch(`${mock.url}/api/access/call`, { method: "POST", headers: { "Content-Type": "application/pgp-encrypted" }, body: armored });
    assert.equal((await send()).status, 200);
    const replay = await send();
    assert.equal(replay.status, 409);
    assert.deepEqual(await replay.json(), { error: "nonce seen" });
  });

  it("refuses a POST with a read token", async () => {
    const read = await call(await session(READ), "GET", "/api/state");
    assert.equal(read.status, 200);
    const write = await call(await session(READ), "POST", "/api/grow");
    assert.equal(write.status, 403);
  });

  it("refuses the token routes", async () => {
    const answer = await call(await session(), "GET", "/api/access");
    assert.equal(answer.status, 403);
  });

  it("rejects an answer whose nonce is not the request's", async () => {
    mock.misbehave = "nonce";
    try {
      await assert.rejects(call(await session(), "GET", "/api/state"), (error: unknown) => {
        assert.ok(error instanceof EnvelopeError);
        assert.match((error as Error).message, /another request/);
        return true;
      });
    } finally {
      mock.misbehave = null;
    }
  });

  it("rejects an answer signed by a key that is not the pinned one", async () => {
    mock.misbehave = "signer";
    try {
      await assert.rejects(call(await session(), "GET", "/api/state"), (error: unknown) => {
        assert.ok(error instanceof EnvelopeError);
        assert.match((error as Error).message, /not signed by the host key/);
        return true;
      });
    } finally {
      mock.misbehave = null;
    }
  });

  it("sends a string body as a string (the Nix of /api/apply)", async () => {
    const answer = await call(await session(), "POST", "/api/apply", "{ ... }:\n{\n  losos.hostName = \"x\";\n}\n");
    assert.equal(answer.status, 200);
    assert.equal(typeof mock.calls.at(-1)?.body, "string");
  });

  it("unlocks a key under a passphrase", async () => {
    const locked = await generateLegacyKey("Locked", "correct horse");
    const key = await readUserKey(locked.privateKey);
    await assert.rejects(unlock(key, "wrong"));
    const open = await unlock(key, "correct horse");
    assert.equal(open.isDecrypted(), true);
  });

  it("refuses a public key and a key without an encryption subkey", async () => {
    await assert.rejects(readUserKey(owner.publicKey), /public key/);
    const { privateKey } = await (await import("openpgp")).generateKey({
      type: "ecc",
      curve: "curve25519Legacy",
      userIDs: [{ name: "Sign only" }],
      subkeys: [],
      format: "armored",
    });
    await assert.rejects(readUserKey(privateKey), /encryption subkey/);
  });
});
