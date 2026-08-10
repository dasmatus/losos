# losos-ctl — the losos backend

The privileged Haskell backend the losos Nextcloud app talks to. It is the only
thing on this SSH-less appliance that can rewrite the persisted flake config and
run `nixos-rebuild`. The PHP app invokes it over a locked-down sudoers rule
(user `nextcloud` → root, NOPASSWD, command pinned to this binary).

The wire format is specified in [schema.json](./schema.json) (JSON Schema
2020-12). Summary:

| Subcommand | stdout JSON |
|---|---|
| `state --json` | `{"mode":"local\|mesh","sharing":bool}` |
| `change --mode <local\|mesh>` | `{"job":"<id>"}` (async rebuild starts) |
| `status --json` | `{"state":"idle\|building\|done\|failed","progress":0-100,"message":"…"}` |
| `rebuild-done <exitcode>` | *(internal, no stdout)* records terminal rebuild status |

## What `change` does

1. Rewrites the `losos.sharingMyStorage = <bool>;` line in the persisted flake
   file (default `/etc/nixos/defaults.nix`).
2. Updates `state.json` to `building` and records a job id.
3. Spawns a **detached** rebuild: `setsid -f sh -c 'nixos-rebuild switch --flake
   /etc/nixos#install > log 2>&1; ec=$?; losos-ctl rebuild-done $ec'`. `setsid`
   gives it a new session so it outlives the sudo/PHP process tree; `-f` forks
   so `setsid` returns immediately.
4. Prints `{"job":"…"}` and exits. The detached shell re-enters this binary via
   `rebuild-done` when nixos-rebuild finishes, flipping `status` to
   `done`/`failed`.

No second daemon — the CLI is its own rebuild watcher.

## Paths (env-overridable for dev/tests)

| Env | Default | Purpose |
|---|---|---|
| `LOSOS_STATE_DIR` | `/var/lib/losos` | state.json + rebuild.log |
| `LOSOS_CONFIG` | `/etc/nixos/defaults.nix` | flake file with the sharingMyStorage line |
| `LOSOS_FLAKE` | `/etc/nixos#install` | flake ref for nixos-rebuild |
| `LOSOS_CTL_BIN` | `/run/current-system/sw/bin/losos-ctl` | path the detached shell re-enters |

## Develop

From the repo root:

```sh
nix develop .#          # fish + full Haskell stdlib + PHP
cd backend
cabal build             # build losos-ctl
cabal run losos-ctl -- state --json          # needs LOSOS_* to do real work;
LOSOS_STATE_DIR=/tmp/losos cabal run losos-ctl -- change --mode mesh
LOSOS_STATE_DIR=/tmp/losos cabal run losos-ctl -- status --json
```

The flake also builds it as `.#packages.x86_64-linux.losos-ctl` (via
`callCabal2nix`), which `losos.backend.package` defaults to.