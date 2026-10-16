# The edge in `losos`

Operator notes, kept out of the handbook. The envelope is the one in
[terminal-access.md](terminal-access.md); this page lists what differs on an
edge (`edge/registrar/src/operator.rs`).

## Seeding and tokens

The operator's public key is `losos.edge.access.gpgKeys` in the edge's NixOS
configuration. With the list empty (the default) the edge serves no
`/operator/*` route. The edge has no web UI, so its tokens come from the
operator's machine, behind the same GitHub sign-in and allowlist as the
certificate push (`edge/registrar/operators.json`):

```sh
losos-registrar provision token create --edge https://register.<domain> \
  --name laptop --key <fingerprint> [--scope read|full] \
  [--expires-in-days D | --never-expires]
losos-registrar provision token list   --edge https://register.<domain>
losos-registrar provision token revoke --edge https://register.<domain> <id>
```

`create` makes a full token that expires in 90 days unless told otherwise,
and prints it on stdout, once. The verbs call `POST /operator/tokens`,
`/operator/tokens/list` and `/operator/tokens/revoke` with
`Authorization: Bearer <GitHub token>`. The bodies and answers are those of
the box's `/api/access/tokens` routes; the list also carries `hostKey` and
`keys`.

## Routes

```
GET  /operator/host-key      {fingerprint, publicKey}, unauthenticated
POST /operator/call          the envelope; at most 256 KiB
```

Inside the envelope `path` is one of the six below. Any other path gets a
sealed 404, another method on one of them a sealed 405. As on a box, the
HTTP status of a sealed answer is the inner one.

| method | path | scope | answer |
|---|---|---|---|
| GET | `/overview` | read | `{health, version, identity, tenants, enrolled, routes, domains, optIns}` |
| GET | `/secrets` | read | `[{name, sealed, updated}]`, one per `losos.edge.credentials.secrets` entry |
| POST | `/secrets` | full | `{name, value}`: seal one; answers `{name, sealed, report}` or 400 with the script's words |
| POST | `/reconcile` | full | wake the reconciler now; `{reconciling: true}` |
| GET | `/escrow/releaser` | read | `{address, networks: [{network, escrow, balanceWei, escrowAgrees, error}]}`; `address` null until a key is sealed |
| POST | `/escrow/releaser` | full | `{replace?}`: make the escrow key here and seal it; answers `{address, replaced, report}`, 409 while one is sealed and `replace` is not set |

In `/overview`, `identity` is null without an identity key, else
`{official}` plus `name`, `url` and `notAfter` (Unix seconds) once a
certificate is installed. `official` means the certificate is installed and
unexpired; the edge does not check the root's signature on itself, the
boxes do. `tenants` is `[{id, hostname, seenAgoSecs, via}]`. `enrolled`,
`routes` and `domains` are counts, or null where that opt-in is off.

Both secret paths run `losos-seal-credential`, the script the Actions
workflow reaches over SSH, through `systemd-run --pipe --wait`. That puts it
outside the registrar's sandbox (the registrar cannot see the Stripe gate's
blobs) and outside its cgroup. `GET` runs `losos-seal-credential --list`,
which prints names and modification times only. `POST` passes the value on
the script's stdin, never in an argument, the environment or a file, and the
script checks its shape against the entry's `pattern` before it seals
anything. When an entry's `units` include `losos-registrar.service`, the
script restarts it three seconds later from a transient timer
(`LOSOS_SEAL_DEFER`), so the answer goes out first.

`POST /escrow/releaser` is the one secret the edge makes rather than takes.
The registrar draws a secp256k1 key from the system's CSPRNG, hands its hex
to the same script as `usdc-releaser-key` on stdin, and answers only the
address; `POST /secrets` refuses that name, so the key never existed
anywhere else. `GET /escrow/releaser` reads the loaded key's address and asks
each EVM network for its balance and the escrow's `releaser()`. Both answer
404 on an edge that settles USDC on no EVM network.

Every credential the registrar loads (`usdc-rpc-*`, `usdc-releaser-key`,
`ens-rpc`) has a `SetCredential=` fallback of `unsealed`, which the
registrar reads as "not configured". Without it systemd would refuse to
start the unit while a blob is missing, and the terminal would have nothing
to seal it through.

## The client

`losos login https://register.<domain>` notices an edge by its
`/operator/host-key`. `losos edge secret set <name>` reads the value
without echoing it, or from stdin in a script. `losos edge escrow releaser`
shows the escrow key's address and gas, and `--new` or `--replace` makes it
on the edge. `losos ci setup --edge <url>`
gives a GitHub repository its own key and token for the edge
(`admin-ui/tui/README.md`).
