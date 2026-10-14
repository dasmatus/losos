# losos, the terminal client

`losos` manages a LosOS box or edge from a terminal, without SSH. It has
the web UI's panes, words, colours and salmon, and it talks to the box
through one sealed envelope per request: signed by your OpenPGP key,
encrypted to the box's host key, and answered the same way. The wire
contract is in [docs/terminal-access.md](../../docs/terminal-access.md) and,
for an edge, [docs/edge-terminal.md](../../docs/edge-terminal.md).

## Install

On a 64-bit x86 Linux computer:

```sh
curl -fsSL https://proxy.losos.dasmat.us/updates/main/x86_64/install.sh | sh
```

The script downloads `losos` and `SHA256SUMS`, checks one against the
other, and puts the program in `~/.local/bin`, or in `/usr/local/bin` when
run as root. It never runs sudo. `LOSOS_CHANNEL` picks another channel. Read
[install.sh](install.sh) before piping it anywhere. It is short.

`losos` keeps your keys and tokens in the desktop's keychain through
`secret-tool`. The package is `libsecret-tools` on Debian and Ubuntu and
`libsecret` on Fedora and Arch. GNOME Keyring, KWallet and KeePassXC all answer it.
There is no plain-file fallback: with no Secret Service, `losos` says so in
one sentence and exits non-zero.

## Use

```sh
losos key import owner.asc          # or: losos key import --gpg <id>
losos login https://mattbox.local   # pin the host key, give the token
losos                               # the full-screen view
```

`login` fetches the host key, trying the box's path first and then the
edge's, and shows its fingerprint in groups of four. Compare it with the one in the web
UI's Access tokens pane before you say yes. You type the token without
echo. If a host you already pinned shows a different key later, `login`
refuses loudly and changes nothing.

The commands:

| command | what it does |
|---|---|
| `losos [open] [url]` | the full-screen view of a box or an edge |
| `losos login <url>` | pin a host key and keep a token for it |
| `losos logout <url>` | forget a host and its token |
| `losos hosts` | the hosts you are logged in to |
| `losos key import <file.asc>` | keep a secret key that can sign and has an encryption subkey |
| `losos key import --gpg <id>` | the same, exported from gpg |
| `losos key list`, `losos key remove <fpr>` | the keys you keep |
| `losos call <METHOD> <path> [json]` | one request; prints the inner status and the JSON, exits 1 on 400 or more |
| `losos edge secret list` | the edge's sealed credentials |
| `losos edge secret set <name>` | seal a new value, typed without echo or piped on stdin |
| `losos edge reconcile` | have the edge write its routing config again now |
| `losos ci setup --edge <url>` | give a GitHub repository its own key and token for the edge |
| `losos --version`, `losos --help` | |

Options:

- `--host <url>` picks a host when you are logged in to more than one.
- `--lang en|sk|de` sets the language. Without it, `LC_ALL`, `LC_MESSAGES`
  and `LANG` decide.
- `--theme dark|light` sets the colours. Without it, `COLORFGBG` decides,
  and dark wins when that is unset.
- `--key <fingerprint>` picks the signing key at login.

For a key under a passphrase, `losos` asks once per run and never stores
the answer. `LOSOS_PASSPHRASE` supplies it to a script.

In a script, `login` takes the fingerprint on the command line and the
token on stdin:

```sh
printf '%s\n' "$TOKEN" | losos login https://mattbox.local --fingerprint "<the fingerprint the web UI shows>"
```

### The full-screen view

The box's panes are the web UI's: Overview, Storage, Mesh, Apps, Network,
Security, Advanced, History, Backup and About. Advanced only shows values
here. Settings changes stay a draft until Apply, and Apply asks before the
box rebuilds itself. Access tokens, the market, the Lab and anything that wipes data
stay in the web UI. An edge has two panes. Overview lists its tenants with
their last heartbeat and shows which opt-ins are on, with a Reconcile
action. Secrets lists the sealed credentials and sets new values.

Keys: arrows or `j`/`k` to move, Enter to open or change, Tab between the
sidebar and the pane, `/` to search the panes the way the web sidebar
does, `a` to apply, `d` to discard, `r` to refresh, `?` for help, `q` to
quit. The footer shows the ones that apply.

