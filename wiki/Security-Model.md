# Security model

The full document is
[`docs/security-model.md`](https://github.com/dasmatus/losos/blob/main/docs/security-model.md)
in the repository, versioned with the code. The short version:

## Trust boundaries

1. **Internet to edge.** Only the [master proxy](Master-Proxy) on the VPS
   faces the internet.
2. **Edge to appliance.** A rathole tunnel, encrypted with Noise.
3. **LAN to appliance.** HTTPS with a self-signed certificate that you trust
   once. Until you do, a LAN attacker can intercept the first use.
4. **`notshared` and `shared`.** Separate users and groups, mode-700 homes.

## Known limitations

- **Shared origin.** The admin UI, Nextcloud and Forgejo share one origin. An
  XSS in Nextcloud or Forgejo can read the admin token, and the token is
  root. The fix would be a separate hostname for the admin UI.
- **Theft of the whole box is not defended against.** The disk key is sealed
  to the TPM without PCR binding, so the chip releases it to any software
  that runs on that machine. Only the disk on its own is unreadable. On a
  machine without a TPM the keyfile sits on an unencrypted ESP, and even the
  disk alone is readable.
- **The audit log is not tamper-evident** and does not rotate.
- **Throttling is per address**, so address spoofing on the LAN bypasses it.
