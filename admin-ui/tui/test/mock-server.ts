/* A box or an edge that speaks the envelope contract (docs/terminal-access.md),
 * for the tests. Its own host key, its seeded keys, its tokens, and the
 * server's checks in the contract's order:
 *
 *   1. decrypt with the host key                         400
 *   2. a signature by a seeded key                       401
 *   3. host is ours (400), ts within 300 s (401), nonce unseen for 600 s (409)
 *   4. token known, unexpired, bound to the signer (401), scope allows (403)
 *   5. path under /api/, no "..", not /api/access, /api/setup, /api/sign-in (403)
 *   6. run it and seal the answer to the signer
 *
 * `misbehave` makes it answer wrongly on purpose, so the client's own checks
 * can be tested: a nonce that is not the request's, or an answer signed by a
 * key that is not the host key. */

import { createServer, type IncomingMessage, type Server } from "node:http";
import type { AddressInfo } from "node:net";
import * as openpgp from "openpgp";
import { boxRoute, edgeRoute } from "./fixtures";

export interface MockToken {
  token: string;
  fingerprint: string;
  scope: "read" | "full";
  expires: number | null;
}

export type Misbehaviour = "nonce" | "signer" | null;

export interface Mock {
  url: string;
  kind: "box" | "edge";
  hostPublicKey: string;
  hostFingerprint: string;
  /** Every inner call the mock ran, in order. */
  calls: { method: string; path: string; body: unknown }[];
  misbehave: Misbehaviour;
  /** Seed another public key, as a new install-target.nix would. */
  seed: (armoredPublicKey: string) => Promise<void>;
  addToken: (token: MockToken) => void;
  close: () => Promise<void>;
}

export interface MockOptions {
  kind: "box" | "edge";
  seeded: string[];
  tokens: MockToken[];
  route?: (method: string, path: string, body: unknown) => { status: number; body: unknown };
}

async function readBody(req: IncomingMessage): Promise<string> {
  const chunks: Buffer[] = [];
  for await (const chunk of req) chunks.push(chunk as Buffer);
  return Buffer.concat(chunks).toString("utf8");
}

export async function generateLegacyKey(name: string, passphrase?: string) {
  return openpgp.generateKey({
    type: "ecc",
    curve: "curve25519Legacy",
    userIDs: [{ name, email: `${name.toLowerCase().replace(/[^a-z]/g, "")}@example.org` }],
    format: "armored",
    ...(passphrase === undefined ? {} : { passphrase }),
  });
}

export async function startMock(options: MockOptions): Promise<Mock> {
  const host = await generateLegacyKey("Host");
  const hostKey = await openpgp.readPrivateKey({ armoredKey: host.privateKey });
  const hostFingerprint = hostKey.getFingerprint().toUpperCase();
  const impostor = await openpgp.readPrivateKey({ armoredKey: (await generateLegacyKey("Impostor")).privateKey });
  const seeded = await Promise.all(options.seeded.map((armoredKey) => openpgp.readKey({ armoredKey })));
  const seen = new Map<string, number>();
  const route = options.route ?? (options.kind === "edge" ? edgeRoute : boxRoute);
  const keyPath = options.kind === "edge" ? "/operator/host-key" : "/api/access/host-key";
  const callPath = options.kind === "edge" ? "/operator/call" : "/api/access/call";

  const mock: Mock = {
    url: "",
    kind: options.kind,
    hostPublicKey: host.publicKey,
    hostFingerprint,
    calls: [],
    misbehave: null,
    seed: async (armoredKey) => {
      seeded.push(await openpgp.readKey({ armoredKey }));
    },
    addToken: (token) => {
      options.tokens.push(token);
    },
    close: async () => {
      server.closeAllConnections();
      await new Promise<void>((resolve) => server.close(() => resolve()));
    },
  };

  const plain = (status: number, error: string) => ({ status, type: "application/json", body: JSON.stringify({ error }) });

  async function handle(raw: string): Promise<{ status: number; type: string; body: string }> {
    // 1. decrypt
    let data: string;
    let signatures: Awaited<ReturnType<typeof openpgp.decrypt>>["signatures"];
    try {
      const message = await openpgp.readMessage({ armoredMessage: raw });
      const result = await openpgp.decrypt({ message, decryptionKeys: hostKey, verificationKeys: seeded, format: "utf8" });
      data = result.data as string;
      signatures = result.signatures;
    } catch {
      return plain(400, "not a message for this host");
    }
    // 2. signed by a seeded key
    let signer: openpgp.Key | undefined;
    for (const sig of signatures) {
      try {
        await sig.verified;
        signer = seeded.find((k) => k.getKeys(sig.keyID).length > 0);
        if (signer !== undefined) break;
      } catch {
        /* try the next */
      }
    }
    if (signer === undefined) return plain(401, "not signed by a seeded key");
    const signerFingerprint = signer.getFingerprint().toUpperCase();
    // 3. host, ts, nonce
    let env: { v: number; host: string; token: string; method: string; path: string; ts: number; nonce: string; body: unknown };
    try {
      env = JSON.parse(data);
    } catch {
      return plain(400, "not JSON");
    }
    if (env.v !== 1 || env.host !== hostFingerprint) return plain(400, "not for this host");
    const now = Math.floor(Date.now() / 1000);
    if (typeof env.ts !== "number" || Math.abs(now - env.ts) > 300) return plain(401, "stale");
    for (const [nonce, at] of seen) if (now - at > 600) seen.delete(nonce);
    if (typeof env.nonce !== "string" || !/^[0-9a-f]{32}$/.test(env.nonce)) return plain(400, "bad nonce");
    if (seen.has(env.nonce)) return plain(409, "nonce seen");
    seen.set(env.nonce, now);
    // 4. token
    const token = options.tokens.find((t) => t.token === env.token);
    if (token === undefined || (token.expires !== null && token.expires < now) || token.fingerprint !== signerFingerprint) {
      return plain(401, "token");
    }
    if (token.scope === "read" && env.method !== "GET") return plain(403, "read-only token");
    // 5. path
    if (options.kind === "box") {
      const p = env.path;
      if (!p.startsWith("/api/") || p.includes("..") || /^\/api\/(access|setup|sign-in)(\/|$|\?)/.test(p)) {
        return plain(403, "path not allowed");
      }
    }
    // 6. run and seal
    mock.calls.push({ method: env.method, path: env.path, body: env.body });
    const inner = route(env.method, env.path, env.body);
    const nonce = mock.misbehave === "nonce" ? "0".repeat(32) : env.nonce;
    const sealed = await openpgp.encrypt({
      message: await openpgp.createMessage({ text: JSON.stringify({ v: 1, nonce, status: inner.status, body: inner.body }) }),
      encryptionKeys: signer,
      signingKeys: mock.misbehave === "signer" ? impostor : hostKey,
      format: "armored",
    });
    return { status: inner.status, type: "application/pgp-encrypted", body: sealed as string };
  }

  const server: Server = createServer((req, res) => {
    void (async () => {
      const path = (req.url ?? "/").split("?")[0];
      if (req.method === "GET" && path === keyPath) {
        res.writeHead(200, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ fingerprint: hostFingerprint, publicKey: host.publicKey }));
        return;
      }
      if (req.method === "POST" && path === callPath) {
        const raw = await readBody(req);
        const answer = await handle(raw);
        res.writeHead(answer.status, { "Content-Type": answer.type });
        res.end(answer.body);
        return;
      }
      res.writeHead(404, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ error: "not found" }));
    })();
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", () => resolve()));
  mock.url = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
  return mock;
}
