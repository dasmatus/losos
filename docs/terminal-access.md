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
| `host` | `url` | JSON: `{"url", "kind": "box"\|"edge", "fingerprint", "publicKey", "token", "keyFingerprint", "tlsPin"?}` |
| `cloud` | `url` | JSON: `{"url", "user", "secret", "location"}`, the LosOS cloud app password of `losos cloud setup` |
| `git` | `url`, `protocol`, `server` | the same, with the LosOS Git token of `losos git setup`; `server` is the host as git's credential protocol names it |

`losos cloud setup` also stores the app password under GVfs's own schema,
so the file manager finds it before it would ask: `xdg:schema`
`org.gnome.keyring.NetworkPassword`, `user`, `server`, `protocol` `http`,
`object` the realm of the WebDAV challenge, `authtype` `basic`, and `port`
with the scheme's default filled in (GVfs looks up 443 for a `davs://`
address without one).

When no Secret Service answers, `losos` says so and stores nothing.

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

## This computer: LosOS cloud and LosOS Git

`losos cloud setup` and `losos git setup` ask the box for a credential of
the computer's own over a sealed call:

```text
POST /api/devices/cloud  {"password", "device"}  -> {"user", "appPassword", "path"}
POST /api/devices/git    {"password", "device"}  -> {"user", "token", "path"}
```

The owner's password goes along because nothing less makes either one.
Nextcloud mints an app password only for the real password, and Forgejo
creates an access token only under Basic auth with the account's password;
its API refuses an administrator's token, `Sudo` and a token used as a
password for that route. lososd sends both requests over loopback with the
password in a Basic header, never in an argv or a file, as the sign-in
does (`backend/src/devices.rs`). For Git, LosOS cloud judges the password
first and LosOS Git is given it before Forgejo is asked. A token named
`LosOS on <device>` from an earlier setup is deleted first, since Forgejo
refuses a second token of one name. Its scopes are `write:repository` and
`read:user`.

A wrong password is 403 with `wrongPassword: true`, not 401, so a client
does not mistake it for a bad token. 409 with `gitOff: true` means the box
runs no LosOS Git, 429 that Nextcloud is throttling, 503 that one of the
two cannot be asked yet.

On the computer, `git setup` writes git's global configuration for
`<box>/forgejo`: an empty `credential.<url>.helper` and then `!<losos> git
credential`, so only `losos` answers for the box, and
`credential.<url>.username`. `losos git credential get` reads git's
`protocol` and `host` and answers from the `kind=git` item; `store` and
`erase` are accepted and ignored. A box with a pinned certificate also gets
`http.<url>.sslVerify false` and `http.<url>.pinnedPubkey
sha256//<base64>` of the certificate's public key, read over the pinned
connection, which is libcurl's form of the same pin. `cloud setup` adds
`<davs location> LosOS cloud` to `$XDG_CONFIG_HOME/gtk-3.0/bookmarks` once.

