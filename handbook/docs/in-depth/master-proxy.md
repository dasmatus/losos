---
title: Master proxy
sidebar_position: 5
mdx:
  format: md
---

# Master proxy

Optional. It makes the box reachable from the internet without opening a port
at home.

![internet → Traefik (VPS, :443) → tunnel server ⇐ QUIC tunnel ⇐ tunnel client (box) → nginx](/img/diagrams/master-proxy-en.svg)

## Edge (VPS)

Import the `nixosModules.edge` flake output into the VPS configuration and set
`losos.edge.*`:

- `losos.edge.publicDomain` is the domain appliances are served under.
- `losos.edge.tenants` is the allow-list of appliances, each with a hostname
  and a token file. The registrar creates routes and certificates only for
  ids listed here.
- `losos.edge.cluster.enable` also runs the [mesh](mesh.md) control plane, an
  rke2 server with Longhorn. The proxy works without it.
- `losos.edge.tang.enable` (on by default) lets boxes installed with
  `--tang-edge` open their disk through the edge, with a Tang key pair per
  box under `/var/lib/losos-registrar/tang` and a page at `/tang/lost` where
  an owner reports a stolen box. [TPM and the disk key](../reference/tpm.md)
  has the box side.

`losos-registrar` keeps the tunnel server's file in step with the registered
appliances, and Traefik finds them through the registrar.

## How Traefik finds the boxes

Traefik discovers the boxes through its HTTP provider. Every two seconds it
asks the registrar, on a loopback-only port (`losos.edge.traefikProviderPort`,
default 8090), for the current routes: one per live box name, custom domain
and published machine, each pointing at that box's end of the tunnel. A box
that registers is reachable within seconds, and one whose heartbeats stop
loses its route when the registrar prunes it. The registrar writes nothing
into Traefik's directory, which holds only the routes NixOS manages.

If the registrar stops or restarts, Traefik keeps the routes it last
received, so the boxes stay reachable. Upgrading an edge that still has the
`losos.yml` and `losos-vms.yml` files an older registrar wrote removes them
on the switch.

## HTTP/3

The public entry point answers HTTP/3 over QUIC on UDP port 443 as well as
HTTP/1.1 and HTTP/2 on TCP port 443. Traefik announces HTTP/3 in an
`Alt-Svc` header, so a browser moves to QUIC after its first request. A
network that blocks UDP keeps working over TCP. Open UDP 443 in any
firewall in front of the edge; the edge's own firewall opens it.

