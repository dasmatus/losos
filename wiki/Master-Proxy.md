# Master proxy

Optional. It makes the box reachable from the internet without opening a port
at home.

```
internet → Traefik (VPS, :443) → rathole server ⇐ tunnel ⇐ rathole client (box) → nginx
```

## Edge (VPS)

Import the `nixosModules.edge` flake output into the VPS configuration and set
`losos.edge.*`:

- `losos.edge.publicDomain` is the domain appliances are served under.
- `losos.edge.tenants` is the allow-list of appliances, each with a hostname
  and a token file. The registrar creates routes and certificates only for
  ids listed here.
- `losos.edge.cluster.enable` also runs the [mesh](Mesh) control plane, an
  rke2 server with Longhorn. The proxy works without it.

`losos-registrar` keeps Traefik's dynamic config and rathole's server config
in step with the registered appliances.

## Appliance

Set `losos.proxy.enable = true` and the `losos.proxy.*` options
(`registrarUrl`, `edgeRatholeEndpoint`, `hostname`, `applianceId`,
`tokenFile`).

## Tunnel encryption

rathole uses its Noise transport. The edge generates a key pair on first
boot. Each appliance fetches the public key from the registrar on first start
and stores it at `losos.proxy.noisePublicKeyFile`. To pin the key out of band,
put the file there beforehand; the appliance never overwrites it. Setting
either key option to `null` falls back to plain TCP. Don't.

## Admin UI stays LAN-only

Tunnel traffic reaches nginx from `127.0.0.1`, so the admin routes deny
loopback. Do not add `allow 127.0.0.1` to them, or the admin UI becomes
reachable from the internet.

## Edge on the same LAN

An edge does not have to be a VPS. With `losos.edge.lan.advertise = true` it
also announces itself over mDNS as `_losos-edge._tcp`, with the registrar URL
in a `url=` record (`losos.edge.lan.url`, default `http://<edge>.local:8443`).
It binds the registrar API off-loopback and opens its port. A box on the same
network then finds it with no configuration and may share storage through
it, see [Mesh](Mesh#finding-the-edge). This is what an on-premises
deployment looks like: one always-on machine on the company network running
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

The boxes need nothing. The ISO installs the published `install`
configuration, the Mesh pane shows "Edge proxy found: … On this network" once
the edge answers, and sharing can be switched on.

This is also how a LAN with no internet pools storage. A box grows its own
disk without any edge (`losos-ctl grow`, Local mode). Pooling across boxes is
the mesh, though, and the mesh's control plane (rke2 server, Longhorn, the
registrar) lives on the edge machine. So an edge on a LAN PC is what makes an
offline site work. The WAN can be unplugged, since mDNS is all the boxes use
to find it. Two limits today:

- The tunnel's enrolment address (`losos.proxy.registrarUrl`) is a
  build-time option that still defaults to the public edge. A site that
  wants the tunnel to end on its own edge sets it in its own flake.
- The advert is mDNS, so the edge and the boxes must share a broadcast
  domain (one VLAN).

`demo/edge-lan/run.sh` stands this whole arrangement up on one machine as VMs
on a virtual switch: the edge, which is also the LAN's router, and two boxes
installed from the ISO. It walks through the verification checklist
(`demo/edge-lan/CHECKLIST.md`). With no edge, both boxes refuse. The edge
appears, and both find it and allow sharing. The edge goes away, they refuse
again, and it comes back. Its README says which parts stand in for what on a
real site, and what the pooled-storage steps still need.

## Several edges: federation

A user-hosted edge can relay its boxes through an official edge, so two sites
each behind their own local edge reach each other over the internet, and a
box prefers its local edge, falls back to the official one and switches its
edge-dependent features off when neither answers. The design, the hub-side
tenant attribute (`relayZone`), the spoke-side `losos.edge.uplink.*`, open
LAN enrolment and the gateway VM image are on [Edge federation](Edge-Federation).

## Official edges

Any edge can be found and can relay storage. Only edges LosOS runs may
process P2P storage and compute trading (the [market](Market)). The box
checks that itself, on every scan, and a company's own edge gets everything
except the market.

The check is an Ed25519 identity:

- **The LosOS root key.** One keypair. The public half is a file every box
  ships, `keys/official-edge-root.pub` in this repository
  (`losos.proxy.officialRootKeyFile`). The project owner holds the private
  half offline. It never enters the repository or any box, and signs edge
  certificates and nothing else. Until someone writes the public key into
  that file, no edge is official and the market is off on every box built
  from the tree. The default fails closed.
