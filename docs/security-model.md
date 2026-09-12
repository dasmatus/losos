# Security model

What losos defends against, what it does not, and which of those are
deliberate choices rather than oversights. Written to be argued with.

## Trust boundaries

The appliance has no SSH and no login shell. Every durable secret lives on an
encrypted `/persist`; the root filesystem is tmpfs and is rebuilt each boot.
There are four boundaries that matter:

1. **The internet → the edge.** Only the master proxy (Traefik + rathole +
   `losos-registrar`) is internet-facing, and it runs on a separate VPS, not
   on the appliance.
2. **The edge → the appliance.** A rathole tunnel. Traffic arrives at the
   appliance's Nginx *from 127.0.0.1*, which is why the LAN-only guard on the
   admin routes must never allow loopback.
3. **The LAN → the appliance.** Everything on the local network is treated as
   trusted. See "Known limitations".
4. **`notshared` ↔ `shared`.** Two uids, each with its **own primary group**,
   two home directories at mode `700`. Neither can read the other's data.

   This did not hold until recently, and the way it failed is worth keeping:
   both accounts were `isNormalUser` with no explicit `group`, so nixpkgs gave
   both the primary group `users` — and the homes' `750` granted r-x to exactly
   that group. The mode kept out accounts *outside* `users` and nothing else,
   so each data domain could read the other while the code, the comments and
   the README all claimed otherwise. `tests/impermanence.nix` asserts the real
   behaviour in a booted VM now, cross-reads and all.

## The admin token

`lososd` mints a 64-hex-character token from `/dev/urandom` on first start,
writes it `0600` in a `0700` directory, and fsyncs it. It is not managed by
NixOS and never enters the Nix store.

That token is **root-equivalent**: `POST /api/apply` writes arbitrary Nix to
`overrides.nix` and runs `nixos-rebuild switch`. This is by design — the admin
UI's whole purpose is reconfiguring the box — which is why the token's
handling gets the attention it does:

- The token file is validated on read. It must be exactly 64 lowercase hex
  characters; anything else is discarded and re-minted with a loud error.
  Previously *any* file content was accepted as the shared secret, so a
  truncated or hand-edited file became a guessable root password.
- Comparison is constant-time, after a case-insensitive scheme strip.
- The API binds `127.0.0.1` only. Nginx proxies `/api/` to it, under the
  LAN-only guard.

## Enrollment at the edge

`losos.edge.tenants` is the security boundary: the registrar only ever writes
a Traefik router, and only ever requests a certificate, for an id listed
there. Enforcement:

- Blank tokens are rejected *before* the tenant lookup, so a rejection leaks
  nothing about which ids exist.
- A token file below 32 characters refuses that tenant entirely rather than
  accepting a weak secret.
- The unknown-id path performs a decoy read and compare, so it costs roughly
  what a known id costs. This was the larger timing oracle — a byte-compare
  difference is far harder to measure than one filesystem read versus two.
- The router's hostname comes from the whitelist, not from the runtime
  registry, so nothing an appliance sends can widen what it is served.

## System hardening

`losos.hardening.*` (`modules/hardening.nix`) is the kernel-and-userspace layer
under everything above. It is staged rather than one switch, because the
obvious shortcut no longer exists: NixOS removed `profiles/hardened.nix` in
26.05, and `linux_hardened` in the nixpkgs this flake tracks is
`throw "linux_hardened has been removed due to lack of maintenance"`. Upstream's
stated reason — the profile "lacks a consistent and transparent baseline" and
was "often more of a 'grab bag' of settings than a cohesive security policy" —
is a fair criticism to inherit rather than repeat.

**On by default** (`hardening.enable`): KSPP kernel parameters; sysctl
tightening for kernel-address and log disclosure, unprivileged eBPF, ptrace,
`userfaultfd`, the `fs.protected_*` family and the network stack; a
kernel-module blacklist that also blocks explicit `modprobe`; a separate `/tmp`
tmpfs and `noexec` on `/dev/shm`; dbus-broker; and systemd sandboxing on
`nginx`, `avahi-daemon` and `lososd`.

**Opt-in**, because each can cost something: `hardening.apparmor`,
`hardening.malloc`, `hardening.nosmt`, `hardening.usbguard`.