A box that ends TLS itself (see
[The edge cannot read the traffic](#the-edge-cannot-read-the-traffic)) is
served over HTTP/2 and HTTP/1.1 on TCP only. HTTP/3 stays on the edge's own
names.

## When a box can't be reached

When Traefik cannot reach a box (it is switched off, restarting or offline,
or its tunnel is down), the visitor gets a page from the edge that says so,
in English, Slovak or German after the browser's language, instead of a bare
"Bad Gateway". The status stays 502, or 504 when the box did not answer in
time.

The page needs the edge to end TLS for the name, so it is shown only for
names the edge still ends TLS for. Once a box ends TLS itself, a visitor to
a box that is off gets the browser's own connection error instead.

## Appliance

Set `losos.proxy.enable = true` and the `losos.proxy.*` options
(`registrarUrl`, `edgeTunnelEndpoint`, `hostname`, `applianceId`,
`tokenFile`).

## The tunnel

The box keeps one QUIC connection to the edge, on UDP port 2333
(`losos.edge.tunnelBindPort`). Each visitor's connection travels as its own
stream inside it, so a slow download does not hold up other requests, a new
request needs no new handshake, and the connection survives the box's
address changing. The box only dials out, so it works behind NAT and
carrier-grade NAT with no port forward. A keepalive every 10 seconds holds
the NAT mapping open.

Where a network drops UDP, the box gives QUIC five seconds and then dials
the same port over TCP: TLS 1.3 with the same certificate, carrying HTTP/2
streams. It tries QUIC again every five minutes and moves back when it gets
through. The edge's firewall opens the port for both.

## Tunnel encryption

The tunnel is TLS 1.3 on both transports. The edge makes a self-signed
certificate on its first start and keeps it in `/var/lib/losos-tunnel`. Each
appliance fetches the certificate's SHA-256 from the registrar
(`GET /tunnel-pin`) before its first dial and stores it at
`losos.proxy.tunnelPinFile`; from then on the tunnel accepts that
certificate and no other. To pin it out of band, put the file there
beforehand; the appliance never overwrites it. Each service the box serves
is claimed with the box's token, compared in constant time.

## The edge cannot read the traffic

The box gets its own Let's Encrypt certificate for each of its public names
and ends TLS itself. The edge routes each connection by the name in the TLS
hello and passes the encrypted bytes through to the box without opening
them, so whoever runs the edge, or the machine it runs on, cannot read
sign-ins, files or pages. This is on by default
(`losos.proxy.tlsPassthrough.enable`).

1. The edge routes `http://<name>/.well-known/acme-challenge/` on port 80
   to every box, so the box can answer Let's Encrypt's HTTP-01 challenge
   through the tunnel. The box asks with `lego` (`losos-public-tls`, every
   hour and whenever the edge names a new name) and keeps the certificates
   in `/var/lib/losos-public-tls/live/<name>/`.
2. The box's heartbeat lists the names it holds a valid certificate for.
   Until the first one is there, the edge keeps ending TLS as before, so the
   move costs no downtime.
3. For those names the edge drops its own TLS router and keeps a TCP router
   with `HostSNI` and passthrough. It asks for no certificate of its own.
4. On the box, nginx reads the first bytes of each tunnel connection: TLS
   goes to the box's own certificate, plain HTTP (the challenges, an older
   edge) to port 80 as before. The admin UI stays LAN-only on both.

Once a box has asked for passthrough, a certificate that lapses does not
hand TLS back to the edge. Visitors then see a certificate warning until
the box renews it.

Every hour the box also opens each of its names from outside, through
public DNS and the edge, and compares the certificate it gets with its own.
The Mesh page shows the result: **Encrypted to this box** when they match,
**Readable** while the edge still ends TLS, and **Intercepted** when a
certificate the box did not make answers. That last one means something
between visitors and the box is ending their TLS, for example an edge that
got a certificate of its own for the name. Such a certificate also shows up
in the public Certificate Transparency logs.

What the edge still sees: the name each visitor asks for, when, and how
much data passes. It also learns the box's home address from the tunnel.
Passthrough costs three things on the box's names: the offline page,
HTTP/3, and Encrypted Client Hello, since each of them needs the edge to
hold the keys. The edge's own names keep all three.

Custom domains an edge routes to a box are covered the same way: the
heartbeat's answer tells the box every name routed to it, and the box asks
for a certificate for each. Across [federation](edge-federation.md) the
spoke forwards the box's list, and the hub routes `<spoke>.<box>` by name to
the spoke, which passes it on to the box.

:::note[How this is tested]
`nix/tests/edge-passthrough.nix` (`losos-edge-passthrough`) runs a test ACME
server, an edge and a box. Before the box has a certificate, a capture of
the edge's side of the tunnel shows a password a visitor posts. After the
box gets its certificate through the edge, the visitor checks it against
the ACME server's root, Traefik has only a passthrough route for the name,
and the same password is nowhere in the capture. Pointing the name at
another machine turns the box's own check to **Intercepted**.
:::

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
it, see [Mesh](mesh.md#finding-the-edge). This is what an on-premises
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
to find it.

:::note[Two limits today]
- The tunnel's enrolment address (`losos.proxy.registrarUrl`) is a
  build-time option that still defaults to the public edge. A site that
  wants the tunnel to end on its own edge sets it in its own flake.
- The advert is mDNS, so the edge and the boxes must share a broadcast
  domain (one VLAN).
:::

`nix/demo/edge-lan/run.sh` stands this whole arrangement up on one machine as VMs
on a virtual switch: the edge, which is also the LAN's router, and two boxes
installed from the ISO. It walks through the verification checklist
(`nix/demo/edge-lan/CHECKLIST.md`). With no edge, both boxes refuse. The edge
appears, and both find it and allow sharing. The edge goes away, they refuse
again, and it comes back. Its README says which parts stand in for what on a
real site, and what the pooled-storage steps still need.

## Several edges: federation

A user-hosted edge can relay its boxes through an official edge, so two sites
each behind their own local edge reach each other over the internet, and a
box prefers its local edge, falls back to the official one and switches its
edge-dependent features off when neither answers. The design, the hub-side
tenant attribute (`relayZone`), the spoke-side `losos.edge.uplink.*`, open
LAN enrolment and the gateway VM image are on [Edge federation](edge-federation.md).

## Official edges

Any edge can be found and can relay storage. Only an edge that passes the
gateway checks may process P2P storage and compute trading (the
[market](market.md)). Anyone may run such an edge, an open gateway, as long
as it proves what it runs. The box checks that itself, on every scan, and an
edge that does not pass gets everything except the market.

What a box asks of an edge:

- **A passport.** The edge's owner signs a short statement with their
  OpenPGP key: the edge's URL, the edge's own Ed25519 key (made on the edge;
  the private half never leaves it), where the owner's share of the fee goes
  on each network, and an expiry. The box checks the signature and the key
  (self-signatures, not revoked, not expired) and shows the key's user id as
  the edge's owner.
- **The fees.** The settings beside the passport split each sale 2% to LosOS
  and 2% to the owner, and LosOS's addresses are exactly the ones the box
  ships (`losos.proxy.gateways.payouts`).
- **A TPM.** The edge's TPM endorsement key carries its maker's certificate,
  and the box checks it against the makers it ships
  (`losos.proxy.gateways.tpmRoots`, Microsoft's TrustedTpm bundle by
  default). The box makes a credential only that TPM can open.
- **A quote.** The TPM quotes PCRs 4, 11 and 15 over the box's fresh nonce
  and the edge's key. PCRs 4 and 11 must show one of the published gateway
  images (`losos.proxy.gateways.images`), and PCR 15 must hold the hash of
  exactly the settings the edge sent. The edge measures its settings at boot,
  before the registrar starts, and PCR 15 cannot be reset, so the quote shows
  the settings that boot started with.

Two round trips carry it: `GET /passport?nonce=<hex>` and
`POST /passport/attest`. The box's half is `backend/access/src/gateway/check.rs`,
the edge's half `edge/registrar/src/gateway.rs`.

The result is `official` per edge on `GET /api/edge`, with the gateway's
state beside it, and a sign beside each edge's name on the Mesh pane: a
check for a checked gateway, with its owner and image in the tooltip; a
warning for any other edge, with a tooltip listing what that edge cannot do
for the box; a stop sign for an edge the box caught tampered. A box may have
several edges in reach. Sharing works through any of them, trading only
through a checked one. While none is in reach, the market relay answers
`{available: false, reason: "noOfficialEdge"}` and refuses every action with
409 `officialEdgeRequired`. Nothing leaves the box.

A refusal is one of two kinds. Something the box does not know yet, such as
an image it has no measurements for, a TPM maker it has no root for or a fee
address that differs from the ones it ships, is shown and asked again on the
next scan. Proof of tampering, such as a forged attestation, settings that
changed after the edge booted, or firmware that started a program outside
the published image, disables the edge
on that box for good, by its URL, its TPM and its owner's key, in
`/var/lib/losos/gateways.json`. The box then uses that edge for nothing,
sharing included. A checked gateway is believed for ten minutes before the
box asks again.

The owner of an edge claims it with `POST /passport/settings`, signed with
their OpenPGP key (`losos-registrar passport statement` prints the text to
sign, `passport settings` assembles the file). The edge writes the file and
reboots to measure it. Until then it answers `/passport` with 404 and is not
checked. An edge without `losos.edge.passport.enable` serves no passport at
all; a company's own edge usually looks like that, including the one
`nix/demo/edge-lan/` boots.

Two lists ship empty in this tree: `nix/gateway/images.nix` until a gateway
image boots as a unified kernel image, and `nix/gateway/payouts.nix` until
LosOS's addresses are written there. While either is empty no edge passes,
and the market is off on every box. The default fails closed.

So a company that runs its own edge gets a working on-premises deployment,
and it can trade through that edge too once the edge runs the published
image under its passport and pays LosOS's share. A stranger who changes the
image, the settings or the fee addresses gets an edge no box trades through.

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

The zone is signed with DNSSEC ([below](#wallets-at-box-names)). Publish
its DS record at the same registrar, so resolvers trust the signatures:
`keymgr <zone> ds` on the edge prints it. Until the DS is there, resolvers
treat the zone as unsigned, and a browser has no reason to trust the ECH key
below either.

:::note[Only on a claimed gateway]
The zone needs `losos.edge.passport.enable`, and nothing is handed out until
the edge's owner has claimed it, with settings signed by the owner's key and
measured at this boot. Before that the zone is only SOA, NS and glue, and
`/domains/*` answers 503.
:::

### Encrypted Client Hello

Every name in the zone also carries an HTTPS record with the edge's Encrypted
Client Hello (ECH) key:

```
mattbox IN HTTPS 1 . alpn="h3,h2" ech="AEz+DQBI…"
```

A browser that finds the record encrypts the part of its TLS hello that names
the box, and shows the network only the edge's public name,
`register.<publicDomain>` unless `losos.edge.dns.ech.publicName` names
another. Someone watching the line sees that the browser went to the edge,
not which box it asked for. ECH is on with the zone
(`losos.edge.dns.ech.enable`), and the module refuses a zone without DNSSEC
or without ECH: an unsigned record could be changed on its way to the
browser.

The edge makes the key on its first start (`losos-ech-key`,
`/var/lib/losos-ech/ech.pem`, readable by root and Traefik) and keeps it
until the public name changes. `losos-registrar ech-key --key-file …
--public-name …` prints the value the records carry. Traefik decrypts with
the key. No released Traefik can do that yet, so an edge with the zone runs
Traefik built from its ECH change (`nix/traefik-ech.nix`), which serves no
dashboard.

A box holds an official edge to this. An edge that passes every gateway
check stays trusted only at an `https://` URL whose name has an HTTPS record
that validates under DNSSEC and carries an ECH key. lososd asks `delv` on
every scan, which validates from the DNS root's key itself, and lists an edge
that fails as refused (`domainNotHttps`, `domainUnsigned` or `domainNoEch`)
until it passes. `losos-ctl edge-domain <url>` runs the same check by hand.
The URL is the one the edge's passport names, so that name belongs in the
zone: an edge whose zone is its whole `publicDomain` gets
`register.<publicDomain>` there with the rest. `losos.proxy.gateways.domain`
turns the check off, or points it at another resolver and trust anchor.

:::note[How this is tested]
`nix/tests/edge-dns.nix` (`losos-edge-dns`) reads the key from the signed
HTTPS record, validates it with `delv`, and opens TLS to a box's name with
Go's ECH client. Traefik accepts ECH, the captured hello carries only the
public name, and a key Traefik does not hold is turned down.
`losos-ctl edge-domain` passes a box's name, finds no key at a name the zone
proves absent, validates nothing from a trust anchor that does not match,
and refuses plain HTTP.
:::

A box that ends TLS itself gets no HTTPS record at its names, because only
the holder of the ECH key can decrypt the hello and the edge passes that
box's connections through unopened. Its name then travels in the clear in
the TLS hello, as it did before ECH.

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
minutes and whenever the Network pane asks. LosOS Login reads it to give
its session cookie a domain for each of those names
(`modules/identity.nix`). LosOS Cloud and LosOS Git answer on whichever
name a request came by, so they need no list.

A box behind a local edge gets its domains routed too, through that local
edge, once it has vouched for the local edge with a relay pass and joined
this edge's mesh. The registrar keeps those routes itself, in
`relay-routes.json` beside its registry. [Edge
federation](edge-federation.md#custom-domains-behind-a-local-edge) has the
details and `losos.edge.dns.relayRoutes`.

### Wallets at box names

A box binds its wallet to its names with `POST /domains/wallet`
(`{address, signature}`, relayed by lososd from Settings → Network). The
signature is the wallet's `personal_sign` of

```
LosOS box name v1
zone: <zone>
box: <box id>
address: <0x address, lowercase>
```

so it binds nothing on another edge or for another box. The registrar
recovers the signer, keeps the address and the signature in `domains.json`,
and writes one TXT record at each of the box's names in the zone, its
hostname and its label:

```
mattbox IN TXT "ENS1 dnsname.ens.eth 0x…"
```

That is ENS's gasless DNS resolution (ENSIP-17): a wallet asks ENS on
Ethereum mainnet, ENS's DNSSEC gateway fetches the TXT record, a contract
checks the signatures up to the DNS root, and `dnsname.ens.eth`, ENS's first
DNS resolver, answers the address. It answers `addr` ABI-encoded, the way
every wallet decodes it. Its successor, `2.dnsname.ens.eth`, reads per-chain
keys (`a[e8453]=…`) but answers a bare 20 bytes, which viem's default lookup
and ethers fail to decode, so a buyer's wallet would find nothing. The box's
wallet is one address on every EVM network, so the Ethereum answer is the one
to give. `{}` unbinds.

The call carries the box's token, so only the box can bind or unbind, and
lososd asks it only for an administrator (the `network` section). The
signature shows the wallet agreed to this box's name. It cannot stop the
edge, which signs the zone, from serving something else. The name is as
trustworthy as the edge.

ENS only reads a signed zone, so Knot signs this one
(`losos.edge.dns.dnssec.enable`, on by default) with its default policy,
ECDSA P-256 with keys in `/var/lib/knot`. Resolvers trust the signatures once
the parent zone publishes the zone's DS record, which `keymgr <zone> ds`
prints on the edge. Publish it next to the NS records above.

Knot answering is not a wallet finding the name. While a wallet is bound,
the reconciler asks the DNS-over-HTTPS resolver (`losos.edge.dns.checkUrl`)
for the TXT record at each of the box's names, as ENS's gateway would, and
reports it per name in `wallet.published`: `found` once the record validates
(the resolver's AD bit), `unsigned` while the parent zone publishes no DS,
`missing` while it does not delegate the zone, `other` while a resolver still
holds an older record. A name not found yet is looked up again every half
minute, a found one hourly. The Network pane shows it beside each name.

The box's own wallet resolves names through the relay its chain reads use
(`/api/market/rpc` with `network: "ens"`). A DNS name's lookup has an
offchain step (EIP-3668): the resolver reverts with the gateway's URL, the
page sends `ens_ccipRead` with the sender and the calldata, and the edge
posts them to `https://dnssec-oracle.ens.domains/`, the only gateway it asks,
and hands back the answer, which the contract checks. The admin page's CSP
still lets it talk to its own box and nothing else.

An owner's own domain gets the same record shown in the pane, to add at the
domain itself. While a wallet is bound, each check also looks up TXT at the
domain and reports `wallet`: `found` (the whole record as the edge publishes
it, DNSSEC-validated by the resolver), `unsigned`, `cname` (a CNAME can
carry no TXT, and ENS reads only the exact name), `other` or `missing`. It never decides whether the domain
is routed.

:::note[How this is tested]
`nix/tests/edge-dns.nix` (`losos-edge-dns`) boots an edge and a client and asks
Knot over UDP and TCP, binds a wallet and validates its signed record with
`delv` from the zone's DS. `nix/tests/box-wallet-name.nix`
(`losos-box-wallet-name`) adds a box with LosOS Login and a laptop: the owner
binds the wallet in Chromium on the Network pane, through lososd, and the
record validates. A stand-in for public DNS on the edge then answers
nothing, an unsigned record and a validated one as the test delegates the
zone and publishes its DS, and the pane follows each step.
`edge/registrar/tests/domains.rs` drives the claim and wallet flows against a
real registrar and a fake DNS-over-HTTPS server.
`admin-ui/app/tests/ens.unit.mjs` replays, through the relay, a DNS name's
lookup recorded from Ethereum mainnet and ENS's gateway.
:::

## Demo deployment on Vercel

`edge/vercel/` runs the registrar's API, the same router and the same token
check, as a Vercel Function at `https://losos-edge.dasmat.us`, which is the
default `losos.proxy.registrarUrl`. The registry lives in a Neon Postgres,
and a status page at `/` shows the registered boxes and the Traefik
configuration the edge would write for them. It is the control plane only.
The tunnel, Traefik's TLS, the mesh and the market cannot run on a
serverless host, so the data path, reaching a box through its public
hostname, still needs the VPS. The page also lists ids and hostnames to
anyone with the URL. It is a demonstration host, not an edge. Deployment
steps and the environment variables are in
[edge/vercel/README.md](https://github.com/dasmatus/losos/blob/main/edge/vercel/README.md).