- **An edge certificate.** Each official edge has its own keypair and a small
  JSON certificate `{name, url, public_key, not_after, signature}` signed by
  the root. The registrar answers `GET /identity?nonce=<hex>` with the
  certificate and a signature over the nonce by the edge's key.
- **The box's check** (`backend/src/edge.rs`) runs for every edge whose
  `/health` answered. The root signed the certificate, the certificate names
  the URL the box is talking to, it has not expired, and the edge's key
  signed the nonce the box just made up. All four pass or the edge is not
  official. The result is `official` per edge on `GET /api/edge`, and a sign
  beside each edge's name on the Mesh pane: a check for an official edge, a
  warning for any other, with a tooltip listing what that edge cannot do for
  the box. A box may have several edges in reach. Sharing works through any
  of them, trading only through an official one. While no official edge is
  in reach, the market relay answers `{available: false, reason:
  "noOfficialEdge"}` and refuses every action with 409
  `officialEdgeRequired`. Nothing leaves the box.

The owner runs the key ceremony **on a machine of their own** with the
registrar binary. Every step that makes or uses the root key first signs the
owner in with GitHub:

```sh
# once: the root. Keep root.key offline. --publish opens the pull request
# that writes the public key into keys/official-edge-root.pub.
losos-registrar provision root-keygen --out root.key --publish

# per edge: the edge's own public key read from GET /identity/public-key,
# a certificate signed by the root for the URL boxes will probe, pushed to
# POST /identity/cert with the sign-in's GitHub token, and the box's four
# checks run against the result. No SSH.
losos-registrar provision edge --name "LosOS edge Berlin" \
  --url https://register.losos.cfd --root-key root.key --days 365

# any time: the checks alone.
losos-registrar provision verify --url https://register.losos.cfd --root-key root.key
```

The edge needs nothing done by hand. The registrar makes its identity key on
its first start (`losos.edge.identity.keyFile`, default
`/var/lib/losos-registrar/identity.key`), and the private half never leaves
the edge. It writes the pushed certificate beside it
(`losos.edge.identity.certFile`).

`POST /identity/cert` is the one route that authenticates a GitHub account
rather than an appliance token. The edge asks GitHub whose token it was
handed and installs the certificate only if that account's numeric id is on
the operator allowlist compiled into the binary, the same list the tool
checked before pushing. The edge refuses a certificate for another key before
it asks GitHub, takes pushes one at a time, and writes nothing on a refusal.
Until a certificate arrives, the edge answers `/identity` with 404 and is not
official. A company's own edge looks like that, including the one
`demo/edge-lan/` boots. An edge with both options set to `null` serves no
identity at all. The tests build a root of their own at build time with the
lower-level commands behind `provision`, `losos-registrar identity
keygen|sign|show|verify`.

### Who may run the ceremony

`provision` carries the public client id of the project's GitHub OAuth App
and an allowlist of GitHub accounts, both compiled into the binary from
`backend-registrar/operators.json`. Each gated command prints a one-time
code, the operator enters it at github.com/login/device, and GitHub hands the
tool a token for that account. The tool asks GitHub who that is and compares
the account's *numeric id* with the list, because logins can be renamed and
ids cannot. Anyone else is refused before anything is made or signed (exit
3). There is no secret involved. The device flow needs no client secret, and
the tool stores no token. `LOSOS_GITHUB_TOKEN` (for example `gh auth token`)
skips the browser and goes through the same check. Changing who is allowed is
a pull request against that file, reviewed like the key itself.

