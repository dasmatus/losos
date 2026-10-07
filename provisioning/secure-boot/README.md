# Secure Boot signing key, made on your own machine

Release ISOs are signed so a UEFI firmware can verify the stick before
anything on it runs: the medium's loader is one unified kernel image
(kernel, initrd, command line) signed with the LosOS *db* key, and that
loader checks the system image's hash before mounting it
(`modules/secure-boot.nix`). The handbook's *Secure Boot and signed media*
page is the owner-facing half; this is the operator's.

The private key follows the same rule as the official-edge root key
(`../edge-identity/`): it is made **on the operator's own computer**, never
on a box, in CI, or in a session, and it leaves that computer exactly once,
into the repository secret CI signs with. There is no sign-in gate here,
because GitHub itself is the gate: only someone who can set the repository
secret and merge the certificate into `main` can make a key count.

## The ceremony, once

Needs `openssl` and nothing else.

```sh
provisioning/secure-boot/keygen.sh ~/losos-secure-boot
```

That makes, and refuses to ever overwrite:

| file                       | what                                                   | where it goes                                    |
| -------------------------- | ------------------------------------------------------ | ------------------------------------------------ |
| `losos-secure-boot-db.key` | the private key, RSA 2048, mode 0600                   | offline storage; the `SECURE_BOOT_DB_KEY` secret |
| `losos-secure-boot-db.pem` | the self-signed certificate, 10 years                  | `keys/secure-boot-db.pem`, every release         |
| `losos-secure-boot-db.cer` | the same certificate in DER                            | firmware enrolment dialogs; every release        |

Then:

1. **The secret.** `gh secret set SECURE_BOOT_DB_KEY --repo dasmatus/losos
   < ~/losos-secure-boot/losos-secure-boot-db.key`. CI writes it to a
   temporary file for the length of one `losos-sign-iso` call and deletes
   it. Pull requests from forks never see it and ship unsigned.
2. **The certificate.** Append the `.pem` below the comment in
   `keys/secure-boot-db.pem` and open a pull request. From the merge on:
   - the `iso` job on `main` signs the ISO's loader and boots it under OVMF
     with the certificate enrolled (`tests/iso-boot.py --firmware uefi-sb
     --enroll keys/secure-boot-db.pem`), beside the stock-keys leg that
     must refuse it;
   - every release signs the ISO and `SHA256SUMS`, and attaches
     `SHA256SUMS.sig`, the `.pem` and the `.cer`;
   - the medium carries the certificate at `EFI/losos/` for firmware
     "enroll from file" dialogs;
   - the release job **fails** rather than publish unsigned if the secret
     is missing.
3. **Keep the key offline.** Losing it means a new key and a new enrolment
   on every machine that trusts the old one. Leaking it means anyone can
   sign a medium those machines trust, and the only remedy is the same new
   key plus asking every owner to remove the old certificate from `db`.

## Rotation

Run the ceremony into a new directory, replace the secret, replace the
block in `keys/secure-boot-db.pem`. Media signed with the old key keep
booting on machines that still hold the old certificate; machines enrol the
new one from the next release's stick. There is no revocation list (dbx
updates need a KEK the owner holds, which LosOS deliberately does not ask
for), so a rotation is an announcement, not a mechanism.

## Verifying what CI published

```sh
R=https://github.com/dasmatus/losos/releases/download/<tag>
curl -fsSLO $R/SHA256SUMS $R/SHA256SUMS.sig $R/losos-secure-boot-db.pem $R/losos-installer-<tag>.iso
openssl dgst -sha256 -verify <(openssl x509 -pubkey -noout -in losos-secure-boot-db.pem) \
  -signature SHA256SUMS.sig SHA256SUMS
sha256sum -c --ignore-missing SHA256SUMS
nix run .#losos-sign-iso -- verify --cert losos-secure-boot-db.pem losos-installer-<tag>.iso
openssl x509 -in losos-secure-boot-db.pem -noout -fingerprint -sha256   # matches the release notes and keys/
```

## What the test proves without this key

`tests/secure-boot.nix` (`nix build .#checks.x86_64-linux.losos-secure-boot`,
needs KVM to be quick) generates a throwaway PK, KEK and db in the build
sandbox, enrols them into OVMF's variable store with `virt-fw-vars`, signs
the real ISO with the throwaway db key through the same `losos-sign-iso`,
and boots it under OVMF's Secure Boot build: the installer's first line
reads *Secure Boot: enabled*, `bootctl status` says *enabled (user)*, the
firmware's `SecureBoot` variable is 1, and `db` holds the test certificate.
It then watches the same firmware refuse the medium under Microsoft-only
keys, refuse a copy with one byte of the loader changed, refuse the unsigned
build, and watches stage 1 stop a copy with one bit of the system image
changed. The screenshots land in the check's output. None of that key
material is the production key, and none of it is committed.