### CI

`losos ci setup --edge <url> [--repo owner/name]` gives a GitHub
repository its own access to an edge you are already logged in to. The
repository is `dasmatus/losos` unless you name another.

1. It makes a new curve25519 key, `LosOS CI <owner/name>`, with no
   passphrase. It is never your own key.
2. It prints the public half and the fingerprint. Add the key to
   `losos.edge.access.gpgKeys` in the edge's configuration, rebuild the
   edge, and make a token bound to it on your own machine:
   `losos-registrar provision token create --edge <url> --name ci --key <fingerprint> --scope full`.
3. It asks for that token, without echo.
4. With `gh` installed and signed in, it sets the secrets
   `LOSOS_CI_GPG_KEY` and `LOSOS_EDGE_TOKEN` and the variables
   `LOSOS_EDGE_URL` and `LOSOS_EDGE_HOST_KEY` on the repository, each value
   on gh's stdin, never in its arguments. Without gh it prints the four
   names to add by hand, and their values only with `--print`.

`ci setup` also keeps the CI key in your keychain, like any imported key,
so you can rotate it later.

On the runner, set `LOSOS_CI=1`. `losos call` and `losos edge ...` then
read the key, token, URL and pinned host key from those four variables and
never touch a keychain. `edge secret set` reads the value from stdin.
Without `LOSOS_CI=1`, `losos` ignores the four variables.

```yaml
- run: printf '%s' "$VALUE" | losos edge secret set stripe-secret-key
  env:
    LOSOS_CI: "1"
    LOSOS_CI_GPG_KEY: ${{ secrets.LOSOS_CI_GPG_KEY }}
    LOSOS_EDGE_TOKEN: ${{ secrets.LOSOS_EDGE_TOKEN }}
    LOSOS_EDGE_URL: ${{ vars.LOSOS_EDGE_URL }}
    LOSOS_EDGE_HOST_KEY: ${{ vars.LOSOS_EDGE_HOST_KEY }}
    VALUE: ${{ secrets.STRIPE_KEY }}
```

## Develop

Node 20 or later. Everything below runs from `admin-ui/tui`.

```sh
npm ci
npm run build          # dist/losos.mjs, one file
npm run build:binary   # dist/losos, one executable (needs bun)
npm test               # builds, then runs every suite
npm run typecheck
npm run screenshots    # the PNGs under .github/screenshots/tui/after/
```

`npm run gen` runs first in each of them. It writes three files into
`src/generated/`. One is the palette parsed from the web UI's `tokens.css`,
light and dark, truecolor with a 256-colour fallback. The others are the salmon as half-block art drawn from
`salmon-128.png`, and the panes' search words taken from the web UI's `panes.ts`.
Git ignores those files and `dist/`.

The words come from the web UI's catalogue in `admin-ui/app/src/i18n`,
imported as it is. Strings only the terminal needs are in
[src/i18n.ts](src/i18n.ts), each in English, Slovak and German.

The tests run against a mock box and edge, `test/mock-server.ts`, which
makes the server's checks in the contract's order. A fake `secret-tool` and
a fake `gh` in `test/fake-bin/` go first on PATH. The suites cover:

- the envelope: wrong signer, stale timestamp, replayed nonce, a read
  token refused a POST, an answer with the wrong nonce or signed by the
  wrong key;
- the keychain;
- the bundled CLI end to end, including `ci setup` and `LOSOS_CI=1`;
- every pane, drawn with ink-testing-library;
- `install.sh`.

### Why bun for the binary

`npm run build:binary` compiles the bundle with `bun build --compile`
into one 80 MiB file that needs nothing installed. `npm run
verify:binary` drives that file through a real terminal, made by util-linux
`script`, against the mock: the login screen, the full-screen view with
its raw-mode keys, the hidden prompt of `edge secret set`, and the exit
statuses of the plain commands. `npm run verify:binary -- --bundle` does
the same for `node dist/losos.mjs`.

The fallback was a Node single executable, and bun made it unnecessary. It would
also be harder. Node 20 and 22 build one only from a CommonJS script, and
Ink's layout engine loads with a top-level await, which CommonJS cannot
hold.
