# losos-sign-iso: sign, verify or inspect the UEFI loader inside a LosOS
# installer ISO, in place.
#
# The medium's UEFI loader is one unified kernel image at
# EFI/BOOT/BOOTX64.EFI on the EFI system partition image the ISO carries as
# an El Torito boot image (modules/secure-boot.nix). This script never
# rebuilds the ISO: it reads the El Torito catalog to find where that FAT
# image starts, pulls the loader out with mtools, signs it with sbsign, and
# writes it back over the original. Every other byte of the ISO stays where
# it was, so the boot catalog, the hybrid GPT and the squashfs are untouched,
# and a verify is the same extraction followed by sbverify.
#
#   losos-sign-iso sign   --key db.key --cert db.pem [--out signed.iso] in.iso
#   losos-sign-iso verify --cert db.pem iso
#   losos-sign-iso show   iso
#
# The key never enters a Nix store: this runs after `nix build`, on the CI
# runner with the SECURE_BOOT_DB_KEY secret, or in the test sandbox with a
# throwaway key. Without --out, sign modifies the ISO in place.

usage() {
  cat >&2 <<'EOF'
usage: losos-sign-iso sign   --key KEY --cert CERT [--out OUT.iso] IN.iso
       losos-sign-iso verify --cert CERT ISO
       losos-sign-iso show   ISO
EOF
  exit 2
}

# The byte offset of the EFI El Torito boot image inside the ISO: xorriso
# reports its LBA in 2048-byte sectors.
efi_offset() {
  local lba
  lba=$(xorriso -indev "$1" -report_el_torito plain 2>/dev/null \
    | awk '$1 == "El" && $4 == "img" && $7 == "UEFI" { print $NF; exit }')
  if [ -z "$lba" ]; then
    echo "losos-sign-iso: $1 has no UEFI El Torito boot image" >&2
    exit 1
  fi
  echo $((lba * 2048))
}

loader="::/EFI/BOOT/BOOTX64.EFI"

cmd=${1:-}
[ -n "$cmd" ] || usage
shift

key=""
cert=""
out=""
iso=""
while [ $# -gt 0 ]; do
  case "$1" in
    --key) key=$2; shift 2 ;;
    --cert) cert=$2; shift 2 ;;
    --out) out=$2; shift 2 ;;
    -*) usage ;;
    *)
      [ -z "$iso" ] || usage
      iso=$1
      shift
      ;;
  esac
done
[ -n "$iso" ] || usage

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

case "$cmd" in
  sign)
    [ -n "$key" ] && [ -n "$cert" ] || usage
    if [ -n "$out" ]; then
      # A store path is read-only; the copy must not be.
      cp --reflink=auto --no-preserve=mode "$iso" "$out"
      chmod u+w "$out"
      iso=$out
    fi
    off=$(efi_offset "$iso")
    mcopy -i "$iso@@$off" "$loader" "$tmp/unsigned.efi"
    sbsign --key "$key" --cert "$cert" --output "$tmp/signed.efi" "$tmp/unsigned.efi"
    sbverify --cert "$cert" "$tmp/signed.efi" >/dev/null
    # -o: replace the file; the FAT image carries the slack for the bigger
    # copy (modules/secure-boot.nix).
    mcopy -o -i "$iso@@$off" "$tmp/signed.efi" "$loader"
    mcopy -i "$iso@@$off" "$loader" "$tmp/back.efi"
    cmp -s "$tmp/signed.efi" "$tmp/back.efi" || {
      echo "losos-sign-iso: the loader read back differs from the one written" >&2
      exit 1
    }
    echo "losos-sign-iso: signed EFI/BOOT/BOOTX64.EFI in $iso with $cert"
    ;;
  verify)
    [ -n "$cert" ] || usage
    off=$(efi_offset "$iso")
    mcopy -i "$iso@@$off" "$loader" "$tmp/loader.efi"
    sbverify --cert "$cert" "$tmp/loader.efi"
    ;;
  show)
    off=$(efi_offset "$iso")
    echo "EFI system partition image at byte offset $off"
    mcopy -i "$iso@@$off" "$loader" "$tmp/loader.efi"
    echo "EFI/BOOT/BOOTX64.EFI: $(stat -c %s "$tmp/loader.efi") bytes"
    mdir -i "$iso@@$off" ::/EFI/losos 2>/dev/null | sed -n 's/^ *\(losos[^ ]*\).*/certificate on the medium: EFI\/losos\/\1/p' || true
    sbverify --list "$tmp/loader.efi" || true
    ;;
  *) usage ;;
esac
