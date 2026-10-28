# Security model

What LosOS defends against, and what it does not.

## Trust boundaries

The appliance has no SSH and no login shell. The root filesystem is a tmpfs;
durable secrets live on the encrypted `/persist`.

1. **Internet → edge.** Only the master proxy (Traefik, the tunnel
   server, `losos-registrar`) faces the internet, and it runs on a separate VPS.
2. **Edge → appliance.** The tunnel, QUIC or TLS over TCP. Its traffic reaches the appliance's
   nginx from `127.0.0.1`, so the LAN-only guard on the admin routes must not
   allow loopback.
3. **LAN → appliance.** Trusted once; see [The LAN](#the-lan).
4. **`notshared` ↔ `shared`.** Two users, each with its own primary group and
   a mode-700 home. Neither can read the other's data.

   This used to be broken: both users had the primary group `users` and
   mode-750 homes, so each could read the other. `nix/tests/impermanence.nix` now
   checks cross-reads in a booted VM.

## The admin token

`lososd` creates a 64-hex-character token from `/dev/urandom` on first start
and writes it with mode 0600 in a 0700 directory. NixOS does not manage it and
it never enters the Nix store.

The token is equivalent to root: `POST /api/apply` writes arbitrary Nix to
`overrides.nix` and runs `nixos-rebuild switch`.

- On an unclaimed box, the first LAN caller to `POST /api/setup/claim` can
  set the owner password, and the first person's sign-in name and password,
  without a token. That person is an administrator. The response hands that
  caller the admin token exactly once, then the route refuses further
  claims. Set up a new box only on a trusted LAN: whoever claims it first
  becomes its owner. The window is guarded by source address (`lanOnly`,
  with loopback denied), not by a secret, and closes on first use. While
  LosOS Login is still starting the route answers 503 and changes
  nothing (`GET /api/setup/claim` says `ready: false` with the reason); the
  window stays open until a claim succeeds.
- The claim route also refuses what a web page in a LAN browser could send:
  a body that is not `application/json` (which would skip the CORS
  preflight), a `Host` that is not the box's hostname, `<hostname>.local` or
  an IP literal (DNS rebinding), and an `Origin` that does not match `Host`.
- `claimed` fails closed. Any state file reads as claimed, whatever it says
  (a stored `false` from the older build included), and so does a path that
  cannot be read or is a dangling symlink. Only a box with nothing at the
  state path is unclaimed.
- The file must contain exactly 64 lowercase hex characters. Anything else is
  discarded and replaced, with an error in the log.
- A claimed box never hands the token out again. People sign in through
  LosOS Login instead ([Signing in](#signing-in)), and the token stays what it
  was printed as during setup: the spare admin key.
- To rotate the token, write a new 64-character lowercase-hex value to the
  file and restart `lososd`, or delete the file and restart it to mint a
  random replacement. Nothing on the box shows the new value, so the printed
  spare is then stale and only someone who can read the file has a key.
- Comparison is constant-time.
- The API listens on `127.0.0.1` only. nginx proxies `/api/` to it behind the
  LAN-only guard. LosOS Cloud, LosOS Git and the apps' hostNetwork pods share that
  loopback, so an iptables owner match (`modules/daemon.nix`) resets any
  connection to the API port that is not made by root or nginx.
- The LAN-only guard denies `10.42.0.0/16`, the mesh's pod network, before it
  allows `10.0.0.0/8`. A mesh pod reaching the box's own address is delivered
  locally with its pod address, which the allow would otherwise accept.
- The guard also refuses a request whose source address is the address it
  arrived on. Only the box itself can send from its own address, so this stops
  a hostNetwork pod that binds the LAN address and goes through nginx. Not
  covered: a box with two local addresses, where a pod binds one and connects
  to the other.

## Signing in

People sign in to the admin pages on LosOS Login, which is Authelia
(`modules/identity.nix`, `losos.identity.enable`, on by default), on
`127.0.0.1:${losos.identity.port}` (9091) behind nginx at
`https://<host>/auth/`. The portal is https only; plain http to `/auth/`
redirects.

- **The people file.** `/var/lib/losos-identity/users.yml` holds each
  person's name, groups and argon2id hash. lososd is its only writer
  (`backend/src/people.rs`); it is root-owned and readable by Authelia's
  group. Authelia watches it, so a password change or a removed account takes
  effect at once. Password change and reset inside Authelia are off, because
  the box sends no mail. A sign-in name is lower case, starts with a letter,
  then letters, digits, `.`, `_` or `-`, at most 32 characters.
- **The Users pane** (`/api/people*`, admin only in `backend/sections.json`)
  adds, changes and removes people, and answers 409 on a box that does not
  sign people in itself. It hands out only `losos-admins`, `losos-viewers`
  and `losos-<section>`, never `losos-owner`, and refuses to change or remove
  the owner, the caller's own account or the last administrator. Authelia's
  `refresh_interval` is `always`, so a removed person's session stops naming
  them to LosOS Cloud and LosOS Git at their next request.
- **Start order.** Authelia starts only once the people file exists (a path
  unit, after the claim). Its file backend refuses to start with an empty
  users map, and without the file it writes an example account of its own.
- **nginx asks Authelia before every `/api/` request** (`auth_request`) and
  passes `Remote-User` and `Remote-Groups` on. Every location that proxies to
  lososd sets both, and `X-Losos-As`, itself, empty when nobody signed in, so
  a client cannot supply them. lososd believes `Remote-User` only from
  loopback and only with `LOSOS_IDENTITY=authelia`; the iptables owner rule
  keeps everyone but root and nginx off the API port.
- **The spare key bypasses Authelia.** A request carrying
  `Authorization: Bearer` skips the `auth_request` and lososd checks the
  token itself, so the printed key opens the box while Authelia is down,
  misconfigured or not started yet. The public routes (health, the claim, the
  terminal's host key and call routes) never ask Authelia.
- **Sections.** `backend/sections.json` maps each route to `public`,
  `signed-in`, `admin` or a section (storage, backup, apps, market, network,
  look, lab, access), and both lososd (`backend/src/sections.rs`) and nginx
  read it. A section needs the group `losos-<section>`; `losos-admins` may do
  everything and `losos-viewers` may `GET` every section. `signed-in` covers
  the overview's reads, `GET /api/me` and `POST /api/set-password`, which
  changes the caller's own password (and, for the person who claimed the box,
  the LosOS Cloud password). A route missing from the file is admin only. A
  person outside a route's section gets 403 with the section named. A person
  whose account was removed after signing in gets 403 with `noAccount`, not
  401, which would send the page round Authelia and back.
- **`X-Losos-As`** names the person a call runs as. lososd honours it only
  together with the admin Bearer token; it is how a terminal call runs as
  its token's owner (`docs/terminal-access.md`).
- **Session.** Cookie `losos_session`, 12 h, 2 h of inactivity, one month
  with "remember me". Its domains are `<host>.local` and each LAN IPv4 of the
  box, rendered into `/run` by `losos-identity-setup` and re-checked every
  2 minutes; Authelia restarts when the set changes.
- **Brute force.** Authelia's own regulation covers wrong passwords on
  `/auth/`. lososd's per-address throttle still counts bad Bearer tokens.
- **Backup and erase.** The people file is in the backup and comes back on a
  restore. The factory wipe deletes it and Authelia's state.

## Enrollment at the edge

The registrar only creates a Traefik router or requests a certificate for ids
listed in `losos.edge.tenants`.

- Blank tokens are rejected before the tenant lookup, so the response does not
  reveal which ids exist.
- A tenant whose token file is shorter than 32 characters is refused.
- An unknown id still costs a decoy read and compare, so its timing matches a
  known id.
- The router's hostname comes from the allow-list, not from what the appliance
  sends.
- Traefik buffers each request before it reaches the registrar and holds one
  source address to 8 in flight, so slow anonymous requests cannot hold the
  registrar's 64 permits. While the registrar is shedding load it does not
  prune tenants for missed heartbeats.
- Traefik reads the routes from the registrar's provider listener, which binds
  loopback only and is not the API's listener, so the public
  `register.<domain>` router (no path rule) cannot list every name the edge
  serves, and neither can a LAN client of a gateway whose API is bound
  off-box.

## System hardening

`modules/hardening.nix`, `losos.hardening.*`. NixOS removed
`profiles/hardened.nix` in 26.05 and `linux_hardened` no longer exists, so the
settings are listed individually.

**On by default** (`hardening.enable`): KSPP kernel parameters; sysctls for
kernel pointers and logs, unprivileged eBPF, ptrace, `userfaultfd`,
`fs.protected_*` and the network stack; a module blacklist that also blocks
explicit `modprobe`; a tmpfs `/tmp` and `noexec` on `/dev/shm`; dbus-broker;
systemd sandboxing on `nginx` and `lososd`; mDNS is systemd-resolved, under its
own upstream sandbox, in place of Avahi (`modules/mdns.nix`).

**Opt-in**: `hardening.apparmor`, `hardening.malloc`, `hardening.nosmt`,
`hardening.usbguard`. They are in the Security pane of the settings page.
Because `apply` rewrites `overrides.nix` from what the UI generates, an option
the UI does not write cannot be turned on at all. A test in
`backend/src/overrides.rs` checks that the module and the daemon's default
body agree.

**Not covered.** `hardened_malloc` is preloaded, so it does not reach k3s,
rke2 or containerd (static Go binaries) or anything in a pod. Hardening does
not address the limitations below.

**Left out on purpose.** `nix/tests/hardening.nix` checks that these stay off:

- **Strict `rp_filter`.** It is `2` (loose). Strict mode drops mDNS replies on
  a multi-homed host, and mDNS is how the box is found. Calico also fails
  under it.
- **Disabling user namespaces.** `user.max_user_namespaces = 0` stops both
  kubelets and containerd.
- **`noexec` on `/tmp`.** Nix builds run there, and the nightly rebuild is the
  box's only self-repair path.

## Known limitations

### The admin UI shares an origin with LosOS Cloud and LosOS Git

`<host>.local/` serves the admin UI and `/api/`. The same origin serves
`/cloud/` and `/git/`, which host user content. LosOS Cloud serves the
files people upload under a sandboxing CSP, and active types as downloads.

`sessionStorage` is per origin, so a stored XSS in either application can
read the admin token while a tab holds the spare key. A CSP on the admin
pages does not help, and neither would an `HttpOnly` cookie: Authelia's
session cookie rides on every request to the origin, so a script there can
call `/api/` as whoever is signed in. **An XSS in LosOS Cloud or LosOS Git is
a full appliance compromise.**

The fix is a separate origin for the admin UI, such as a second mDNS name.
That was declined in favour of a single URL. In place instead: a strict CSP,
`X-Frame-Options`, `nosniff` and `Referrer-Policy` on the admin pages, the
LAN-only guard, and keeping both applications patched.

### Widgets written by hand

The owner can write a widget as HTML, style and script of their own
(Settings → Look, or the board's gallery). Its source is stored on the box
and run only inside `<iframe sandbox="allow-scripts" src="/widget-frame/">`:
no `allow-same-origin`, so the frame is an opaque origin that cannot read the
admin page, its `sessionStorage` (the token) or the API, and `/widget-frame/`
is the one path nginx serves under a permissive CSP of its own
(`modules/containers.nix`), with `frame-ancestors 'self'` and no
`X-Frame-Options` so only the box's own pages may embed it. Everything the
widget learns about the box comes over postMessage from the page, which
answers only the readings the built-in widgets already have.

The author is a person allowed to change the look, or the spare key's
holder, so this is not an injection surface: what it is, is a way for a
hand-written widget that does not work to not take the admin page with it. A
widget may fetch the internet (`connect-src https: http:` in the frame's
policy); the admin page itself still cannot.

### The LAN

`losos.tls.enable` (default on) serves HTTPS with a certificate the box
generates (`modules/tls.nix`). The first-run wizard shows its fingerprint.
ACME is not possible for a `.local` name.

Until a client trusts that certificate, an attacker on the LAN who can
ARP-spoof can intercept first use.

### Physical access

By default (`losos.tpm.enable = true`, which is what the installer ISO
produces wherever it finds a TPM 2.0 chip) the installer seals the LUKS key
into a TPM2 token right after formatting, and `/persist` unlocks from the chip
at boot with no secret in the initrd. The token is bound to no PCRs, on
purpose: the box updates firmware, bootloader and kernel unattended and has no
shell to recover a lockout from. That buys exactly one property: the disk on
its own (pulled, cloned, imaged) is unreadable. It does not buy protection
against a thief who takes the whole box, since the chip will unseal for any
software booted on that machine; a stolen box is in the threat model, not
outside it. The same random key the volume was formatted with is kept inside
the encrypted volume at `/etc/keys/persist-keyfile` as a recovery slot, for
`losos-ctl grow` and for re-enrolment; it is only reachable once the volume is
open.

Without a chip, or with `losos-ctl install --no-tpm`, that keyfile is baked
into the initrd on the unencrypted ESP instead, so whoever has the disk has
the data. `nix/tests/tpm.nix` boots the TPM path end to end under swtpm;
`nix/tests/install.nix` the keyfile path.

### Tunnel encryption

The tunnel is TLS 1.3 (rustls), carried by QUIC on UDP and, where a network
drops UDP, over TCP on the same port. The edge's `losos-tunnel` unit makes a
self-signed certificate on first start (`/var/lib/losos-tunnel/key.der`,
mode 0600) and writes its SHA-256 to `/var/lib/losos-tunnel/pin`; each
appliance pins that hash (`losos.proxy.tunnelPinFile`) and accepts no other
certificate, whatever its name or issuer.

Each appliance fetches the pin from the registrar (`GET /tunnel-pin`) on first
start. That fetch is trust on first use, protected only by the registrar's
TLS. A pin file placed there beforehand is never overwritten, which is how to
pin it out of band. Each service still needs its own token: the edge refuses
a hello whose token does not match, on either transport.

### Throttling and audit log

`lososd` counts failed authentications per client address (`X-Real-IP` from
nginx). After 10 failures the address gets `429` with `Retry-After`, for a
window starting at 1 s and doubling up to 5 min. During the window even the
correct token is refused; a correct token afterwards clears the count.
`/api/health` and `/api/setup/claim` are not throttled. Wrong passwords on
`/auth/` are Authelia's to count, not lososd's.

`POST /api/apply`, `/api/change`, `/api/set-password`, `/api/factory-reset`
and `/api/grow` each append a JSON line to `/var/lib/losos/audit.log`:
timestamp, route, remote address, outcome (`ok`, `rejected`, `error`,
`unauthorized`, `forbidden`, `no account`, `throttled`) and, once the caller
is known, `who`: the sign-in name, or `spare-key`. The file is mode 0600 and
survives reboots. It does not record the token or request bodies. Read-only
routes are not logged.

Not covered: root can rewrite the audit log, it does not rotate, and a failed
write does not block the request. Throttling is per address, so an attacker
who spoofs addresses gets a fresh budget for each.
