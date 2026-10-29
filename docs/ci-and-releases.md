# CI and releases

CI is `.github/workflows/ci.yml`. It runs the same commands as the devenv
scripts, written out directly, because running jobs through `devenv shell`
was too slow. Keep the two in step by hand.

`.github/actions/setup-nix` installs Nix and writes the substituters to
`/etc/nix/nix.conf` before the daemon starts. Nix ignores `NIX_CONFIG` from
untrusted clients, and a build that misses the cache compiles from source
without saying so.

## Jobs

- clippy and rustfmt (rustfmt checked on pull requests only)
- Rust tests for both crates
- flake evaluation
- package builds
- the installer ISO, built on every push and then booted under OVMF and
  SeaBIOS by `nix/tests/iso-boot.py`. A boot passes when the installer sends a
  DNS query for `github.com`. On timeout, the job uploads a screenshot.
- discovery: a netboot test box (`nix/tests/discovery/box.nix`, LosOS Cloud
  and the Windows share) built once, then booted in QEMU on an Ubuntu, a
  macOS (Intel) and a Windows Server runner, on a tap, vmnet-host and a TAP
  adapter. `nix/tests/discovery/check.py` claims it and asks each system
  with its own tools whether it lists the box once (Avahi, Bonjour, File
  Explorer's Network view), mounts the owner's SMB share and LosOS Cloud's
  WebDAV address, and reads and writes a file through both. Then it switches
  the share off over the box's serial console and checks that the WebDAV
  entry takes the SMB one's place. Windows Server's WebDAV redirector needs
  a restart to install, so where it is missing the Windows leg checks WebDAV
  over HTTP and says so. `devenv shell discovery` runs the Linux leg locally.
- `losos-registrar` for the dev machine, a static musl build pushed to GHCR
  on every push to main and every tag and served by the proxy (below)

## What a pull request runs

A pull request runs only the jobs its files reach. The `changes` job lists
the pull request's files and `.github/scripts/ci-changes.sh` maps them to
lanes, one per group of jobs:

| Lane | Jobs | Turned on by |
|---|---|---|
| `rust` | clippy, rustfmt, the crate suites | the eleven crates, and the web UI files `admin-ui/tui/build.rs` reads |
| `wasm` | clippy for wasm32 | `backend/plugins/`, `backend/mdx/`, `admin-ui/lab/` |
| `solana`, `evm` | the escrow programs | `edge/contracts/`, `edge/registrar/` |
| `eval` | the flake evaluation | anything but Markdown, Rust, pictures, `docs/` and `.github/` |
| `ctl` | `losos-ctl` | the backend crate |
| `ui` | registrar, admin UI, handbook, browser checks | `admin-ui/`, `handbook/`, `edge/registrar/` |
| `tui` | the static `losos` client | `admin-ui/tui/` and what its build reads |
| `images` | the cluster's OCI image | `nix/images.nix` |
| `iso` | the ISO and its boot legs | the modules the ISO imports, the installer, the signing tool |
| `discovery` | the discovery box and its three runners | the modules `nix/tests/discovery/box.nix` imports, LosOS Cloud, the Windows share |

`flake.nix` and `nix/packages.nix` turn on every lane that builds with Nix.
A change to CI itself or the toolchain (`ci.yml`, `.github/actions/`,
`flake.lock`, devenv, `nix/fast-build.nix`) turns on everything. The job
summary of `changes` names the file that turned each lane on.

A skipped job is grey, not red. The `result` job at the end of the run
passes when every job that ran passed, so it is the one check worth
requiring on `main`.

Everything runs on pushes to `main`, on tags, on the weekly run, on a manual
run from the Actions tab, and on a pull request labelled `ci: full`. The
label is read when the job runs, so add it and re-run the workflow.

To see what a change would run before pushing it:

```sh
git diff --name-only origin/main... | .github/scripts/ci-changes.sh
```

When a new file is read by a job outside its lane (a `build.rs` reading the
web UI, a VM test reading a Rust file), add it to that lane in the script.

Beside `ci.yml`, `dependency-hashes.yml` keeps the Nix hashes of the lock
files current. On a pull request from a branch of this repository that
changes a `Cargo.lock` or `package-lock.json`, Dependabot's included, it runs
`.github/scripts/fix-hashes.py`, which vendors each changed lock file with a
placeholder hash so no cache can answer for it, and pushes the real hashes
back to the branch as one commit. It runs as `pull_request_target`, so a
Dependabot branch gets a token that can push, and for the same reason it
never takes a fork's branch. With the `LOCKFILE_UPDATE_TOKEN` secret its
push starts CI on the branch like any other; without it the job starts
`ci.yml` itself. The weekly `lockfile-updates.yml` runs the same script
before it opens its pull request. Run it by hand from the Actions tab with a
branch name, and tick `all` to check every lock file.

A pull request can still bring a stale hash to `main`: it was merged while
its run waited for a runner, or the lock file changed in the merge. So the
same workflow also runs on every push to `main` that touches a lock file,
checks the lock files changed in the last five merges, and when one is stale
opens a pull request from a `bot/dependency-hashes-<sha>` branch with the fix
and closes any older one it made. It never pushes to `main`, which takes
signed commits only.

`stale-branches.yml` deletes stale branches every Sunday:
any branch whose last commit is older than 30 days (the `STALE_BRANCH_DAYS`
repository variable overrides it), except the default branch, `gh-pages`,
`release/*`, and branches that are the head or base of an open pull request.
Each deletion is leased to the tip it judged, so a branch pushed to during
the run survives. The job summary lists what went, with the
`git push origin <hash>:refs/heads/<branch>` line that restores each one. Run
it by hand from the Actions tab for a dry run.

`failed-runs.yml` deletes unsuccessful workflow runs every day: any completed
run that ended in failure, cancelled, timed out or failed to start, unless its
commit is still live, the tip of a branch or the head of an open pull request,
so a red `main` or a red PR keeps the run that says why. One pass deletes at
most 800 (`GITHUB_TOKEN` has 1000 calls an hour) and the next day takes the
rest. A deleted run's logs and artifacts are gone for good; the job summary
lists each one. Run it by hand from the Actions tab for a dry run, or untick
`dry_run` to clear the backlog at once.

## Merging

Local hooks (`nix/hooks`, see [Development](development.md#git-hooks)) can
be skipped with `--no-verify`, and a pull request can be merged while its
run is still going. What actually holds is a ruleset on `main`, set once
under Settings, Rules, Rulesets, New branch ruleset:

- Target: the default branch. Enforcement: Active.
- Require status checks to pass: the check
  `result: every job this change needs passed`, with "Require branches to
  be up to date before merging" ticked. The `result` job sums up the run,
  so one required check covers every lane, skipped ones included, and the
  up-to-date rule makes a pull request run against the `main` it lands on.
  Two pull requests that each pass alone can still break `main` together
  (one removes an option, the other adds a test that sets it), and only a
  run on the merged tree sees that.
- Block force pushes and Restrict deletions.
- Bypass list: the history-scrub deploy key (Deploy keys) and GitHub
  Actions, which push to `main` themselves (the scrub, `fmt-bot`).

With many pull requests open, "up to date" means merging `main` into each
before it can land. A merge queue does that merging itself: tick "Require
merge queue" instead. `ci.yml` already runs on `merge_group`, every lane,
so the queue's runs report `result`.

Runs on `main` are never cancelled by a newer push (`concurrency` in
`ci.yml`), so every merge gets a finished run. Only a pull request's
superseded runs are cancelled.

## Releases

Push a `v*` tag, or draft a release with a new `v*` tag in the web UI.
The version needs no bump first. The boot menus, the boot pictures,
os-release, the console banners and `losos --version` show
`nix/version.nix`, and CI keeps that file on the newest tag:

- `version.yml` commits the newest `v*` tag to `nix/version.nix` on
  main when a tag is pushed, and again on any push to main that finds the
  file behind. Installed boxes build from main, so they show the new
  version after their next nightly update. The commit goes through the
  GitHub API with `GITHUB_TOKEN`, so GitHub signs it, and it only
  fast-forwards. A file already ahead of every tag is left alone.
  `.github/scripts/sync-version.sh --dry-run` shows what it would commit.
- The `tui` job writes the tag over the file before it builds `losos`, so
  the client published for the tag names it.
- `desktop-pin.yml` opens a pull request on LosOS Desktop
  (`losos-project/desktop`, or the repository the `DESKTOP_REPOSITORY`
  variable names) that moves its pin of this repository,
  `nixos/pkgs/losos-pin.nix`, to the tag. The desktop builds `losos` from
  that commit and boots a box from it beside the desktop in its
  `losos-box` VM test, so the pull request's CI is where a change here
  that breaks the desktop shows up. `.github/scripts/desktop-pin.sh TAG
  DIR` rewrites the file in a checkout of the desktop, hashes included.
  Opening the pull request needs the `DESKTOP_PIN_TOKEN` secret, a
  fine-grained token for the desktop repository with Contents and Pull
  requests read and write; without it the job puts the file in its
  summary. Release candidates are left out; *Run workflow* takes any tag.

The release job:

1. writes the tag into `nix/version.nix`, then builds the installer ISO
   and the demo QCOW2,
2. signs the ISO's UEFI loader in place with the `SECURE_BOOT_DB_KEY`
   secret (`losos-sign-iso`) and signs `SHA256SUMS` with the same key. If
   the secret is missing or signing fails, the release still publishes:
   the ISO is the unsigned build, the run carries a warning, and the
   release notes say *Not signed* and why,
3. attaches the ISO, its `.sha256`, `SHA256SUMS`, `SHA256SUMS.sig` and the
   certificate (`.pem` and `.cer`) to the GitHub release,
4. pushes both media to GHCR, since the QCOW2 is over GitHub's 2 GiB asset
   limit.

The `iso` job on main does the same signing before its boot legs. OVMF with
Microsoft's keys must refuse the medium, and the same OVMF store with the
committed certificate added beside Microsoft's must boot it (`nix/tests/iso-boot.py --firmware
uefi-sb-ms|uefi-sb`). A fork's pull request has no key, so it boots unsigned
and skips the enrolled leg, and so does a run where signing failed. The key is made once on the owner's machine with
`docs/provisioning/secure-boot/keygen.sh`.

Your release notes are kept. The download section goes between two marker
comments, and a re-run replaces only that section.

### Media for a tag that has none

A release whose tag run failed can get its media later. Open Actions, pick
the `ci` workflow, choose *Run workflow* on `main`, and fill in
`release_tags`: either `missing`, for every published release without an
installer ISO, or the tags themselves, separated by spaces (`v0.1.7
v0.1.8`). Each tag is built from its own tree, the same way a tag push
builds it, two at a time, and its media are attached to the existing
release. The release's title and your notes stay as they are. Tags from
before the edge gateway image (v0.1.6 and older) cannot build the media
and are skipped with a notice.

A tag that does not build as tagged cannot get media that way. Once the
break is fixed on `main`, put that fix as a patch in
`.github/release-fixes/<tag>/` (`git format-patch -1 <commit>`) and run
the workflow for the tag. The patches are applied over the tag's tree
before the build, the tag itself stays where it is, and the run notes
each patch it used. Only fixes that are on `main` belong there: an
installed box rebuilds from `main`, not from the tag. v0.1.9 is built
this way, with the Stripe gate fix from `main` that makes
`losos-registrar` compile.

## Binary cache

CI publishes signed Nix store paths as OCI artifacts at
`ghcr.io/dasmatus/losos/nix-cache`. A separate deployment of the
[LosOS Desktop proxy](https://github.com/dasmatus/losos-desktop/tree/main/proxy)
serves them, with `GHCR_REPOSITORY=dasmatus/losos`.

The publisher stores one OCI artifact per Nix store path at
`ghcr.io/dasmatus/losos/nix-cache:<store-hash>`. The appliance, installer ISO
and CI runners pull these artifacts directly from GHCR with `oras`; no hosted
cache proxy is needed for Nix substitutions.

Setup:

1. Run `nix key generate-secret --key-name losos-1` once.
2. Store the secret half as repository secret `NIX_CACHE_SIGNING_KEY`.
3. Make the `losos/nix-cache` GHCR package public after CI creates it.
4. Keep the public half in `modules/options.nix` as
   `losos.cache.trustedPublicKeys`. It must match the signing secret or Nix
   will reject the signatures and fall back to building from source.

The `setup-nix` action starts `tools/losos-ghcr-cache.py` on
`127.0.0.1:8787` after installing Nix. The installer ISO and installed
appliance run the same service from their system configuration. It pulls only
the requested store-path artifact, caches the NAR on local disk, and rewrites
only the unsigned `URL` field in narinfo; Nix still verifies the signed
store path, NAR hash and references. `cache.nixos.org` remains enabled for
upstream paths, and GitHub Pages remains a static fallback. The separate
`LOSOS_PROXY_URL` deployment is still used by non-cache features such as
update downloads, but it is not part of the binary-cache path.
Every job reports what it got from the cache. `setup-nix` puts a `nix` shim
on `PATH` that counts nix's own `copying path '…' from '<url>'` and
`building '…'` lines per invocation. It writes a **Nix binary cache use**
table to the job's step summary, one row per nix command that copied or
built anything, with paths from the LosOS cache, from `cache.nixos.org`,
from elsewhere, and built on the runner. It also prints one `LosOS cache: …`
line in the job log right after the command. A row with the LosOS column at
0 and a non-zero "built here" means this project's own paths were compiled
although an earlier run should have pushed them. A job that only evaluates
adds no row. The shim changes neither nix's output nor its exit status.

Check `<proxy-url>/nix-cache-info` and `nix copy --from <proxy-url>
<store-path>` before relying on it. An unreachable cache is not fatal. The
appliance waits 5 s and falls back to `cache.nixos.org` and building. Never
put the secret key in the flake or on an appliance.

### The copy on GitHub Pages

The GitHub Pages site that serves the handbook also serves a copy of the
cache at `https://losos.dasmat.us/proxy`. Pages runs no code, so the copy is
a plain file cache and remains a fallback if direct GHCR access is unavailable. On each
push to main, the `publish-cache` job writes the paths it publishes with
`nix copy --to file://`, signs them with the same `losos-1` key and leaves
out every path cache.nixos.org already serves. The `handbook` workflow runs
again when `ci` finishes on main and deploys the newest copy under `/proxy`.

It has three limits:

- It holds the newest build of main only. A box on an older revision finds
  its paths on the proxy alone.
- A Pages site may be at most 1 GB, so the copy keeps under 800 MiB of NARs
  and drops the largest paths first. The job's step summary names what it
  left out.
- Its `nix-cache-info` says `Priority: 45`, after the proxy (30) and
  cache.nixos.org (40), so nix tries it after both.

`losos.cache.substituters` lists both by default. The repository's Pages
source must be "GitHub Actions" (Settings, Pages), as for the handbook.

## The dev-machine tool

A gateway's owner and an operator ([gateway runbook](./provisioning/edge-identity/README.md))
run `losos-registrar passport` and `provision` on their own computer, which
has no Nix store. `.#losos-registrar-static` is the same crate linked statically
against musl. The `registrar-and-ui` job builds it and the `publish-tool` job
pushes it to GHCR as the OCI artifact
`ghcr.io/dasmatus/losos/images:<channel>-x86_64`, one layer per file:
`losos-registrar`, `losos` (the terminal client, `.#losos-static`, built
the same way by the `tui` job), its `install.sh`, and `SHA256SUMS` over all three. That is the shape the proxy's
`/updates/<channel>/<arch>/<file>` route serves. It picks the layer whose
title is the file name and redirects to GHCR's storage, except `SHA256SUMS`,
which it serves inline. LosOS Desktop's sysupdate reads the same route. The
channel is `main` for a push to main, the tag for a release (which also moves
`stable`), and a cleaned-up branch name for a manual run elsewhere. Pull
requests never publish.

```sh
curl -fsSLO https://nightly.link/dasmatus/losos/workflows/ci/main/losos-pages-updates.zip && unzip -o losos-pages-updates.zip losos-registrar SHA256SUMS && sha256sum -c --ignore-missing SHA256SUMS && chmod +x losos-registrar
```

That is the newest successful run's `losos-pages-updates` artifact, kept
90 days, through nightly.link. The update artifacts can also be pulled directly from GHCR, without a hosted
proxy, with `oras pull ghcr.io/dasmatus/losos/images:main-x86_64`.

The job's step summary carries the artifact reference, the URL and the sum.
`oras pull ghcr.io/dasmatus/losos/images:main-x86_64` fetches the same files
without the proxy.

## Edge keys from Actions secrets

The `edge-credentials` workflow (`.github/workflows/edge-credentials.yml`)
seals the keys from the repository's Actions secrets, so no one pastes them
on the edge. It is the alternative to sealing them by hand as the handbook's
Market page (Operator setup) describes.

1. Make an SSH key for the workflow
   (`ssh-keygen -t ed25519 -N "" -f edge-credentials`) and set
   `losos.edge.credentials.deployKey` to its public half. The edge lets that
   key in as root only to run `losos-seal-credential`: no shell, no
   forwarding, no terminal.
2. Add the Actions secrets:

   | Secret                  | Holds                                                   |
   | ----------------------- | ------------------------------------------------------- |
   | `STRIPE_KEY`            | the Stripe secret or restricted key, test or live       |
   | `STRIPE_WEBHOOK_SECRET` | optional: both `whsec_` secrets, one per line           |
   | `EDGE_SSH_KEY`          | the private half of the key from step 1                 |
   | `EDGE_SSH_HOST`         | the edge's address (a secret or a variable)             |
   | `EDGE_SSH_KNOWN_HOSTS`  | the edge's line from `ssh-keyscan` (secret or variable) |
   | `EDGE_SSH_PORT`         | optional, 22 when unset                                 |

3. Run **edge-credentials** on `main` from the Actions tab. It prints whether
   the Stripe key is a test or a live key, never the key itself, and pipes
   each secret that is set into `losos-seal-credential`. That checks the
   secret's shape, seals it with `systemd-creds` under its credential name
   and restarts the gate. Rotating a key is changing the secret and running
   the workflow again.

### USDC networks

An edge settles USDC on the networks listed under
`losos.edge.market.usdc.networks`, all live or all test. On an edge that
takes cards they match the Stripe key's mode; on a coins-only edge they set
the mode (see below):

```nix
losos.edge.market.currency = "usd"; # prices are USDC cents
losos.edge.market.usdc = {
  enable = true;
  networks = {
    base = { pay = "0x…"; treasury = "0x…"; };          # LososEscrow, fee address
    arbitrum = { pay = "0x…"; treasury = "0x…"; };
    optimism = { pay = "0x…"; treasury = "0x…"; };
    solana = { treasury = "<base58 owner>"; };          # no contract on Solana
  };
  ensRpcUrlSealed = "/var/secrets/losos-ens-rpc.cred";  # optional
};
```

The names are `base`, `arbitrum`, `optimism` and `solana`. For a test edge
they are `base-sepolia`, `arbitrum-sepolia`, `optimism-sepolia` and
`solana-devnet`. Each EVM network needs its own
`LososEscrow` deployment (`edge/contracts/`), with the same treasury and the
releaser key's address. Deploying it from a fresh key at nonce 0 gives it the
same address on every chain. On Solana the treasury is the owner of
the USDC account that takes the fee, and there is no contract. The buyer's
transaction makes both transfers itself. `genesisHash` overrides the genesis a
test Solana network must report, for a local validator.

Every network has an RPC endpoint sealed as `usdc-rpc-<network>` to
`/var/secrets/losos-usdc-rpc-<network>.cred` (`rpcUrlSealed`), read by the
registrar with `LoadCredentialEncrypted=` like the Claude key. The USDC
credentials do not go through the Actions workflow. Seal each endpoint from
the operator's machine with the terminal client (`docs/edge-terminal.md`),
which reads the URL without echoing it:

```sh
losos edge secret set usdc-rpc-base      # https://base-mainnet.g.alchemy.com/v2/KEY
losos edge secret set usdc-rpc-solana    # https://mainnet.helius-rpc.com/?api-key=KEY
losos edge secret list
```

Payments on an EVM network wait in the escrow until the registrar releases
them to the seller and the treasury, or refunds them to the buyer, with the
releaser key. The edge makes that key itself, so it never exists anywhere
else:

```sh
losos edge escrow releaser --new    # prints the address, never the key
losos edge escrow releaser          # the address, its ETH and whether each escrow takes it
```

The registrar draws the key from the system's CSPRNG, pipes it to
`losos-seal-credential` on stdin as `usdc-releaser-key`
(`/var/secrets/losos-usdc-releaser-key.cred`, `releaserKeySealed`) and
answers only the address. `losos edge secret set usdc-releaser-key` is
refused for that reason. Deploy each `LososEscrow` with that address as the
releaser, then send it a little ETH on every EVM network for gas. The key
can do nothing but release a deposit to the seller and treasury recorded at
payment, or refund it to the payer. `--new` refuses while a key is sealed;
`--replace` makes another, after which the treasury calls
`setReleaser(<new address>)` on every escrow. The registrar refuses to
confirm payments while an escrow's `releaser()` is another address, and the
second command shows which ones still do.

`ens-rpc` (`losos edge secret set ens-rpc`) names an Ethereum mainnet
endpoint (Sepolia for a test edge). With it, the admin pages show ENS names and take them when
sending USDC; without it, they do neither. The registrar checks its chain id
before the first lookup and forwards only `eth_call`, `eth_chainId` and
`eth_blockNumber` to it.

The registrar starts with any of these still unsealed (the unit falls back
to a placeholder, which reads as "not configured"), so the terminal can
reach it to seal them; until they are, the market answers 503 for USDC. Plain `http` is accepted only for a node
on the edge's own loopback, such as `http://127.0.0.1:8545`.

### Buying USDC with euros at Binance

The wallet's euro purchase is Binance Connect's hosted on-ramp, and the edge
is the Binance partner (`edge/registrar/src/onramp.rs`). It needs USDC on
`base` (live) and a partner account, which Binance's Pay Onchain team opens
on request. Binance has no sandbox; a new partner starts on Binance's QA
environment. Binance asks for:

- the partner's RSA public key. Make the pair on the operator's machine,
  2048 bits or more: `openssl genpkey -algorithm RSA -pkeyopt
  rsa_keygen_bits:2048 -out binance-connect.pem`, then `openssl pkey -in
  binance-connect.pem -pubout`. The registrar signs with ring, which takes
  no key under 2048 bits;
- the edge's public IP addresses, for Binance's allowlist;
- the notification URL, `https://register.<publicDomain>/market/onramp/webhook`;
- an 80×80 PNG logo for its checkout.

Binance answers with the API origin, a client id, a sign access token and
its own public key for notifications. The first three and the key file are
configuration; the private key and the token are sealed:

```nix
losos.edge.market.usdc.binance = {
  enable = true;
  apiUrl = "https://…";                          # from Binance
  clientId = "…";                                # from Binance
  webhookPublicKeyFile = ./binance-webhook.pub;  # Binance's key, PEM or base64
};
```

```sh
losos edge secret set binance-connect-key < binance-connect.pem
losos edge secret set binance-connect-token
```

The key lands in `/var/secrets/losos-binance-connect-key.cred`
(`keySealed`) and the token in `/var/secrets/losos-binance-connect-token.cred`
(`tokenSealed`); delete the PEM from the operator's machine once Binance has
the public half, or keep it offline. Until both are sealed the boxes are not
offered the purchase and the routes answer 503. Orders live in
`/var/lib/losos-registrar/onramp.json`, keyed by the 32-hex id the edge sends
Binance as `externalOrderId`; a box can read only its own.

### Coins only, without cards

`losos.edge.market.cards.enable` (default `true`) lets buyers pay by card
through Stripe Connect. Set it to `false` and the edge takes coins only:
USDC, plus Bitcoin and Monero where they are on. Such an edge needs no
Stripe account:

```nix
losos.edge.market = {
  enable = true;
  cards.enable = false;
  usdc.enable = true;     # required without cards
  bitcoin.enable = true;  # optional
  monero.enable = true;   # optional
};
```

- No `losos-stripe-gate` unit runs and no Stripe key or webhook secret is
  sealed, so `STRIPE_KEY` and `STRIPE_WEBHOOK_SECRET` can stay unset in the
  `edge-credentials` workflow.
- The registrar gets `--market-no-cards` in place of
  `--market-gate-socket`.
- The market's mode (the live or the test ledger) comes from its USDC
  networks, so a live edge lists the mainnet ones (`base`, `arbitrum`,
  `optimism`, `solana`).
- It needs `losos.edge.market.usdc.enable`; the build refuses the
  configuration without it.
- Hardware sales (`losos.edge.market.hardware.enable`) are paid by card, so
  they need cards on.
- `losos.edge.market.returnUrl` is needed only with cards. Without them it
  may stay empty.

### Bitcoin orders

With USDC on, an edge can also take Bitcoin for the same orders:

```nix
losos.edge.market.bitcoin = {
  enable = true;
  chain = "main";    # the default; a test edge: "testnet4", "signet" or "regtest"
  pruneMiB = 10240;  # the default
};
```

This starts `bitcoind-losos.service`, a pruned node with RPC on
`127.0.0.1:8332` only, and passes the registrar
`--market-btc-rpc http://127.0.0.1:8332` and the node's cookie
(`/var/lib/bitcoind-losos/<chain>/.cookie`, no subdirectory on `main`). There
is nothing to seal: the cookie changes each time the node starts and the
registrar reads it on every call. The registrar makes a watch-only,
private-key-free wallet `losos-watch` on first use and imports each order's
address into it. Before it takes a seller's key or hands out an address it
checks that the node's chain fits the market's mode, and refuses
otherwise. Until the node has caught
up with the chain, payments show up late, so let it sync before listing.

### Monero orders

With USDC on, an edge can also take Monero:

```nix
losos.edge.market.monero = {
  enable = true;
  network = "mainnet";  # the default; a test edge: "stagenet" or "testnet"
};
```

This starts `monero.service`, a pruned `monerod` with restricted RPC on
`127.0.0.1:18081` only, and `losos-monero-wallet.service`, a
`monero-wallet-rpc` on `127.0.0.1:18083` holding one view-only wallet per
seller under `/var/lib/losos-monero-wallet/wallets`. There is nothing to
seal: the wallet service writes a random `user:password` to
`/var/lib/losos-monero-wallet/rpc-login` on its first start and puts it in
its own config file, never on the command line, and the registrar reads the
same file on every call (`--market-xmr-login-file`). Before it takes a
seller's key or hands out a subaddress, the registrar checks that the node's
network fits the market's mode and that the seller's address is for that
network. As with Bitcoin, let the node sync before listing.

### Zcash orders

With USDC on, an edge can also take shielded Zcash:

```nix
losos.edge.market.zcash = {
  enable = true;
  network = "mainnet";  # the default; "testnet" or "regtest" only on a test edge
  # servers = [ ... ];  # the default lists four mainnet operators
};
```

This starts nothing. The registrar gets one `--market-zec-server` per entry
of `servers` and `--market-zec-network main`, and asks those public
lightwalletd servers over gRPC (TLS checked against the webpki roots) for
compact blocks and the mempool. It decrypts them itself, so the sellers'
viewing keys stay in the market ledger on the edge. Each server must report
`chainName` `main` on a live edge or it is not asked, and a payment counts
only when servers of two operators (told apart by domain) show it in a block
of the same hash. The registrar refuses a list of fewer than two operators at
start. Before it takes a seller's key or hands out an address it checks that
the network fits the Stripe key's mode, that the key is for that network,
and that two operators answer. The default servers came from
hosh.zec.rocks/zec; replace one that goes away. There is nothing to seal and
nothing to sync.

`edge/registrar/tests/zcash.rs` runs in every `cargo test`, with no node:
stand-in lightwalletd servers in the test process serve compact blocks whose
payments are real Orchard and Ironwood notes encrypted to the order's
address, one of them lying.

### Platform fees on coin sales

Bitcoin, Monero and Zcash payments go whole to the seller, so the market's
fee on them is a debt in the ledger (`coin_sellers.<appliance>.fees_owed`,
with `owed_since` and `fee_credit`), settled in USDC (handbook, Market,
"Platform fees on coin sales"). One knob:

```nix
losos.edge.market.coinFeeLimit = 1000;  # the default: US$10, in cents
```

It reaches the registrar as `--market-coin-fee-limit` (0 to 100000). While a
seller owes more than that, or has owed anything for 30 days (fixed), the
edge drops coins from that seller's new orders and refuses new coin
payments. 0 stops coin payments at the first cent owed. A ledger from before
this kept no start date. The first read stamps one, and the edge writes it
with the next change to the ledger.

Two things move USDC to the treasury for it, both into
`losos.edge.market.usdc.networks.<network>.treasury`:

- A USDC order of a seller who owes takes the debt on top of its fee, up to
  half the gross (the escrow's own cap). The invoice's `fee` includes it and
  `fee_offset` names it; the order keeps it as `fee_offset`. The debt drops
  when the edge's release of the deposit is mined, or when a Solana payment
  is confirmed, and never for a refund.
- `POST /market/fees/pay` (`tx`, `network`) counts a plain USDC `transfer`
  to the treasury from the seller's signed EVM payout address, read from the
  edge's own RPC endpoint after an `eth_chainId` check, with the usual
  confirmations. Each transaction counts once (`fee_payments` in the
  ledger), never for more than is owed, and a transaction that paid an order
  is refused. Solana is refused.

`edge/registrar/tests/usdc.rs` covers both paths and the pause against a
stand-in RPC. `edge/contracts/test-payment.sh` also sends a real fee transfer on
anvil and checks the registrar reads it.
