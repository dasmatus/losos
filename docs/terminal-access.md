# Terminal access: the wire contract

`losos`, the terminal client in `admin-ui/tui/`, manages a box (and, for the
operator, an edge) without SSH. It talks to the same HTTP API the web UI
does, through one signed and encrypted endpoint. This page is the contract
between the three sides: the client, `lososd` (`backend/src/access.rs`) and
the shared Rust crate both servers use (`backend/access/`, `losos-access`).

## Two credentials, both needed

- **A personal access token.** Made and revoked in the web UI
  (Settings, Access tokens). The web UI shows it once. The box keeps only
  its SHA-256.
- **A GPG key that was seeded before the box ran.** The public half is
  `losos.access.gpgKeys`, written into the box's `install-target.nix` by the
  installer (`losos-ctl install --gpg-key FILE`, or the ISO's own
  `losos.access.gpgKeys`). Nothing at runtime adds one: the web UI shows the
  seeded keys and cannot change them, the Advanced pane treats the option as
  fixed by the installer, and `POST /api/apply` refuses it.

Every token is bound to one seeded key. A request is honoured only when it
carries a live token **and** is signed by the key that token is bound to.
Someone who steals a web session can make a token, but cannot use it
without the owner's secret key; someone who steals the secret key has
nothing to use it with until they also have a token.

## Tokens

```
losos_pat_<id: 8 hex>_<secret: 40 hex>
```

The box stores `{id, name, fingerprint, scope, created, expires, lastUsed,
sha256}` in `/var/lib/losos/access/tokens.json` (mode 0600, under `/var`, so
it survives the nightly reboot). `scope` is `read` (GET only) or `full`.
`expires` is a Unix time or `null`. At most 32 tokens.

## Keys

Each box makes its own OpenPGP key on first start
(`/var/lib/losos/access/host-key.asc`, mode 0600): an Ed25519 primary that
signs and a Curve25519 subkey that decrypts. Its public half is served
without authentication, so the client can pin it the first time it
connects, the way SSH pins a host key:

```
GET /api/access/host-key
200 {"fingerprint": "<40 hex, upper case>", "publicKey": "-----BEGIN PGP PUBLIC KEY BLOCK-----…"}
```

The client's key must be able to sign and must have an encryption subkey,
which every key `gpg --full-generate-key` makes does.

## One call

```
POST /api/access/call
Content-Type: application/pgp-encrypted
<an ASCII-armoured OpenPGP message, signed by the client key, encrypted to the host key>
```

The message's literal data is UTF-8 JSON:

```json
{
  "v": 1,
  "host": "<host key fingerprint>",
  "token": "losos_pat_…",
  "method": "GET",
  "path": "/api/state",
  "ts": 1760000000,
  "nonce": "<32 hex>",
  "body": null
}
```

The server, in this order:

1. decrypts with the host key (400 if it is not a message for this host);
2. checks the signature against the seeded keys (401 if none made it);
3. checks `host` is its own fingerprint (400), `ts` is within 300 s of its
   clock (401), and `nonce` has not been seen in the last 600 s (409);
4. checks the token: known, not expired, bound to the key that signed, and
   its scope allows `method` (401, or 403 for scope);
5. checks the path: it starts with `/api/`, has no `..`, and is not under
   `/api/access`, `/api/setup` or `/api/sign-in` (403). A token can never
   make, list or revoke tokens;
6. runs the request against its own loopback API as the web UI would, and
   seals the answer. A `body` that is a JSON string goes as raw
   `text/plain`, which is how `/api/apply` takes its Nix; any other body
   goes as JSON.

Failures in steps 1 to 5 answer plain JSON, `{"error": "…"}`, with the
status above, because the server cannot yet encrypt to anyone it trusts.
Each one counts against the per-address throttle the Bearer routes use.

A sealed answer has the status of the inner call and
`Content-Type: application/pgp-encrypted`. It is signed by the host key and
encrypted to the client key that signed the request. Its literal data:

```json
{ "v": 1, "nonce": "<the request's nonce>", "status": 200, "body": { … } }
```

The client checks the signature against the pinned host key and that
`nonce` is the one it sent, so an answer cannot be replayed onto another
request. `body` is the inner answer's JSON, or a string when it was not
JSON.

## The web UI's side

All three need the admin Bearer token, like every other settings route.

```
GET  /api/access
200 {"hostKey": {"fingerprint"}, "keys": [{"fingerprint", "userIds", "created"}],
     "tokens": [{"id", "name", "fingerprint", "scope", "created", "expires", "lastUsed"}]}

POST /api/access/tokens   {"name", "fingerprint", "scope": "read"|"full", "expiresInDays": 30|90|365|null}
200 {"token": "losos_pat_…", "record": {…}}
400 {"error"}             a name that is empty or over 64 characters, a key that was not seeded, a bad scope
409 {"error"}             32 tokens already

POST /api/access/tokens/revoke   {"id"}
200 {"revoked": true}
404 {"error"}
```

## What the client keeps, and where

Nothing in a plain file. `losos` stores everything in the desktop's Secret
Service through libsecret (`secret-tool`), so GNOME Keyring, KWallet and
KeePassXC all work. Every item carries `service=losos` and a `kind`:

| kind | other attributes | secret |
|---|---|---|
| `gpg-key` | `fingerprint` | the armoured secret key, as exported (still under its passphrase if it had one) |
| `host` | `url` | JSON: `{"url", "kind": "box"\|"edge", "fingerprint", "publicKey", "token", "keyFingerprint"}` |

When no Secret Service answers, `losos` says so and stores nothing.
