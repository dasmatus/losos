# Security model

What losos defends against, and what it does not.

## Trust boundaries

The appliance has no SSH and no login shell. The root filesystem is a tmpfs;
durable secrets live on the encrypted `/persist`.

1. **Internet → edge.** Only the master proxy (Traefik, rathole,
   `losos-registrar`) faces the internet, and it runs on a separate VPS.
2. **Edge → appliance.** A rathole tunnel. Its traffic reaches the appliance's
   nginx from `127.0.0.1`, so the LAN-only guard on the admin routes must not
   allow loopback.
3. **LAN → appliance.** Trusted once; see [The LAN](#the-lan).
4. **`notshared` ↔ `shared`.** Two users, each with its own primary group and
   a mode-700 home. Neither can read the other's data.

   This used to be broken: both users had the primary group `users` and
   mode-750 homes, so each could read the other. `tests/impermanence.nix` now
   checks cross-reads in a booted VM.

## The admin token

`lososd` creates a 64-hex-character token from `/dev/urandom` on first start
and writes it with mode 0600 in a 0700 directory. NixOS does not manage it and
it never enters the Nix store.

The token is equivalent to root: `POST /api/apply` writes arbitrary Nix to
`overrides.nix` and runs `nixos-rebuild switch`.

- On an unclaimed box, the first LAN caller to `POST /api/setup/claim` can set
  the owner password without a token. The response hands that caller the admin
  token exactly once, then the route refuses further claims. Set up a new box
  only on a trusted LAN: whoever claims it first becomes its owner. The window
  is guarded by source address (`lanOnly`, with loopback denied), not by a
  secret, and closes on first use.
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
- A claimed box cannot fetch the token again. To rotate it, write a new
  64-character lowercase-hex value to the file and restart `lososd`, or delete
  the file and restart it to mint a random replacement. Deleting it requires
  local access to retrieve the replacement.
- Comparison is constant-time.
- The API listens on `127.0.0.1` only. nginx proxies `/api/` to it behind the
  LAN-only guard. The hostNetwork Nextcloud and Forgejo pods share that
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

## System hardening

`modules/hardening.nix`, `losos.hardening.*`. NixOS removed
`profiles/hardened.nix` in 26.05 and `linux_hardened` no longer exists, so the
settings are listed individually.

**On by default** (`hardening.enable`): KSPP kernel parameters; sysctls for
kernel pointers and logs, unprivileged eBPF, ptrace, `userfaultfd`,
`fs.protected_*` and the network stack; a module blacklist that also blocks
explicit `modprobe`; a tmpfs `/tmp` and `noexec` on `/dev/shm`; dbus-broker;
systemd sandboxing on `nginx`, `avahi-daemon` and `lososd`.

**Opt-in**: `hardening.apparmor`, `hardening.malloc`, `hardening.nosmt`,
`hardening.usbguard`. They are in the Security pane of the settings page.
Because `apply` rewrites `overrides.nix` from what the UI generates, an option
the UI does not write cannot be turned on at all. A test in
`backend/src/overrides.rs` checks that the module and the daemon's default
body agree.

**Not covered.** `hardened_malloc` is preloaded, so it does not reach k3s,
rke2 or containerd (static Go binaries) or anything in a pod. Hardening does
not address the limitations below.

**Left out on purpose.** `tests/hardening.nix` checks that these stay off:

- **Strict `rp_filter`.** It is `2` (loose). Strict mode drops mDNS replies on
  a multi-homed host, and mDNS is how the box is found. Calico also fails
  under it.
- **Disabling user namespaces.** `user.max_user_namespaces = 0` stops both
  kubelets and containerd.
- **`noexec` on `/tmp`.** Nix builds run there, and the nightly rebuild is the
  box's only self-repair path.

## Known limitations

### The admin UI shares an origin with Nextcloud and Forgejo

`<host>.local/` serves the admin UI and `/api/`. The same origin serves
`/nextcloud` and `/forgejo/`, which host user content.

`sessionStorage` is per origin, so a stored XSS in either application can read
the admin token. A CSP on the admin pages does not help, and neither would an
`HttpOnly` cookie. **An XSS in Nextcloud or Forgejo is a full appliance
compromise.**

The fix is a separate origin for the admin UI, such as a second mDNS name.
That was declined in favour of a single URL. In place instead: a strict CSP,
`X-Frame-Options`, `nosniff` and `Referrer-Policy` on the admin pages, the
LAN-only guard, and keeping both applications patched.

### The LAN

`losos.tls.enable` (default on) serves HTTPS with a certificate the box
generates (`modules/tls.nix`). The first-run wizard shows its fingerprint.
ACME is not possible for a `.local` name.

Until a client trusts that certificate, an attacker on the LAN who can
ARP-spoof can intercept first use.

### Physical access

With `losos.tpm.enable`, `/persist` unlocks from the TPM. Without a TPM the
keyfile is in the initrd on an unencrypted ESP, so whoever has the machine has
the data.

### Tunnel encryption

The rathole tunnel uses Noise (`Noise_NK_25519_ChaChaPoly_BLAKE2s`). The edge
holds the private key (`losos.edge.noisePrivateKeyFile`, a 0600 file under
`/var/secrets`); each appliance pins the public key
(`losos.proxy.noisePublicKeyFile`).

The edge generates the pair on first boot. Each appliance fetches the public
key from the registrar (`GET /noise-public-key`) on first start. That fetch is
trust on first use, protected only by the registrar's TLS. A key file placed
there beforehand is never overwritten, which is how to pin it out of band.
Setting either option to `null` falls back to plain TCP.

### Throttling and audit log

`lososd` counts failed authentications per client address (`X-Real-IP` from
nginx). After 10 failures the address gets `429` with `Retry-After`, for a
window starting at 1 s and doubling up to 5 min. During the window even the
correct token is refused; a correct token afterwards clears the count.
`/api/health` and `/api/setup/claim` are not throttled.

`POST /api/apply`, `/api/change`, `/api/set-password`, `/api/factory-reset`
and `/api/grow` each append a JSON line to `/var/lib/losos/audit.log`:
timestamp, route, remote address and outcome (`ok`, `rejected`, `error`,
`unauthorized`, `throttled`). The file is mode 0600 and survives reboots. It
does not record the token or request bodies. Read-only routes are not logged.

Not covered: root can rewrite the audit log, it does not rotate, and a failed
write does not block the request. Throttling is per address, so an attacker
who spoofs addresses gets a fresh budget for each.
