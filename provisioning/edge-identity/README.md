# Official-edge identity, provisioned from your own machine

The LosOS root key and every official edge's identity are made with one
tool, `losos-registrar provision`, run **on the operator's own computer**,
never on a box or a VPS. Every step that makes or uses the root key first
signs you in with GitHub and refuses unless your account is on the
allowlist compiled into the tool. What "official" buys and how a box checks
it is in the wiki,
[Master Proxy → Official edges](../../wiki/Master-Proxy.md#official-edges).

- the root private key is made **on your machine**, written once to a file
  you keep offline (0600, never overwritten), and leaves it only as signed
  certificates;
- each edge makes its **own** key pair on its first start and never lets
  the private half out; the tool reads the public half from the edge,
  signs the certificate, and pushes it back to the edge's registrar over
  HTTPS, where the same allowlist is checked a second time before anything
  is installed. No SSH access to the edge, nothing secret on your disk;
- the root **public** key is published by the same tool as a pull request
  that fills `keys/official-edge-root.pub`, using the same sign-in;
- who may do any of this is `backend-registrar/operators.json`: GitHub
  accounts by numeric id, with the login beside it for reading. The tool
  compares the id, because logins can be renamed and ids cannot, and the
  edge compares it again when the certificate arrives: the list is
  compiled into the one binary both of them run.

The gate decides whom the tooling serves and makes each signing a named
act. The root key stays the whole secret: keep `root.key` offline.

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
   | Application description    | anything, e.g. `official-edge key ceremony` |
   | Authorization callback URL | `https://losos.dasmat.us/` (required by the form; unused by the device flow) |
   | **Enable Device Flow**     | **ticked**                              |

3. Register it and copy the **Client ID** (it looks like `Ov23li…` or
   `Iv1.…`). Do not generate a client secret; nothing here uses one.
4. Put the client id into `backend-registrar/operators.json` as
   `github_oauth_client_id` and merge that change: the id is public, and the
   tool is built with it. Until then every gated command stops with "no
   GitHub OAuth App client id" and tells you this; `--client-id <id>` or
   `LOSOS_GITHUB_CLIENT_ID=<id>` work meanwhile.

Who is allowed is the `operators` list in the same file. Adding or removing
a person is a pull request, reviewed like the key itself.

## Getting the tool

CI publishes `losos-registrar` for the dev machine, linked statically so it
runs on any x86_64 Linux with nothing installed, to GHCR, and the LosOS
proxy serves it. One line fetches it, checks it against the sums CI wrote
beside it, and makes it executable:

```sh
curl -fsSLO "https://proxy.losos.dasmat.us/updates/main/x86_64/{losos-registrar,SHA256SUMS}" && sha256sum -c --ignore-missing SHA256SUMS && chmod +x losos-registrar
```

`main` is the newest build of the main branch; a release tag is published
under its own name (`/updates/v1.0/x86_64/…`) and as `stable`. The files
come from the OCI artifact `ghcr.io/dasmatus/losos/images:<channel>-x86_64`,
which `oras pull` fetches too; the proxy only redirects to GHCR's storage
(`SHA256SUMS` it serves itself). Or build it yourself: `nix build
.#losos-registrar-static` (the same binary), `nix build .#losos-registrar`
(for a machine with Nix), or `cargo build` in `backend-registrar/`.

When `proxy.losos.dasmat.us` does not answer, put
`https://losos-cache-proxy.vercel.app` in its place in the line above: it is
the same proxy under Vercel's own name. When neither answers, the same two
files of the newest main build are on the GitHub Pages site at
`https://losos.dasmat.us/proxy`.

## Operator steps

You need: `losos-registrar` on your machine (above), a VPS running the
edge module (`nixosModules.edge`, see the wiki) built from a tree that has
the push route (any build after this page says so), and a browser signed
in to a GitHub account on the allowlist. No shell on the edge is needed.

1. **Check the sign-in.** `losos-registrar provision whoami` prints the code,
   waits for you to enter it, and ends with your login and id and the
   allowlist row that admitted you; anyone else gets "is not on the operator
   allowlist" and exit code 3. Nothing else happens.
2. **Make the root key, once.**

   ```sh
   losos-registrar provision root-keygen --out root.key --publish
   ```

   Signs you in (with `--publish` it asks GitHub for `public_repo`, the one
   scope the pull request needs; without it, for nothing beyond your
   public profile), writes `root.key` (0600; it refuses to overwrite an
   existing file, because a second root would orphan every certificate the
   first one signed), prints the public key on stdout, and opens the pull
   request `official-edge-root-key` against `dasmatus/losos` that writes
   the key into `keys/official-edge-root.pub`. Run again with
   `provision publish --root-key root.key` if the PR has to be redone.
3. **Merge that pull request.** Boxes treat edges signed by this key as
   official once they upgrade to a tree that carries it; until then the
   market stays off (fail closed). The edge side does not wait for this.
4. **Per edge:**

   ```sh
   losos-registrar provision edge --name "LosOS edge Berlin" \
     --url https://register.example --root-key root.key --days 365
   ```

   `--name` is what the Mesh pane will show, `--url` the registrar URL
   boxes probe. The tool signs you in, asks the edge for its public key
   (`GET <url>/identity/public-key`; the edge made the key on its first
   start), signs the certificate with `root.key`, and pushes it
   (`POST <url>/identity/cert`, with the GitHub token of the sign-in in
   the `Authorization` header). The edge asks GitHub whose token that is
   and installs the certificate only if the account's id is on its own
   copy of the allowlist; then it answers `/identity` with it, no restart
   needed. The tool ends by running the box's four checks and printing
   `official: <name> at <url>`.

   What the edge answers when it refuses: `403` for an account that is
   not listed (exit 3 on this side; the tool itself refuses a stranger
   before it asks the edge for anything), `401` for a token GitHub does
   not accept, `400` for a certificate that is not for the edge's key,
   `404` for an edge that serves no identity key at all (see the note
   below). Nothing is written on a refusal.
5. **Verify**, any time, from anywhere:

   ```sh
   losos-registrar provision verify --url https://register.example --root-key root.key
   ```

   fetches `GET <url>/identity?nonce=<fresh>` and runs the same four checks
   a box runs; it ends with `official: <name> at <url>`. No sign-in: anyone
   may ask an edge who it is. Every box with that edge in reach shows the
   check sign on its Mesh pane within one scan (20 s) and the market
   opens.

The edge side needs nothing configured: `losos.edge.identity.keyFile` and
`certFile` default to `/var/lib/losos-registrar/identity.key` and
`identity.cert.json`, the registrar's own state directory, which is why it
can make the key and write the certificate there. An edge whose
configuration sets both to `null` serves no identity at all (every
`/identity*` route is 404) and cannot be made official by anyone.

Renewal is step 4 again before the certificate expires (`--days`, default
365): a fresh certificate for the same key, taken up the moment it lands.
Re-keying an edge is deleting its `identity.key` and restarting the
registrar, then step 4 (the old certificate is ignored, with a log line,
until the new one arrives).

Scripting: `LOSOS_GITHUB_TOKEN` (for example `$(gh auth token)`) skips the
browser; the token is still resolved to an account and checked against the
list, on both ends. Exit codes: 0 done, 1 failed, 2 certificate installed
but `/identity` not answering with it yet, 3 refused (by the tool or by
the edge). The push itself is one request, so a different tool can make
it too:

```sh
curl -fsS -X POST -H "Authorization: Bearer $(gh auth token)" \
  --data @edge.cert.json https://register.example/identity/cert
```

## What stays manual

- Creating the OAuth App and committing its client id (once).
- Making the `losos/images` GHCR package public, once CI has created it
  with its first push to main: GitHub creates a package private, and the
  proxy answers `502 token: 403` for a private one. Same as `losos/nix-cache`
  before it, see the wiki's CI page.
- Merging the GitHub pull request (step 3), which is what turns the key on
  for every box.
- Keeping `root.key`. It exists in exactly one place, the file you chose.
  Losing it means every edge has to be re-certified under a new root,
  which means a new release of LosOS with a new public key. Keep it
  offline, and keep a copy somewhere a laptop failure cannot reach.
