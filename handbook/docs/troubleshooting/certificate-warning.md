---
title: Certificate warning
sidebar_position: 4
slug: /troubleshooting/certificate-warning
---

# Certificate warning

<div className="losos-symptom">

**What you see:** "Your connection is not private", "Warning: potential
security risk ahead", `NET::ERR_CERT_AUTHORITY_INVALID`, or a padlock with a
line through it, on an `https://` address of the box.

</div>

## Why

The box signs its own certificate; no public authority can issue one for a
`.local` name. Until your browser is told to trust that certificate, every
`https://` page warns. Plain `http://<address>` never warns and is fine on
your own LAN; HTTPS matters for passkeys and for not sending the password in
the clear.

## The fix: install the certificate once per computer

The wizard's first step shows the one-line installer; after setup the same
files are still served by the box, LAN-only:

<img src={require("../img/wizard-step1-trust.png").default} alt="The wizard's certificate step, where the one-line installers and the fingerprint are." width="520" />

```bash title="macOS and Linux"
curl -fsSL http://<address>/setup/trust.sh | sh
```

```powershell title="Windows (PowerShell)"
irm http://<address>/setup/trust.ps1 | iex
```

The script adds one certificate to your user's browser stores and prints its
fingerprint; compare it with `http://<address>/setup/state.json`. Then
**restart the browser** and open `https://<name>.local`.

By hand: download `http://<address>/setup/losos-ca.crt` and add it as a
trusted certificate authority (Keychain Access on macOS, `certmgr.msc` →
Trusted Root Certification Authorities on Windows, the browser's own
certificate settings in Firefox, the phone's security settings).

## Still warning after installing

- **You opened `https://<address>`.** The certificate is for `<name>.local`
  only; an IP address always warns. Use the name, or plain `http://` with the
  address.
- **Firefox on Linux** keeps its own store per profile; the script covers
  every profile it finds, but a profile created later needs the script run
  again.
- **The box was reinstalled** or its name changed: it has a new certificate.
  Run the script again; the old entry does no harm.
- **The certificate expired.** It is valid for two years from the install;
  the box mints a new one when it runs out, and the script installs the new
  one.
- **Chrome says the certificate is "not valid for this name"** after a
  rename: same cause, same fix.

## Should I just click through?

On your own LAN, for a box you installed yourself, clicking through once is
not dangerous. But a browser that was told to ignore the warning will also
ignore a real attacker on the same Wi-Fi, and passkeys will not be offered.
Install the certificate instead; it takes one line.
