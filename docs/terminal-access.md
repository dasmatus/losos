# Terminal access: the wire contract

`losos`, the terminal client in `admin-ui/tui/`, manages a box (and, for the
operator, an edge) without SSH. It talks to the same HTTP API the web UI
does, through one signed and encrypted endpoint. This page is the contract
between the three sides: the client, `lososd` (`backend/src/access.rs`) and
the shared Rust crate both servers use (`backend/access/`, `losos-access`).

## Two credentials, both needed

- **A personal access token.** Made and revoked in the web UI
  (Settings, Access tokens). The web UI shows it once. The box keeps only
  its SHA-256. A token carries its owner, the person who was signed in when
  it was made, and every call with it runs as that person.
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
sha256, owner}` in `/var/lib/losos/access/tokens.json` (mode 0600, under `/var`, so
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
200 {"fingerprint": "<40 hex, upper case>", "publicKey": "-----BEGIN PGP PUBLIC KEY BLOCK-----…", "edgeUrl": "https://<name>.<edge zone>" | null}
```

`edgeUrl` is where the client reaches the box away from its LAN (below),
and `null` when the box has no edge or keeps the terminal on its LAN.

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
   `/api/access` or `/api/setup` (403). A token can never make, list or
   revoke tokens;
6. runs the request against its own loopback API as the web UI would, with
   the admin token and, for a token with an owner, `X-Losos-As: <name>`, and
   seals the answer. lososd honours `X-Losos-As` only together with the
   admin token, and it makes the call run as that person, so their sections
   (`backend/sections.json`) apply as they do in the browser. A token with
   no owner, made before tokens carried one or made with the spare key, acts
   as an administrator. A token whose owner is no longer on the box gets
   401. A `body` that is a JSON string goes as raw `text/plain`, which is
   how `/api/apply` takes its Nix; any other body goes as JSON.

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

All three are in the `access` section: an administrator, a person in
`losos-access`, or the spare key. A person who is not an administrator sees
and revokes only their own tokens (revoking someone else's answers 404); a
new token's owner is the person who made it, and none for the spare key.

```
GET  /api/access
200 {"hostKey": {"fingerprint"}, "keys": [{"fingerprint", "userIds", "created"}],
     "tokens": [{"id", "name", "fingerprint", "scope", "created", "expires", "lastUsed", "owner"?}]}

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
| `host` | `url` | JSON: `{"url", "kind": "box"\|"edge", "fingerprint", "publicKey", "token", "keyFingerprint", "tlsPin"?, "edgeUrl"?}` |
| `cloud` | `url` | JSON: `{"url", "user", "secret", "location"}`, the LosOS Cloud app password of `losos cloud setup` |
| `git` | `url`, `protocol`, `server` | the same, with the LosOS Git token of `losos git setup`; `server` is the host as git's credential protocol names it |
| `companion` | `url` | JSON: `{"url", "tlsPin"?, "accessToken", "expiresAt", "refreshToken"}`, the AI agent companion's LosOS Login sign-in (below) |

`losos cloud setup` also stores the app password under GVfs's own schema,
so the file manager finds it before it would ask: `xdg:schema`
`org.gnome.keyring.NetworkPassword`, `user`, `server`, `protocol` `http`,
`object` the realm of the WebDAV challenge, `authtype` `basic`, and `port`
with the scheme's default filled in (GVfs looks up 443 for a `davs://`
address without one).

When no Secret Service answers, `losos` says so and stores nothing.

## Through the edge

All of `/api/` is LAN-only (`lanOnly`, modules/containers.nix), except
these two routes while `losos.access.throughEdge` (on by default) and
`losos.proxy.enable` are both set: `= /api/access/host-key` and
`= /api/access/call` take the tunnel's connections too, and Tor's and
I2P's, which also arrive on loopback. Nothing else changes for them: the
envelope is checked as it is on the LAN, and an edge in the middle sees
only the armoured message. lososd names the address, `https://` and
`losos.proxy.hostname`, as `edgeUrl` (`LOSOS_ACCESS_EDGE_URL`), and the
client keeps it in the host record (`edgeUrl`, above) at `login`.

The client tries the box's own URL first. Only when that one cannot be
connected to at all (the name does not resolve, or nothing takes the
connection: `HttpError::NoConnection`) does it seal the call again, with a
new nonce, and send it to `edgeUrl`, taking only a certificate the web
trusts there. A timeout or a changed certificate may have reached the box,
so neither is retried. Every call through the tunnel comes from 127.0.0.1
as far as lososd can tell, so they share one throttle.

The companion (`losos mcp`) and `losos cloud`/`git setup` reach the box's
API directly and stay on its LAN.

## The TLS certificate

A box answers HTTPS with its own self-signed certificate, which no public
authority vouches for. The envelope already protects every call, but the
client still checks the transport. At `losos login` it accepts the
certificate it is shown, and when no public authority vouches for it, keeps
its SHA-256 (of the DER) as `tlsPin` in the host record. Every later call
accepts that exact certificate or one the web trusts, and nothing else. A
different certificate stops the call with a warning, and `losos login` again
pins the new one once the owner has checked it. An edge with a public
certificate gets no pin.

