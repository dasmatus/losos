# The TPM makers' certificates a box checks a gateway's endorsement key
# against (backend/access/src/gateway/check.rs, `Anchors`): Microsoft's
# TrustedTpm bundle, the roots and intermediate CAs of every TPM maker
# Windows trusts (AMD, Atmel, Infineon, Intel, Microsoft, NationZ, Nuvoton,
# Qualcomm, STMicro), as one PEM file. The box splits it into roots and
# intermediates by whether a certificate signs itself.
#
# Microsoft replaces the file at the same address when a maker adds a CA, so
# the hash below goes stale and a fresh fetch fails until it is bumped here.
# A box never fetches it twice: the bundle keeps a reference to the cab it
# came from, so the cab stays in the box's store for as long as the bundle
# does and every later rebuild of the bundle reads that copy.
{ pkgs }:

let
  # "The certificates were last updated on: 23-October-2026" (version.txt).
  cab = pkgs.fetchurl {
    name = "TrustedTpm-2026-10-23.cab";
    url = "https://download.microsoft.com/download/D/6/5/D65270B2-EAFD-43FD-B9BA-F65CA00B153E/TrustedTpm.cab";
    hash = "sha256-bNl6bJDStL5ZSkf/ttlXkhacXO9kmOscgt/rz1PaDlE=";
  };
in
pkgs.runCommand "losos-tpm-roots"
  {
    nativeBuildInputs = [
      pkgs.cabextract
      pkgs.openssl
    ];
  }
  ''
    mkdir -p x $out
    cabextract -q -d x ${cab}
    # DER mostly, PEM for a few; a file neither reads as is left out (the
    # box would skip it anyway).
    find x -type f \( -iname '*.cer' -o -iname '*.crt' \) | LC_ALL=C sort | while read -r f; do
      openssl x509 -inform DER -in "$f" -outform PEM 2>/dev/null \
        || openssl x509 -in "$f" -outform PEM 2>/dev/null \
        || echo "skipped $f" >&2
    done > $out/tpm-roots.pem
    count=$(grep -c 'BEGIN CERTIFICATE' $out/tpm-roots.pem)
    [ "$count" -gt 1000 ] || { echo "only $count certificates" >&2; exit 1; }
    cp x/version.txt $out/version.txt
    ln -s ${cab} $out/TrustedTpm.cab
  ''
