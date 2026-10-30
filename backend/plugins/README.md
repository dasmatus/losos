# lososd plugins

lososd is a small host. It owns the loopback API listener, the D-Bus name,
who a caller is (the spare key, LosOS Login's `Remote-User`, a managing
edge) and which section of `backend/sections.json` a route sits in. The
work behind a route is done by a plugin: a WebAssembly component that
answers the routes its manifest lists and reaches the box only through the
host's imports.

| Path | What it is |
|---|---|
| `wit/plugin.wit` | The contract: the request a plugin gets, the response it gives, every import. |
| `sdk/` | `losos-plugin`, the Rust side of the contract as plain functions. |
| `widget/` | `losos-widget`, hand-written widget files, shared by the look plugin and lososd's builder. |
| `names/` | `losos-names`, the names lososd and the pins plugin both take: an app's release name, a LosOS Git owner or repository, a LosOS Cloud path. |
| `look/` | The look: background picture and hand-written widgets (`/api/look*`). |
| `pins/` | What the owner pinned to the sidebar (`/api/pins`). |
| `pages/` | The pages as MDX: an administrator's changes and stylesheet, each save a commit in `/var/lib/losos/pages` (`/api/pages*`). |
| `probe/` | A test plugin that tries every import and misbehaves on request. Never shipped. |
| `../src/plugin/` | The host: loading, trust, manifests, the imports, limits. |

## Where plugins load from

lososd loads plugins from one directory, `LOSOS_PLUGIN_DIR`, which the
NixOS module sets to `losos.backend.plugins`, a Nix store path. Each plugin
is two files, `<name>.wasm` and its manifest `<name>.json`. lososd refuses
to start when the directory, or any file in it after links are followed:

- is outside `/nix/store`,
- has a write bit for anyone, or sits in a directory that does,
- is not owned by root,
- is on a filesystem mounted read-write (the store is a read-only mount).

So nobody on a running box can change what lososd runs. A new plugin
arrives the same way as a new lososd: a rebuild. A debug build also takes
`LOSOS_PLUGIN_TRUST_ROOT`, a stand-in for the store without the mount and
owner checks, which is what `tests/plugins.rs` uses. A release build never
reads it.

## The sandbox

A plugin is built for `wasm32-unknown-unknown` and has no WASI: no files,
clock, sockets or environment of its own. Everything goes through an
import, and lososd checks every import against the manifest before it acts.
A path outside the manifest's `files` roots, a program not on its `exec`
list, a URL outside its `http` endpoints or a unit name without its prefix
is answered `denied`. `wasm-tools component wit <name>.wasm` lists the
imports a plugin actually uses.

Each call runs in a fresh instance with the manifest's memory cap and wall
clock. A plugin that loops, runs out of memory or panics gets the caller a
500 and a line in the journal; lososd and the other plugins carry on.
Nothing in a plugin's memory outlives the call: what must is a file, an
entry in `cache` (lososd's memory, per plugin, gone at restart) or a job.

## The manifest

```json
{
  "name": "look",
  "description": "The owner's look.",
  "routes": [
    { "method": "GET", "path": "/api/look" },
    { "method": "PUT", "path": "/api/look/background", "maxBody": 8650752 },
    { "method": "GET", "path": "/api/look/background", "unguarded": true },
    { "method": "DELETE", "path": "/api/look/widgets/{id}" }
  ],
  "files": [
    { "path": "${LOSOS_STATE_DIR:-/var/lib/losos}/look.json", "access": "rw" }
  ],
  "exec": ["git", { "program": "/run/current-system/sw/bin/losos-cloud", "uids": [1000] }],
  "units": { "prefix": "losos-backup-", "programs": ["restic"] },
  "http": ["http://127.0.0.1:11434", "unix:/run/losos-search/cloud.sock", "https://*"],
  "locks": ["state"],
  "timers": [{ "name": "start" }, { "name": "sync", "every": 30 }],
  "dispatch": false,
  "limits": { "memoryMiB": 64, "timeoutSecs": 30, "jobSecs": 600, "jobs": 2 },
  "config": { "lookFile": "${LOSOS_STATE_DIR:-/var/lib/losos}/look.json" }
}
```

- `name` matches the file name: `look.json` beside `look.wasm`.
- `routes`: `/api/` or `/internal/` paths, `{name}` for a segment. Two
  plugins may not claim the same route; the more literal pattern wins
  between `/items/new` and `/items/{id}`. `maxBody` defaults to 256 KiB
  (ceiling 64 MiB). `unguarded` is for a `GET` the browser loads without a
  token, like an `<img>`; nginx still checks the person.
- Every route needs a line in `backend/sections.json`, or only an
  administrator reaches it. A lososd test fails on a route the table does
  not know.
- `files`: roots, `ro` or `rw`, a file or a directory, with `chown` listing
  the uids a write may hand a file to. The longest matching root wins, so
  an `ro` inside an `rw` holds. Below a root needs the root to be a
  directory already (`create-dir` it first).
- `exec`: bare names (resolved on lososd's `PATH`) or absolute paths, with
  `uids` a program may run as. Shells, `sudo`, `systemd-run` and the like
  are refused at load.
- `units`: transient systemd units under `prefix`, running a program on
  `programs`. lososd writes the `systemd-run` line; `User=`, `Exec*=` and
  the other properties that would widen the grant are refused.
- `http`: origins, `unix:` sockets, or `https://*` for any public host.
  lososd's own API is never an endpoint; use `dispatch`.
- `locks`: named locks shared across plugins. `state` is `state.json`'s.
- `timers`: `start` runs once when lososd starts; `every` is in seconds.
- `dispatch`: may call lososd's routes as a given caller, with that
  caller's section checked again.
- `${VAR}` and `${VAR:-default}` are filled in from lososd's environment at
  load, so a test can move every path. An entry that expands to nothing is
  dropped.

## Writing one

A plugin is a crate in this workspace with `crate-type = ["cdylib", "rlib"]`
and a `manifest.json` beside its `Cargo.toml`:

```rust
use losos_plugin::{fs, Plugin, Request, Response};

struct Notes;

impl Plugin for Notes {
    fn handle(req: Request) -> Response {
        match (req.method.as_str(), req.route.as_str()) {
            ("GET", "/api/notes/{id}") => {
                let id = req.param("id").unwrap_or_default();
                match fs::read_string_optional(&format!("/var/lib/losos/notes/{id}")) {
                    Ok(Some(text)) => Response::ok(&serde_json::json!({ "text": text })),
                    Ok(None) => Response::error(404, "no such note"),
                    Err(e) => Response::failed(&e),
                }
            }
            _ => Response::error(404, "not found"),
        }
    }
}

losos_plugin::export!(Notes);
```

`req.caller` says who is calling. lososd has already checked the route's
section; narrow further when a route is someone's own (`caller.person()`),
never wider. Keep the logic in a module that takes its effects as a trait,
the way `look/src/model.rs` takes a `Store`, so it is unit-tested on the host
with `cargo test` and no lososd.

## Build and test

```sh
cargo build --release --target wasm32-unknown-unknown --manifest-path backend/plugins/Cargo.toml
cargo test --manifest-path backend/plugins/Cargo.toml              # plugin logic, on the host
cargo test --manifest-path backend/Cargo.toml --test plugins       # the real lososd, both plugins
nix build .#losos-plugins                                          # what the box loads
nix build .#checks.x86_64-linux.losos-plugins                      # booted: store, sign-in, the look pane
```

`.#losos-plugins` builds every crate here except `probe` for wasm32, wraps
each with `wasm-tools component new` and puts it beside its manifest.
