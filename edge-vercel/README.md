# edge-vercel — the edge registrar as a Vercel Function

A demonstration host for the master-proxy control plane. It runs
`losos-registrar`'s own HTTP API — the same router, authentication, body cap
and load shedding the self-hosted edge runs — as one Vercel Function, keeps
the registry in a Redis reachable over HTTPS, and adds a status page an
audience can watch. It is **not** a replacement for the edge VPS
(`nixosModules.edge`), and the self-hosted path is unchanged by it.

## What runs here, and what cannot

| Edge component | On the VPS (`modules/edge.nix`) | Here |
|---|---|---|
| `losos-registrar serve`: `/register`, `/heartbeat`, `/deregister`, `/health` | yes | **yes, unmodified** |
| Tenant whitelist + token check | `losos.edge.tenants` → `tenants.json` + `/var/secrets` files | the same files, written from `LOSOS_TENANTS` |
| Registry persistence | `registry.json` on disk | Redis (Upstash) over HTTPS; memory only if none is configured |
| Reconciler (prune by TTL, Traefik + rathole config files) | timer, every 15 s | runs once per request; files land on the function's scratch disk and `/status/traefik` shows the Traefik one |
| rathole tunnel server (`:2333`, Noise) | yes | **no** — a raw TCP listener holding long-lived connections has no serverless shape |
| Traefik, per-tenant TLS, `*.publicDomain` routing | yes | **no** — nothing to forward to without the tunnel |
| rke2 mesh control plane, `/cluster/join` | optional | **no** (`/cluster/join` → 503, as on a proxy-only edge) |
| Stripe gate, `/market/*` | optional | **no** (`/market/*` → 503, as on an edge with the market off) |
| `/noise-public-key` | yes | 404 (no tunnel to pin a key for) |
| `/status`, `/status/traefik`, the page at `/` | — | **demo only**; the real edge has no public listing |

So the demo shows the **control plane**: an appliance (a VM, or any machine
running `losos-registrar announce`) authenticates with its token, is allotted
a tunnel port, heartbeats, reports whether it is idle, and disappears from the
registry when its heartbeats stop — and the page shows the exact Traefik
configuration the edge would be writing for it. The data path (a browser
reaching the box's Nextcloud through `https://<hostname>`) needs the tunnel
and stays a self-hosted demonstration (`tests/edge-vm.nix` is the automated
one).

## Privacy, stated plainly

This puts the registrar on a third party's platform and makes its registry
readable at `/status` by anyone who has the URL: appliance ids, the hostnames
the operator whitelisted, tunnel ports, last-seen ages and idle flags. The
tokens never leave the environment variables and the function's scratch
disk, are never written to Redis, and are never shown by any route — but a
production edge deliberately exposes none of this, which is why this host is
a separate crate meant to be torn down after the demonstration.

## Deploy

1. In Vercel, **Add New → Project**, import this repository, and set
   **Root Directory** to `edge-vercel`. Keep *Include files outside the root
   directory in the build step* enabled: the crate depends on
   `../backend-registrar` by path. Framework preset: *Other*. No build
   command — Vercel's Rust runtime finds `Cargo.toml` and builds the one
   `[[bin]]` under `api/`.
2. **Environment variables** (Settings → Environment Variables; mark the
   first one *Sensitive*):

   | Variable | Required | Meaning |
   |---|---|---|
   | `LOSOS_TENANTS` | yes | The whitelist, as JSON: `{"mattbox":{"hostname":"mattbox.losos.cfd","token":"<64 hex chars>"}}`. Same shape as `losos.edge.tenants`, with the token inline instead of a file. |
   | `LOSOS_HEARTBEAT_TTL` | no | Prune a box this long after its last heartbeat. Default `120s`. |
   | `LOSOS_PORT_RANGE` | no | Tunnel ports to allot from, `lo-hi`. Default `50000-50100`. |
   | `LOSOS_BOOTSTRAP_TOKEN` | no | rathole's `default_token`; only reaches the scratch-disk `rathole.toml`. Random when unset. |
   | `RUST_LOG` | no | e.g. `info` (default) or `losos=debug`. |

3. **Storage** (recommended): Storage tab → *Create Database* → **Upstash
   Redis** (free tier) → connect it to the project. The integration writes
   `KV_REST_API_URL`/`KV_REST_API_TOKEN` (or `UPSTASH_REDIS_REST_URL`/
   `UPSTASH_REDIS_REST_TOKEN`; both spellings are read) into the project and
   the function finds them on the next deploy. Pick the same region for the
   function and the store. Without a store the registry lives in one function
   instance's memory: fine for a five-minute demo, but a cold start forgets
   every box and two instances can disagree.
4. Deploy. `https://<project>.vercel.app/health` answers `ok`, `/` shows the
   page, `/status` the JSON.

The appliance token is the same value on both sides: whatever is in
`LOSOS_TENANTS` for an id must be the content of that box's
`losos.proxy.tokenFile`.

## Point an appliance at it

On a box (or the demo VM), `losos.proxy.registrarUrl =
"https://<project>.vercel.app"` with the matching `applianceId`, `hostname`
and `tokenFile`. The announce service registers and heartbeats; the rathole
client has nothing to dial and retries harmlessly (set
`losos.proxy.edgeRatholeEndpoint` to anything). Or run the client alone from
any machine:

```sh
printf '%s' '<the 64-hex token>' > /tmp/demo.token
losos-registrar announce --registrar-url https://<project>.vercel.app \
  --appliance-id mattbox --hostname mattbox.losos.cfd \
  --token-file /tmp/demo.token --heartbeat-interval 10s
```

Then open `https://<project>.vercel.app/`: the box appears within a second,
its *last seen* resets on every heartbeat, and it drops off `LOSOS_HEARTBEAT_TTL`
after the client is stopped (the page reloads `/status` every five seconds;
pruning happens on whichever request comes next).

## Develop

```sh
cargo test            # drives the real router + real store client against a fake Upstash
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

`devenv test` and CI run the same three for this crate alongside the other
two. To run the function locally the way Vercel does, build it and start the
binary with the environment above; `vercel_runtime` listens on
`127.0.0.1:${VERCEL_DEV_PORT:-3000}` when there is no bridge:

```sh
LOSOS_TENANTS='{"mattbox":{"hostname":"mattbox.losos.cfd","token":"<64 hex>"}}' \
VERCEL_DEV_PORT=3000 cargo run --bin registrar
curl -s localhost:3000/health
```

The page in `public/` is served by Vercel, not by the function; locally, open
it behind any static server that proxies the other paths to the binary.

## How it is kept apart

`backend-registrar` gained two things for this host, both additive and used
by nothing on the VPS: `server::build` (the router and reconciler without the
listener and the timer, which `serve` now calls) and
`Registry::{export,import}` (a snapshot with ages instead of monotonic
instants, so the registry can travel between processes). Everything
Vercel-specific — the bridge, the environment handling, the Redis store, the
status routes and the page — is in this crate, which `nix build` never sees.
Removing the Vercel deployment later is deleting this directory.
