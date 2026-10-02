# Security model

The full document is
[`docs/security-model.md`](https://github.com/dasmatus/losos/blob/main/docs/security-model.md)
in the repository, versioned with the code. Summary:

## Trust boundaries

1. **Internet → edge.** Only the [master proxy](Master-Proxy) on the VPS is
   internet-facing.
2. **Edge → appliance.** A rathole tunnel, encrypted with Noise.
3. **LAN → appliance.** HTTPS with a self-signed certificate that you trust
   once. Before that, a LAN attacker can intercept first use.
4. **`notshared` ↔ `shared`.** Separate users and groups, mode-700 homes.

## Known limitations

- **Shared origin.** The admin UI, Nextcloud and Forgejo share one origin. An
  XSS in Nextcloud or Forgejo can read the admin token, which is equivalent to
  root. The fix would be a separate hostname for the admin UI.
- **No TPM means no protection against theft.** The keyfile is on an
  unencrypted ESP.
- **The audit log is not tamper-evident** and does not rotate.
- **Throttling is per address**, so address spoofing on the LAN bypasses it.
