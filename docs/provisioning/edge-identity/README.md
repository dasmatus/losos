# Gateways and the operator tool

There is no LosOS root key and no certificate that makes an edge official.
A box trusts an edge with the market when the edge proves it for itself: its
owner's OpenPGP-signed passport, a TPM showing that a published gateway image
booted untouched, and settings that pay LosOS's share to the addresses every
box ships. How a box checks that is in the handbook,
[Master proxy → Official edges](https://losos.dasmat.us/in-depth/master-proxy#official-edges).

This page is for the people who run gateways and the tools around them:
`losos-registrar`, run on your own computer, never on a box.

## Getting the tool

CI builds `losos-registrar` for the dev machine, linked statically so it
runs on any x86_64 Linux with nothing installed, and keeps it in the
`losos-pages-updates` artifact of the `ci` workflow, which
[nightly.link](https://nightly.link) hands out without a GitHub sign-in.
One line fetches the newest successful build of `main`, checks it against
the sums CI wrote beside it, and makes it executable:

```sh
curl -fsSLO https://nightly.link/dasmatus/losos/workflows/ci/main/losos-pages-updates.zip && unzip -o losos-pages-updates.zip losos-registrar SHA256SUMS && sha256sum -c --ignore-missing SHA256SUMS && chmod +x losos-registrar
```

Put another branch in place of `main` for its newest green build. CI also
pushes the same files to GHCR as the OCI artifact
`ghcr.io/dasmatus/losos/images:<channel>-x86_64`, a release tag under its
own name and as `stable`, which `oras pull` fetches. Or build it yourself: `nix build
.#losos-registrar-static` (the same binary), `nix build .#losos-registrar`
(for a machine with Nix), or `cargo build` in `edge/registrar/`.

If the hosted update route is unavailable, pull the same artifacts
directly from GHCR with `oras pull
ghcr.io/dasmatus/losos/images:main-x86_64`; the `losos-registrar` binary and
`SHA256SUMS` are included in that artifact.

## Claiming a gateway

A gateway (`losos.edge.passport.enable`, on in the gateway image) makes its
own Ed25519 key on its first start; the private half never leaves it. Its
owner claims it with a passport:

1. Read the gateway's key: `GET <url>/passport/edge-key`.
2. Print the statement to sign:

   ```sh
   losos-registrar passport statement --url https://gw.example.net \
     --edge-key <hex> --payout base=0x… --payout solana=… --days 365
   ```

   and sign it: `gpg --detach-sign --armor statement.txt`.
3. Assemble the settings file, checked the way a box checks it:

   ```sh
   losos-registrar passport settings --statement statement.txt \
     --signature statement.txt.asc --key owner.asc --losos-payouts payouts.json
   ```

   `payouts.json` is LosOS's addresses as `nix/gateway/payouts.nix` has them.
4. Sign the settings file with the same key and send both: on the gateway's
   manage page, **Claim this gateway**, or `POST <url>/passport/settings`.
   The gateway writes the file and reboots to measure it into PCR 15. A later
   change needs a signature from the same owner.

`losos-registrar passport check --url <url> --tpm-roots <pem> --images
<json> --payouts <json>` runs a box's whole check against a running gateway
from anywhere: exit 0 when a box would trust it, 1 with the reason when not,
2 when the reason is tampering.

## Publishing a gateway image

A box trusts only the images in `nix/gateway/images.nix`: per image, the PCR
11 its unified kernel image leaves and the Authenticode hashes of every
program the firmware starts on the way to it.
`losos-registrar passport measure --uki <file> --name <name> --app <kernel>`
prints an entry. The list is empty until a gateway image boots as a unified
kernel image, and LosOS's payout addresses in `nix/gateway/payouts.nix` are
empty until they are written there. Until both are filled no gateway passes
and the market stays off on every box.

## The operator sign-in

`losos-registrar provision` signs you in with GitHub's OAuth device flow and
refuses unless your account's numeric id is in `edge/registrar/operators.json`,
which is compiled into the binary.

- `losos-registrar provision whoami` prints the code, waits for you to enter
  it at <https://github.com/login/device>, and ends with your login, your id
  and the allowlist row that admitted you. Anyone else gets "is not on the
  operator allowlist" and exit code 3.
- `losos-registrar provision token create|list|revoke --edge <url>` makes
  and revokes an edge's terminal tokens (`docs/edge-terminal.md`) with the
  GitHub token of the sign-in. The edge checks that token against its own
  copy of the allowlist.

`LOSOS_GITHUB_TOKEN` (for example `$(gh auth token)`) skips the browser; the
token is still resolved to an account and checked against the list.

## One-time: the GitHub OAuth App

The sign-in is GitHub's OAuth *device flow*: the tool prints a one-time
code, you enter it at <https://github.com/login/device> in any browser, and
GitHub hands the tool a token for your account. It needs an OAuth App to
belong to, and only the App's **public client id**; there is no client
secret in this flow and none is stored anywhere.

1. On GitHub, open *Settings → Developer settings → OAuth Apps → New OAuth
   App* (under your own account, not an organisation).
2. Fill it in:

   | field                      | value                                   |
   | -------------------------- | --------------------------------------- |
   | Application name           | `LosOS provisioning`                    |
   | Homepage URL               | `https://losos.dasmat.us`               |
   | Application description    | anything, e.g. `LosOS operator sign-in` |
   | Authorization callback URL | `https://losos.dasmat.us/` (required by the form; unused by the device flow) |
   | **Enable Device Flow**     | **ticked**                              |

3. Register it and copy the **Client ID** (it looks like `Ov23li…` or
   `Iv1.…`). Do not generate a client secret; nothing here uses one.
4. Put the client id into `edge/registrar/operators.json` as
   `github_oauth_client_id` and merge that change: the id is public, and the
   tool is built with it. Until then every gated command stops with "no
   GitHub OAuth App client id" and tells you this; `--client-id <id>` or
   `LOSOS_GITHUB_CLIENT_ID=<id>` work meanwhile.

Who is allowed is the `operators` list in the same file. Adding or removing
a person is a pull request, reviewed like the key itself.

## What stays manual

- Creating the OAuth App and committing its client id (once).
- Making the `losos/images` GHCR package public, once CI has created it
  with its first push to main: GitHub creates a package private, and the
  proxy answers `502 token: 403` for a private one. Same as `losos/nix-cache`
  before it, see the handbook's CI and releases page.
- Writing LosOS's payout addresses into `nix/gateway/payouts.nix` and the
  published image into `nix/gateway/images.nix`.
