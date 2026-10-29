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
curl -fsSLO https://nightly.link/dasmatus/losos/workflows/ci/main/losos-pages-updates.zip
unzip -o losos-pages-updates.zip -d losos-pages-updates
sh losos-pages-updates/install.sh
```

The zip is the newest successful CI build of `main`, through
[nightly.link](https://nightly.link): `losos`, `losos-registrar`,
`install.sh` and `SHA256SUMS`. The script checks `losos` against the sums,
and puts it in `~/.local/bin`, or in `/usr/local/bin` when run as root. It
never runs sudo. Piped into `sh` on its own, it downloads the zip itself,
and `LOSOS_CHANNEL` then picks another branch. Read
[install.sh](install.sh) before you run it. It is short.

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

A box answers HTTPS with its own self-signed certificate. `login` accepts
the one it sees and, when no public authority vouches for it, keeps its
SHA-256 with the host. Every later call takes that certificate or one the
web trusts, and stops with a warning on any other. `login` again pins the
new one. Plain `http://` works too: the envelope does not depend on TLS.

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
| `losos edge escrow releaser` | the escrow key's address, its ETH on each network and whether each escrow takes it; `--new` makes the key on the edge, `--replace` makes another |
| `losos ci setup --edge <url>` | give a GitHub repository its own key and token for the edge |
| `losos cloud setup` | put LosOS Cloud in the file manager, with an app password of this computer's own in the keychain |
| `losos git setup` | let git clone and push to LosOS Git with a token of this computer's own, through `losos git credential` |
| `losos --version`, `losos --help` | |

Options:

- `--host <url>` picks a host when you are logged in to more than one.
- `--lang en|sk|de` sets the language. Without it, `LC_ALL`, `LC_MESSAGES`
  and `LANG` decide.
- `--theme dark|light` sets the colours. Without it, `COLORFGBG` decides,
  and dark wins when that is unset.
- `--key <fingerprint>` picks the signing key at login.
- `-v`, `-vv`, `-vvv` log what the client does to stderr, without
  timestamps. `LOSOS_LOG` takes a full tracing filter instead
  (`LOSOS_LOG=losos=debug,ureq=debug`). Tokens, keys and secrets are never
  logged. While the full-screen view is up, log lines wait until it closes.

Errors are miette reports: what went wrong, why, and what to do next.
`NO_COLOR` turns their colours off. The exit status is 1 for an error, 2
for a refused host key or a usage mistake, and 130 when you cancel.

For a key under a passphrase, `losos` asks once per run and never stores
the answer. `LOSOS_PASSPHRASE` supplies it to a script.

In a script, `login` takes the fingerprint on the command line and the
token on stdin:

```sh
printf '%s\n' "$TOKEN" | losos login https://mattbox.local --fingerprint "<the fingerprint the web UI shows>"
```

### The full-screen view

The box's panes are the web UI's: Overview, Storage, Mesh, Apps, Network,
Security, Advanced, History, Backup and About, and one of its own, This
computer, which runs the two setups above. Advanced only shows values
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

A Rust crate with its own `Cargo.lock`, not in a workspace. It uses clap
(derive) for the command line, miette for errors, tracing with
tracing-subscriber for logs, ratatui for the full-screen view, and
`losos-access` (`backend/access`) for the envelope, the same crate the box
and the edge open it with.

```sh
cargo test     --manifest-path admin-ui/tui/Cargo.toml
cargo clippy   --manifest-path admin-ui/tui/Cargo.toml --all-targets -- -D warnings
cargo run      --manifest-path admin-ui/tui/Cargo.toml -- --help
cargo run      --manifest-path admin-ui/tui/Cargo.toml --example screenshots
nix build .#losos-static   # the binary install.sh fetches: musl, no dynamic library
```

`build.rs` reads the web UI and turns four things into Rust: the catalogue
in `admin-ui/app/src/i18n`, the light and dark palette from `tokens.css`
(truecolor, with a 256-colour fallback), the panes' search words from
`panes.ts`, and the salmon from `salmon-128.png` as half-block art.
`LOSOS_UI_DIR` points it at another copy of `admin-ui`. Strings only the
terminal needs are in [src/i18n/messages.rs](src/i18n/messages.rs), each in
English, Slovak and German.

The tests run against a mock box and edge in `tests/common`, which open
calls with `losos-access` and make the server's checks in the contract's
order, over plain HTTP and over TLS with a self-signed certificate. A fake
`secret-tool` and a fake `gh` in `tests/fake-bin/` stand in for the real
ones. The suites cover:

- the envelope: what the client seals is what the box opens, and another
  signer, a stale call or a replay is refused; an answer for another nonce
  or signed by another key is refused too;
- the commands end to end, including `ci setup` and `LOSOS_CI=1`, and the
  answers a refusal, a changed host key or a stale call get;
- the certificate pin: taken at login, kept, and refused when it changes;
- `cloud setup` and `git setup` over TLS: the keychain items GVfs looks
  up, the bookmark, git's configuration with the key pin, and
  `git credential get` answering from the keychain (a real `git` writes
  and reads the configuration);
- the keychain;
- every pane, drawn into a buffer, in three languages, and that every
  catalogue key the sources name exists;
- `install.sh`;
- the built binary in a real pseudo-terminal (util-linux `script`): the
  login screen, the full-screen view with its raw-mode keys, the hidden
  prompt of `edge secret set`, and the exit statuses. `LOSOS_BIN` runs
  this one against another build, which is how CI checks the static binary
  it publishes.

The screenshots example draws twelve screens in both themes at 120 by 36,
turns each buffer into HTML and has Chromium shoot it into
`.github/screenshots/tui/after/`. `LOSOS_SHOTS` sets another folder and
`LOSOS_CHROMIUM` another browser.
