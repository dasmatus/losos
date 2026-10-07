# Master proxy

Optional. Makes the box reachable from the internet without opening a port at
home.

```
internet → Traefik (VPS, :443) → rathole server ⇐ tunnel ⇐ rathole client (box) → nginx
```

## Edge (VPS)

Import the `nixosModules.edge` flake output into the VPS configuration and set
`losos.edge.*`:

- `losos.edge.publicDomain` — the domain appliances are served under.
- `losos.edge.tenants` — the allow-list of appliances, each with a hostname
  and a token file. The registrar only creates routes and certificates for
  ids listed here.
- `losos.edge.cluster.enable` — also run the [mesh](Mesh) control plane
  (rke2 server with Longhorn). The proxy works without it.

`losos-registrar` keeps Traefik's dynamic config and rathole's server config
in sync with the registered appliances.

## Appliance

Set `losos.proxy.enable = true` and the `losos.proxy.*` options
(`registrarUrl`, `edgeRatholeEndpoint`, `hostname`, `applianceId`,
`tokenFile`).

## Tunnel encryption

rathole uses its Noise transport. The edge generates a key pair on first
boot. Each appliance fetches the public key from the registrar on first start
and stores it at `losos.proxy.noisePublicKeyFile`. To pin the key out of band,
put the file there beforehand; it is never overwritten. Setting either key
option to `null` falls back to plain TCP. Don't.

## Admin UI stays LAN-only

Tunnel traffic reaches nginx from `127.0.0.1`. The admin routes therefore
deny loopback. Do not add `allow 127.0.0.1` to them, or the admin UI becomes
reachable from the internet.

## Edge on the same LAN