They are reachable from the **Security** pane of the settings page, and that is
newer than the options themselves. Until it existed the four were declared in
`options.nix` and named nowhere in `modules/overrides.nix` — and since that file
is the only way to change a `losos.*` option on a box with no SSH and no shell
login, they were not merely off by default, they were *unreachable*. An option
an owner cannot turn on is not a mitigation, however carefully it is described.
Getting this wrong twice is easy, because `apply` replaces `overrides.nix`
wholesale from what the UI generates: a key the UI does not emit is erased on
the next save of any unrelated setting. `backend/src/overrides.rs` carries a
test asserting the module and the daemon's copy of its default body still
agree.

### What it does not cover

Two limits worth stating rather than implying. The hardened allocator works by
preloading, so it covers the host's dynamically linked processes and **nothing
inside Kubernetes**: k3s, rke2 and containerd are static Go binaries that ignore
preloading, and a pod has its own rootfs and therefore its own absent preload
file. And no amount of kernel hardening touches the three limitations below —
the shared origin, the cleartext LAN, or the no-TPM physical-access path.

### Three things it deliberately does not do

Each is on every hardening checklist, and each would cost more here than it
buys. `tests/hardening.nix` asserts all three *absent*, so they read as
decisions rather than oversights.

- **`rp_filter` is 2 (loose), not 1 (strict)**, for two independent reasons.
  Strict reverse-path filtering drops packets whose source would not route back
  out the interface they arrived on — which is what multicast replies look like
  on a multi-homed machine, and mDNS is the only way to reach a box with no SSH
  and no shell login. Calico, half of the canal CNI the mesh cluster runs, also
  does not work under it.
- **User namespaces stay enabled.** `security.allowUserNamespaces = false` sets
  `user.max_user_namespaces = 0`, which stops both kubelets and containerd.
- **`/tmp` is not `noexec`.** Nix builds unpack and execute scripts there, and
  the nightly unattended rebuild is this appliance's only self-repair path.

## Known limitations

These are accepted, not unnoticed.

### The admin UI shares an origin with Nextcloud and Forgejo

`http://<host>.local/` serves the admin SPA and `/api/`. The **same origin**
serves `/nextcloud` and `/forgejo/`, which host user-controlled content:
repositories, rendered markdown, attachments, uploaded files.

`sessionStorage` is scoped per **origin**, not per path. So a stored-XSS in
either application executes on the admin origin and can read the admin token,
and no Content-Security-Policy on the admin responses prevents it — the CSP
constrains the admin documents, not the Nextcloud one. An `HttpOnly` cookie
would not fix it either: on a shared origin, forged same-origin requests carry
the cookie automatically. **Same-origin XSS defeats every browser-side
mitigation.** The only real fix is origin separation — a distinct hostname or
port for the admin plane.

That was considered and declined, in favour of a single clean URL. The
consequence is explicit: *an XSS in Nextcloud or Forgejo is a full appliance
compromise.* The mitigations actually in place are defence in depth, not a
fix — a strict CSP, `X-Frame-Options`, `nosniff` and `Referrer-Policy` on the
admin surface; the LAN-only guard, which reduces exposure to attackers already
on the LAN but does not help here because the attack runs in the victim's own
browser; and keeping both applications patched, which is what actually
carries the risk.

Reversing the decision is cheap and localised: serve the admin vhost under a
second mDNS name on port 80. That is a genuinely different origin, at which
point `sessionStorage` isolation applies and the CSP becomes a real boundary.

### The LAN is inside the trust boundary — mostly, now

This section used to say there was no TLS and that the admin token crossed the
LAN in cleartext on every request. **That is no longer true**, and it is worth
being precise about what replaced it, because a security document that
understates the shipped posture is as misleading as one that overstates it.

`losos.tls.enable` now defaults on. The appliance mints its own CA and serves
HTTPS under it (`modules/tls.nix`), and the first-run wizard shows the
certificate fingerprint so it can be checked before anything secret is typed.
`security.acme` still cannot help — it needs a public name, which a `.local`
box does not have — so this is a self-signed chain you trust once, not a
publicly rooted one.

What remains true: until that certificate is trusted, and on any client that
skips the wizard, a LAN attacker able to ARP-spoof can still mount the usual
first-use interception. The boundary is now "a network you trust *once*"
rather than "a network you trust continuously".

### Physical access

`losos.tpm.enable` unlocks `/persist` from the TPM. On the no-TPM path a
keyfile is baked into the initrd, which lives on an unencrypted ESP — so on
that path, physical possession yields the data. Only the TPM path resists
physical theft.

### Not covered

Rathole runs plain TCP; the tunnel carries post-TLS-termination HTTP, so
enabling its Noise transport is a hardening step still outstanding. Nothing
rate-limits authentication attempts against `/api/`. There is no audit log of
admin actions beyond the systemd journal.
