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

### The LAN is inside the trust boundary

There is no TLS on the appliance. The admin token crosses the local network in
cleartext on every request, as do Nextcloud logins. Anything on the LAN, or
anything able to ARP-spoof it, can read them. `security.acme` cannot help
without a public name; a self-signed certificate with a stable CA would.

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