The allowlist decides whom the tooling serves and makes each signing a named
act. The edge checks the same list on the push, so it also decides who can
change what an edge serves. The root key is still the whole secret. Anyone
holding `root.key` could sign with any Ed25519 tool, but could not install
the result on an edge without a listed account. The operator steps, including
creating the OAuth App, are in
[provisioning/edge-identity/README.md](https://github.com/dasmatus/losos/blob/main/provisioning/edge-identity/README.md).

There is no Forgejo Actions runner on the box, and Actions is off in LosOS
Git. The ceremony used to be two workflows there. That put the root key
inside the appliance's `/var`, where a reinstall lost it, and let any LosOS
Git account run code on the box.

What this protects: a company that runs its own edge gets a working
on-premises deployment but cannot settle trades through it. A stranger who
stands up an edge cannot make boxes trade through it either, because only the
root's private key makes an edge official. What it does not protect: the
market protocol is not hidden, and the owner's private key is the whole
secret. Losing it means shipping a release that re-keys every box's trust
anchor.

## DNS zone and custom domains

An official edge can serve a DNS zone of its own and route domains that box
owners already have. It is off unless `losos.edge.dns.enable` is set.

```nix
losos.edge.dns = {
  enable = true;
  zone = "boxes.losos.dasmat.us";   # default: "boxes.${publicDomain}"
  ipv4 = [ "203.0.113.7" ];         # the edge's public addresses; one family at least
  ipv6 = [ "2001:db8::7" ];
  # nameservers defaults to [ "ns1.<zone>" ], served with glue from ipv4/ipv6.
};
```

The registrar renders the zone file on every reconcile and Knot serves it on
port 53, which the module opens. A path unit reloads Knot when the file
changes. The zone carries SOA, NS and glue, every tenant hostname that falls
inside the zone, and one name per box whose Stripe account is vouched for:
16 hex characters of a SHA-256 of the box UUID. Every name resolves to the
edge, because that is where Traefik and the tunnel are. A box's own address
never appears in public DNS, and neither does a Stripe account id or a UUID.
The serial is the Unix time of the change, or one more than the previous
serial if that is larger.

The operator delegates the zone once, at the registrar of the parent domain:

```
boxes.losos.dasmat.us.      NS  ns1.boxes.losos.dasmat.us.
ns1.boxes.losos.dasmat.us.  A   203.0.113.7
ns1.boxes.losos.dasmat.us.  AAAA 2001:db8::7
```

Nothing is handed out until the edge holds an unexpired identity certificate
([Official edges](#official-edges)). Before that the zone is only SOA, NS and
glue, and `/domains/*` answers 503.

### Custom domains

A box adds a domain over `POST /domains/add` (authenticated with its tunnel
token, relayed by lososd from Settings → Network). The edge accepts it only
when the box's Stripe connected account is ready and carries the box UUID,
the same check the market makes before paying out. Then the owner publishes
two records:

- `_losos-challenge.<domain> TXT "losos-domain-v1=<32 hex>"`, where the hex
  is a SHA-256 over the domain, the box UUID and the Stripe account id. It
  proves control of the domain and binds it to that box and that account.
  A record left behind by a previous owner proves nothing.
- `<domain> CNAME <label>.<zone>`, or A/AAAA records with the edge's
  addresses at a zone apex.

The registrar looks both up over DNS-over-HTTPS (`losos.edge.dns.checkUrl`,
Cloudflare's JSON API by default), at most 16 per pass, every 30 s while a
claim waits and hourly once it is live. Only when both records are right does
the domain get a Traefik router with Let's Encrypt, routed through the
box's tunnel. Waiting for the CNAME too keeps Traefik from asking Let's
Encrypt for a name that does not reach the edge. A live domain lapses after
three failed rechecks in a row, or at once when the Stripe account stops
being ready. A box may hold five domains, a domain belongs to one box, and an
unverified claim is dropped after seven days. Claims live in
`/var/lib/losos-registrar/domains.json`.

On the box, lososd keeps the live domains in
`/var/lib/losos-public-names/domains.json`. It refreshes the file every two
minutes and whenever the Network pane asks. The Nextcloud pod mounts that
directory read-only, adds the names to `trusted_domains` per request, and
uses a request's own domain for `overwritehost` when it is one of them. Only
container mode does this. LosOS Git keeps its edge-name `ROOT_URL`.

`tests/edge-dns.nix` (`losos-edge-dns`) boots an edge and a client and asks
Knot over UDP and TCP. `backend-registrar/tests/domains.rs` drives the claim
flow against a real registrar and a fake DNS-over-HTTPS server.

## Demo deployment on Vercel

`edge-vercel/` runs the registrar's API, the same router and the same token
check, as a Vercel Function at `https://losos-edge.dasmat.us`, which is the
default `losos.proxy.registrarUrl`. The registry lives in a Neon Postgres,
and a status page at `/` shows the registered boxes and the Traefik
configuration the edge would write for them. It is the control plane only.
The rathole tunnel, Traefik's TLS, the mesh and the market cannot run on a
serverless host, so the data path, reaching a box through its public
hostname, still needs the VPS. The page also lists ids and hostnames to
anyone with the URL. It is a demonstration host, not an edge. Deployment
steps and the environment variables are in
[edge-vercel/README.md](https://github.com/dasmatus/losos/blob/main/edge-vercel/README.md).
