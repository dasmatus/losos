#!/usr/bin/env bash
# The LosOS Secure Boot key ceremony: make the db signing key on YOUR machine.
#
#   provisioning/secure-boot/keygen.sh [DIR]      (default: ./losos-secure-boot)
#
# Makes, once, in DIR:
#   losos-secure-boot-db.key   the private key (0600). Keep it offline. Its
#                              only copy outside DIR is the SECURE_BOOT_DB_KEY
#                              repository secret CI signs with.
#   losos-secure-boot-db.pem   the certificate (PEM): goes into
#                              keys/secure-boot-db.pem in the repository, below
#                              that file's comment.
#   losos-secure-boot-db.cer   the same certificate in DER, for firmware
#                              "enroll from file" dialogs (the ISO carries it
#                              too, at EFI/losos/).
#
# Refuses to overwrite an existing key: a second key means media signed with
# the first one stop booting on machines that enrolled it. Needs only openssl.
# Nothing here touches the network, and nothing is ever run in a CI job or
# any other automated run: the whole point is that the private key exists
# on one computer a person owns.
set -euo pipefail

dir=${1:-./losos-secure-boot}
name=losos-secure-boot-db
days=3650

if [ -e "$dir/$name.key" ]; then
  echo "keygen: $dir/$name.key exists; refusing to make a second key (see the header)" >&2
  exit 1
fi
umask 077
mkdir -p "$dir"

# RSA 2048 is what every UEFI implementation verifies (the EFI_CERT_X509
# signature type); larger keys and ECDSA are not universally supported by
# firmware. Ten years, self-signed: firmware checks the certificate against
# db by identity, not through a chain.
openssl req -new -x509 -newkey rsa:2048 -sha256 -nodes -days "$days" \
  -subj "/CN=LosOS Secure Boot db/O=LosOS/" \
  -keyout "$dir/$name.key" -out "$dir/$name.pem" 2>/dev/null
chmod 0600 "$dir/$name.key"
chmod 0644 "$dir/$name.pem"
openssl x509 -in "$dir/$name.pem" -outform DER -out "$dir/$name.cer"
chmod 0644 "$dir/$name.cer"

fp=$(openssl x509 -in "$dir/$name.pem" -noout -fingerprint -sha256 | cut -d= -f2)
cat <<MSG
keygen: made $dir/$name.{key,pem,cer}
        SHA256 fingerprint $fp

Next, in this order (provisioning/secure-boot/README.md has the long form):

  1. Put the private key where CI signs with it, and nowhere else:
       gh secret set SECURE_BOOT_DB_KEY --repo dasmatus/losos < $dir/$name.key
  2. Publish the certificate: append $dir/$name.pem to keys/secure-boot-db.pem
     (below its comment) and open a pull request. From that merge on, every
     ISO CI builds on main and every release is signed with this key, the
     medium carries the certificate at EFI/losos/, and SHA256SUMS.sig on a
     release verifies against it.
  3. Keep $dir/$name.key offline. Losing it means a new key and a new
     enrolment on every machine; leaking it means anyone can sign a medium
     those machines trust.
MSG
