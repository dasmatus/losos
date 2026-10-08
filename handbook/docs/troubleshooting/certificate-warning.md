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

The box signs its own certificate, because no public authority can issue
one for a `.local` name. Until you tell your browser to trust that
certificate, every `https://` page warns. Plain `http://<address>` never
warns and is fine on your own LAN. HTTPS matters for passkeys, and it keeps
the password from crossing the network in the clear.

## The fix: install the certificate once per computer

The wizard's first step shows the one-line installer. After setup the box
still serves the same files, to the LAN only:

<img src={require("../img/wizard-step1-trust.png").default} alt="The wizard's certificate step, where the one-line installers and the fingerprint are." width="520" />

```bash title="macOS and Linux"
curl -fsSL http://<address>/setup/trust.sh | sh
```

```powershell title="Windows (PowerShell)"
irm http://<address>/setup/trust.ps1 | iex
```

The script adds one certificate to your user's browser stores and prints its
fingerprint. Compare it with `http://<address>/setup/state.json`. Then
**restart the browser** and open `https://<name>.local`.

To do it by hand, download `http://<address>/setup/losos-ca.crt` and add it
as a trusted certificate authority. On macOS that is Keychain Access. On
Windows it is `certmgr.msc` → Trusted Root Certification Authorities. Firefox
has its own certificate settings, and a phone has its security settings.

## Still warning after installing

- **You opened `https://<address>`.** The certificate is for `<name>.local`
  only, so an IP address always warns. Use the name, or plain `http://` with
  the address.
- **Firefox on Linux** keeps its own store per profile. The script covers
  every profile it finds, but you need to run it again for a profile you
  create later.
- **The box was reinstalled** or its name changed, so it has a new
  certificate. Run the script again. The old entry does no harm.
- **The certificate expired.** It is valid for two years from the install.
  The box makes a new one when it runs out, and the script installs the new
  one.
- **Chrome says the certificate is "not valid for this name"** after a
  rename: same cause, same fix.

## Should I just click through?

On your own LAN, for a box you installed yourself, clicking through once is
not dangerous. But once you tell a browser to ignore the warning, it also
ignores a real attacker on the same Wi-Fi, and it does not offer passkeys.
Install the certificate instead. It takes one line.