An edge does not have to be a VPS. With `losos.edge.lan.advertise = true` it
also announces itself over mDNS (`_losos-edge._tcp`, with the registrar URL
in a `url=` record, default `http://<edge>.local:8443`, `losos.edge.lan.url`),
binds the registrar API off-loopback and opens its port, so a box on the same
network finds it with no configuration and may share storage through it — see
[Mesh](Mesh#finding-the-edge). This is the shape of an **on-premises
deployment**: one always-on machine on the company network running
`nixosModules.edge`, and boxes installed from the stock ISO beside it.

```nix
# the edge machine's configuration
imports = [ losos.nixosModules.edge ];
losos.edge = {
  enable = true;
  acmeEmail = "ops@example.com";       # a public name gets a certificate as usual
  lan.advertise = true;                # announce on the LAN, bind the API off-loopback
  tenants.<id>.tokenFile = "/run/secrets/tenant-<id>";
};
```

The boxes need nothing: the ISO installs the published `install`
configuration, the Mesh pane shows "Edge proxy found: … On this network" once
the edge answers, and sharing can be switched on. **This is also how storage
is pooled on a LAN with no internet**: a box's own disk grows without any edge
(`losos-ctl grow`, Local mode), but pooling across boxes is the mesh, whose
control plane (rke2 server, Longhorn, the registrar) lives on the edge
machine, so the edge on a LAN PC is what makes an offline site work. The
WAN can be unplugged; mDNS is all the boxes use to find it. Two limits today: the
tunnel's enrolment address (`losos.proxy.registrarUrl`) is a build-time
option that still defaults to the public edge, so a site that wants the
tunnel to terminate on its own edge sets it in its own flake; and the advert
is mDNS, so the edge and the boxes must share a broadcast domain (one VLAN).

`demo/edge-lan/run.sh` stands this whole arrangement up on one machine as
VMs on a virtual switch — the edge (as the LAN's router too), two boxes
installed from the ISO — and walks through the verification checklist
(`demo/edge-lan/CHECKLIST.md`): no edge → refused on both → the edge appears
→ found and allowed on both → edge gone → refused → back. Its README says
which parts stand in for what on a real site, and what the pooled-storage
steps still need.

## Official edges

Any edge can be found and can relay storage. **Only edges LosOS runs may
process P2P storage and compute trading** (the [market](Market)); the box
checks that itself, on every scan, and a company's own edge gets everything
except the market.

The check is an Ed25519 identity:

- **The LosOS root key.** One keypair. The public half is a file every box
  ships, `keys/official-edge-root.pub` in this repository
  (`losos.proxy.officialRootKeyFile`). The private half is held offline by
  the project owner, never enters the repository or any box, and is used
  only to sign edge certificates. Until the public key is written into that
  file, no edge is official and the market is off on every box built from
  the tree: the default fails closed.
- **An edge certificate.** Each official edge has its own keypair and a small
  JSON certificate `{name, url, public_key, not_after, signature}` signed by
  the root. The registrar serves `GET /identity?nonce=<hex>` with the
  certificate and a signature over the nonce by the edge's key.
- **The box's check** (`backend/src/edge.rs`), for every edge whose `/health`
  answered: the root signed the certificate, the certificate names the URL
  the box is talking to, it is not expired, and the nonce the box just made
  up is signed by the certificate's key. Four checks, all four or nothing;
  the result is `official` per edge on `GET /api/edge` and a sign beside
  each edge's name on the Mesh pane: a check for an official edge, a warning
  for any other, with a tooltip listing what that edge cannot do for the box.
  A box may have several edges in reach; sharing works through any of them,
  trading only through an official one. While no official edge is in reach
  the market relay answers `{available: false, reason: "noOfficialEdge"}`
  and refuses every action with 409 `officialEdgeRequired`; nothing leaves
  the box.

The key ceremony is run by the owner **on a machine of their own** with the
registrar binary, and every step that makes or uses the root key first signs
the owner in with GitHub:

```sh
# once: the root. Keep root.key offline. --publish opens the pull request
# that writes the public key into keys/official-edge-root.pub.
losos-registrar provision root-keygen --out root.key --publish

# per edge: a key pair made in memory, a certificate signed by the root for
# the URL boxes will probe, both shipped to the VPS over one SSH session,
# the registrar restarted, and the box's four checks run against it.
losos-registrar provision edge --name "LosOS edge Berlin" \
  --url https://register.losos.cfd --ssh root@edge.example \
  --root-key root.key --days 365

# after the edge's configuration names the two files (below): the checks alone.
losos-registrar provision verify --url https://register.losos.cfd --root-key root.key
```

On the edge: `losos.edge.identity.keyFile = "/var/secrets/losos-edge-identity.key"`
and `losos.edge.identity.certFile = "/etc/losos/edge-identity.cert.json"`,
the two paths `provision edge` installs. The registrar refuses to start if
the certificate is not for that key. An edge with neither option (the
default, and the shape of a company's own edge, including the one
`demo/edge-lan/` boots) answers `/identity` with 404 and is simply not
official. The primitives behind `provision` (`losos-registrar identity
keygen|sign|show|verify`) are what the tests use to build a root of their
own at build time.

### Who may run the ceremony

`provision` carries the public client id of the project's GitHub OAuth App
and an **allowlist** of GitHub accounts, both compiled into the binary from
`backend-registrar/operators.json`. Each gated command prints a one-time
code, the operator enters it at github.com/login/device, and GitHub hands
the tool a token for that account; the tool asks GitHub who that is and
compares the account's *numeric id* (logins can be renamed; ids cannot) with
the list. Anyone else is refused before anything is made or signed (exit 3).
No secret is involved: the device flow needs no client secret, and no token
is stored. `LOSOS_GITHUB_TOKEN` (for example `gh auth token`) skips the
browser; it is checked the same way. Changing who is allowed is a pull
request against that file, reviewed like the key itself.

The gate decides whom the tooling serves and makes each signing a named
act; the root key stays the whole secret, and anyone holding `root.key`
could sign with any Ed25519 tool. The operator steps, including creating
the OAuth App, are in
[provisioning/edge-identity/README.md](https://github.com/dasmatus/losos/blob/main/provisioning/edge-identity/README.md).
There is no Forgejo Actions runner on the box and Actions is off in LosOS
Git: the ceremony used to be two workflows there, which put the root key
inside the appliance's `/var` (a reinstall lost it) and let any LosOS Git
account run code on the box.

What this does and does not protect: a company that runs its own edge gets
a working on-premises deployment and cannot settle trades through it, and a
stranger who stands up an edge cannot make boxes trade through it, because
nothing short of the root's private key makes an edge official. It does not
hide the market protocol, and the owner's private key is the whole secret:
losing it means re-keying every box's trust anchor with a release.

## Demo deployment on Vercel

`edge-vercel/` runs the registrar's API — the same router and the same
token check — as a Vercel Function at `https://losos-edge.dasmat.us` (the
default `losos.proxy.registrarUrl`), with the registry in a Neon Postgres
and a status page at `/` that shows the registered boxes and the Traefik
configuration the edge would be writing for them. It is the control plane
only: the rathole tunnel, Traefik's TLS, the mesh and the market cannot run
on a serverless host, so the data path (reaching a box through its public
hostname) still needs the VPS. The page also lists ids and hostnames to anyone
with the URL; it is a demonstration host, not an edge. Deployment steps and
the environment variables are in
[edge-vercel/README.md](https://github.com/dasmatus/losos/blob/main/edge-vercel/README.md).