## The AI agent companion

`losos agents install` puts `losos mcp --host <url>` into the person's AI
tools (handbook: AI tools on your computer). The companion uses no key and
no access token of the person's. It signs in to LosOS Login with OAuth 2.0
device authorization (RFC 8628) as the public client `losos-companion`
(`modules/identity.nix`): a POST to `/auth/api/oidc/device-authorization`
gives a link with the code in it, the person approves on LosOS Login's
consent page after the usual sign-in (password, and the second factor of a
person in `losos-two-factor`; `losos-locked` is denied), and the client
polls `/auth/api/oidc/token` for an access token (an hour) and a refresh
token (30 days from its last use, rotated at every refresh). Both go into
the `companion` keychain item and nowhere else; no AI tool's settings hold
them.

`losos mcp` sends each read as `GET` with `Authorization: Bearer
<access token>`, straight to the box's API over the pinned certificate (no
envelope). nginx lets a Bearer request through its authz location, and
lososd takes a token that is not the spare key only from loopback, only
under LosOS Login, and only in its `authelia_at_` shape: it asks LosOS Login
about it (`/auth/api/oidc/introspection` as the confidential client
`lososd`, whose secret the identity setup unit makes; the request's own
host, because the issuer is per host) and keeps the answer for at most a
minute and never past the token's expiry. A live token issued to
`losos-companion` is the person it names, as the principal `Companion`,
which is never an administrator and reaches only `GET` on
`losos_access::COMPANION_ROUTES` (`/api/me`, `/api/status`, the two cloud
and the two Git routes, and `/api/mail`, `/api/mail/messages` and
`/api/mail/messages/{id}`, where `{id}` is one segment of letters, digits,
`-` and `_`); everything else is 403. `/api/me` says `"companion": true`. A
message the companion opens is not marked read.

A refresh is made under a lock file in the companion's folder, after reading
the keychain again, because LosOS Login ends the whole sign-in when a used
refresh token is shown twice and several tools may each run a `losos mcp`.
A 401 refreshes once and retries. `losos agents remove`, once the companion
is in no tool, revokes both tokens at `/auth/api/oidc/revocation` and
forgets the item.

On a computer that never ran `losos login`, `--host` names the box and the
client pins the certificate it is shown after the person compares its
SHA-256 with the browser's. Otherwise the host record's pin is used.

The AI tools' own MCP sign-in (OAuth for remote MCP servers) does not fit:
the companion is a local stdio server, and the tools' OAuth is for HTTP
servers that trust a public certificate.

## This computer: LosOS Cloud and LosOS Git

`losos cloud setup` and `losos git setup` ask the box for a credential of
the computer's own over a sealed call:

```text
POST /api/devices/cloud  {"password", "device"}  -> {"user", "appPassword", "path"}
POST /api/devices/git    {"password", "device"}  -> {"user", "token", "path"}
```

The owner's password goes along because nothing less makes either one.
LosOS Cloud mints an app password only once lososd has checked the owner's
password against the people file (`backend/src/devices.rs`). LosOS Git
keeps no passwords either: after the same check lososd asks LosOS Git as
its administrator for a token (`POST /api/v1/admin/users/<owner>/tokens`).
A token named `LosOS on <device>` from an earlier setup is replaced, so
setting up the same computer twice leaves one token. Its scopes are
`write:repository` and
`read:user`.

A wrong password is 403 with `wrongPassword: true`, not 401, so a client
does not mistake it for a bad token. 409 with `gitOff: true` means the box
runs no LosOS Git, 429 that LosOS Cloud is throttling, 503 that one of the
two cannot be asked yet.

LosOS Desktop's backups ask for both at once without a password, through
LosOS Login's device code sign-in as the client `losos-backups`:

```text
POST /api/devices/backups  {"device"}  -> {"cloud": {...}|null, "git": {...}|null, "gitOff"?, "problems"?}
```

LosOS Login has asked the person for their password, their second factor
and their consent by then, so the route takes none. A `losos-backups` token
lasts ten minutes, has no refresh token and reaches this route alone. It
makes the credentials of the person it names, in their own accounts.

On the computer, `git setup` writes git's global configuration for
`<box>/git`: an empty `credential.<url>.helper` and then `!<losos> git
credential`, so only `losos` answers for the box, and
`credential.<url>.username`. `losos git credential get` reads git's
`protocol` and `host` and answers from the `kind=git` item; `store` and
`erase` are accepted and ignored. A box with a pinned certificate also gets
`http.<url>.sslVerify false` and `http.<url>.pinnedPubkey
sha256//<base64>` of the certificate's public key, read over the pinned
connection, which is libcurl's form of the same pin. `cloud setup` adds
`<davs location> LosOS Cloud` to `$XDG_CONFIG_HOME/gtk-3.0/bookmarks` once.

