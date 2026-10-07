# Official-edge identity, provisioned from the box

This directory is a **Forgejo Actions provisioning repository** for LosOS Git
on the owner's box. Pushed there, its two workflows make the LosOS root key on
the box and hand each edge VPS its official identity, so the owner never
handles the private key by hand:

- the root private key is **made on the box**, by the box, and lives only as
  an Actions secret of this repository (Forgejo keeps secrets encrypted at
  rest). It leaves the box only as *signed certificates*;
- each edge gets its own keypair and a certificate signed by the root, both
  **deployed over SSH** from the box to the VPS;
- the root **public** key is published from the box too: committed here as
  `root.pub` and proposed to the LosOS repository on GitHub as a pull request
  that fills `keys/official-edge-root.pub`.

What "official" buys and how a box checks it is in the wiki,
[Master Proxy → Official edges](../../wiki/Master-Proxy.md#official-edges).
The runner that executes these workflows is part of LosOS
(`modules/git-runner.nix`, on with LosOS Git by default): host mode, the
unprivileged `losos-git-runner` user, label `losos`.

## Files

| file | runs where | does |
| --- | --- | --- |
| `.forgejo/workflows/root-keygen.yml` | the box | once: `root-keygen.sh`, commit `root.pub`, `publish-root-key.sh` |
| `.forgejo/workflows/provision-edge.yml` | the box | per edge: `provision-edge.sh` |
| `root-keygen.sh` | the box | makes the root key, stores it as the `LOSOS_ROOT_KEY` secret through the Forgejo API, shreds the file, prints the public key. Refuses to run twice. |
| `provision-edge.sh` | the box | edge keypair → certificate signed by the root → one SSH session installing both on the VPS → `GET /identity?nonce=` checked with `losos-registrar identity verify` |
| `publish-root-key.sh` | the box | opens (or updates) the GitHub PR that writes the public key into `keys/official-edge-root.pub` |

Every script also runs on a laptop with `losos-registrar`, `ssh`, `curl` and
`jq` on PATH (`--help` on each), which is the fallback if the box is not the
place you want the root key to live.

## Operator steps

You need: a box running LosOS with LosOS Git enabled (the default) and built
from a tree that carries `modules/git-runner.nix`; a VPS running the edge
module (`nixosModules.edge`, see the wiki) that you can SSH into as root or as
a user with passwordless `sudo`; your owner account on LosOS Git.

1. **Create the repository.** On LosOS Git, create a *private* repository,
   say `owner/edge-identity`. Keep it to yourself: every workflow in it can
   read the root key.
2. **Push these files into it**, including the `.forgejo/` directory:

   ```sh
   git clone <LosOS Git URL>/owner/edge-identity.git
   cp -r <losos checkout>/provisioning/edge-identity/. edge-identity/
   cd edge-identity && git add -A && git commit -m "edge identity provisioning" && git push
   ```

3. **Set the repository's Actions secrets** (repository Settings → Actions →
   Secrets):
   - `FORGEJO_TOKEN`: an access token of your account (user Settings →
     Applications → Generate token, repository read and write). The
     `root-keygen` workflow uses it to write the `LOSOS_ROOT_KEY` secret and
     push `root.pub`.
   - `GITHUB_TOKEN_LOSOS` (optional): a GitHub fine-grained token with
     *Contents* and *Pull requests* write on the LosOS repository. With it the
     workflow opens the PR that publishes the public key; without it, the
     workflow prints the key for you to paste.
4. **Run `root-keygen`** (Actions tab → root-keygen → Run workflow). Once.
   Afterwards the repository has a `LOSOS_ROOT_KEY` secret and a `root.pub`
   commit, and GitHub has a PR on branch `official-edge-root-key`. The
   workflow refuses to run again while the secret exists, because a second
   root would orphan every certificate the first one signed.
5. **Merge that PR.** Boxes treat edges signed by this key as official once
   they upgrade to a tree that carries it; until then the market stays off
   (fail closed). The edge side does not wait for this.
6. **Per edge, add two more secrets:**
   - `EDGE_SSH_KEY`: an OpenSSH private key the VPS accepts. On your laptop,
     `ssh-keygen -t ed25519 -N '' -f edge-deploy`, put `edge-deploy.pub` into
     the VPS's `authorized_keys` (root, or a passwordless sudoer), paste the
     contents of `edge-deploy` as the secret, then delete both files.
   - `EDGE_KNOWN_HOSTS`: the output of `ssh-keyscan edge.example`.
7. **Run `provision-edge`** with the edge's name (what the Mesh pane will
   show), the URL boxes probe (`https://register.example`) and the SSH
   target (`root@edge.example`). The first run installs
   `/var/secrets/losos-edge-identity.key` (0600) and
   `/etc/losos/edge-identity.cert.json` on the VPS and ends with **"not
   official yet"** (exit 2): the registrar serves `/identity` only once its
   configuration names those two files.
8. **On the VPS**, add to the edge's NixOS configuration and rebuild:

   ```nix
   losos.edge.identity.keyFile = "/var/secrets/losos-edge-identity.key";
   losos.edge.identity.certFile = "/etc/losos/edge-identity.cert.json";
   ```

   Strings, not path literals: a path literal would copy the file into the
   Nix store at evaluation time, which is fine for the certificate and wrong
   for the key, and would fail when evaluating anywhere but on the VPS.
9. **Run `provision-edge` again with `verify_only`.** It fetches
   `GET <url>/identity?nonce=<fresh>` and runs the same four checks a box
   runs; the log ends with `official: <name> at <url>`. Every box with that
   edge in reach shows the check sign on its Mesh pane within one scan
   (20 s) and the market opens.

Renewal is step 7 again before the certificate expires (`days`, default
365): it issues a fresh edge key and certificate, and the registrar picks
them up on restart. The workflow restarts it.

## What stays manual

- Creating the repository and pushing these files (steps 1–2).
- The tokens and SSH material (steps 3 and 6): the box cannot create an
  account token for you, and it must not hold a key that opens the VPS
  unless you put it there.
- The two lines in the VPS's configuration and its rebuild (step 8): the
  edge's configuration is yours, not the box's.
- Merging the GitHub PR (step 5), which is what turns the key on for every
  box.
- Keeping the box. The root key exists in exactly one place, Forgejo's
  secret store under the box's `/var`. A reinstall of the box loses it, and
  then every edge has to be re-certified under a new root, which means a
  new release of LosOS with a new public key. If that is not acceptable,
  make the root on a laptop instead (`losos-registrar identity keygen --out
  root.key`), keep `root.key` offline, and set `LOSOS_ROOT_KEY` to its
  contents yourself instead of running `root-keygen`.
