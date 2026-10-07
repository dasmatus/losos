# nixos-test-vms config for the signed installer medium under Secure Boot
# (modules/secure-boot.nix, losos-sign-iso).
#
# Boots the real installer ISO — the same `nixosConfigurations.iso` the
# release ships, extended only with the test backdoor so the script can look
# inside — from a virtual USB stick, under OVMF's Secure Boot build with SMM,
# four times:
#
#   1. signed, firmware trusts the signing key      → boots; from inside, the
#      firmware says Secure Boot is on and the loader was systemd-stub, and
#      the certificate in `db` is the one the ISO was signed with;
#   2. signed, firmware holds only Microsoft's keys → refused ("Access
#      Denied"), which is what a stock PC does before its owner enrols the
#      LosOS certificate;
#   3. signed, one byte of the loader changed        → refused: the signature
#      covers the whole image, so a tampered medium does not start;
#   4. unsigned (the plain `nix build` output)       → refused;
#   5. signed, one byte of the system image changed  → the firmware starts
#      it (the loader is intact), and stage 1 refuses it: the squashfs no
#      longer matches the hash the signed command line carries.
#
# The keys are THROWAWAY: a PK, a KEK and a db certificate generated in the
# build sandbox below, enrolled into a copy of OVMF's variable store with
# virt-fw-vars, and used for nothing else. They are not the production key,
# which never enters a Nix build (see the module header); the production
# certificate is keys/secure-boot-db.pem, and the only thing this test says
# about it is that the same tool and the same ISO shape work.
#
# The refusals are asserted on the serial console: OVMF's boot manager
# prints `BdsDxe: failed to load Boot0001 ... : Access Denied` when the
# firmware's image verification rejects the loader, then falls through to
# its boot menu. Each run leaves a screenshot in the check's output, so the
# proof is in $out, not only in the log.
{
  pkgs,
  # self.nixosConfigurations.iso, extended here with the test backdoor.
  iso,
  losos-sign-iso,
}:

let
  testIso =
    (iso.extendModules {
      modules = [
        "${pkgs.path}/nixos/modules/testing/test-instrumentation.nix"
        {
          # efi-readvar, to show the certificate the firmware holds in `db`.
          environment.systemPackages = [ pkgs.efitools ];
          # The instrumentation's loglevel=7 floods the screen with driver
          # chatter and, under TCG, leaves the framebuffer seconds behind
          # the serial console; the screenshots are the point here.
          boot.kernelParams = pkgs.lib.mkAfter [ "loglevel=4" ];
        }
      ];
    }).config.system.build.isoImage;
  inherit (testIso) isoName;

  # Owner GUID the enrolled certificates carry; any fixed value works.
  guid = "77fa9abd-0359-4d32-bd60-28f4e78f784b";

  keys =
    pkgs.runCommand "losos-secure-boot-test-keys"
      {
        nativeBuildInputs = [ pkgs.openssl ];
      }
      ''
        mkdir -p $out
        for n in PK KEK db stranger; do
          openssl req -new -x509 -newkey rsa:2048 -nodes -days 36500 \
            -subj "/CN=LosOS test $n, not the production key/O=tests secure-boot.nix/" \
            -keyout $out/$n.key -out $out/$n.pem
        done
      '';

  ovmf = pkgs.OVMFFull;

  # OVMF's empty variable store with the throwaway PK/KEK/db enrolled and
  # Secure Boot switched on — a firmware that trusts the test signer.
  lososVars =
    pkgs.runCommand "ovmf-vars-losos-test-key"
      {
        nativeBuildInputs = [ pkgs.python3Packages.virt-firmware ];
      }
      ''
        virt-fw-vars --input ${ovmf.variables} --output $out \
          --set-pk ${guid} ${keys}/PK.pem \
          --add-kek ${guid} ${keys}/KEK.pem \
          --add-db ${guid} ${keys}/db.pem \
          --secure-boot
        virt-fw-vars --input $out --print
      '';

  # A stock PC: Microsoft's PK-less template with the Microsoft KEK and db
  # certificates, as OVMF ships it.
  msVars = ovmf.variablesMs;

  signedIso =
    pkgs.runCommand "losos-installer-signed-test"
      {
        nativeBuildInputs = [ losos-sign-iso ];
      }
      ''
        mkdir -p $out/iso
        losos-sign-iso sign --key ${keys}/db.key --cert ${keys}/db.pem \
          --out $out/iso/${isoName} ${testIso}/iso/${isoName}
        losos-sign-iso verify --cert ${keys}/db.pem $out/iso/${isoName}
        losos-sign-iso show $out/iso/${isoName}
      '';

  # The signed ISO with one byte of the loader changed: the UKI's .osrel
  # section carries `IMAGE_ID=losos-installer` (modules/secure-boot.nix),
  # which occurs nowhere else in the image outside the compressed squashfs.
  # The signature still parses, but the hash it signs no longer matches.
  tamperedIso =
    pkgs.runCommand "losos-installer-tampered-test"
      {
        nativeBuildInputs = [
          pkgs.python3
          losos-sign-iso
        ];
      }
      ''
        mkdir -p $out/iso
        cp --reflink=auto ${signedIso}/iso/${isoName} $out/iso/${isoName}
        chmod u+w $out/iso/${isoName}
        python3 - $out/iso/${isoName} <<'EOF'
        import mmap, sys
        marker = b"IMAGE_ID=losos-installer"
        with open(sys.argv[1], "r+b") as f:
            m = mmap.mmap(f.fileno(), 0)
            first = m.find(marker)
            assert first >= 0, "marker not found in the ISO"
            assert m.find(marker, first + 1) < 0, "marker is not unique in the ISO"
            at = first + len(marker) - 1
            m[at] = ord("R")  # losos-installeR
            m.flush()
            print(f"flipped one byte at offset {at}")
        EOF
        # Still the same loader, and sbverify now says so.
        if losos-sign-iso verify --cert ${keys}/db.pem $out/iso/${isoName}; then
          echo "the tampered loader still verifies; the byte did not land in it" >&2
          exit 1
        fi
      '';

  # The signed ISO with one byte of the system image changed, a megabyte
  # into the squashfs: the loader and its signature are untouched, so this
  # is the case the firmware cannot catch and stage 1 must.
  tamperedStoreIso =
    pkgs.runCommand "losos-installer-tampered-store-test"
      {
        nativeBuildInputs = [
          pkgs.python3
          pkgs.xorriso
          losos-sign-iso
        ];
      }
      ''
        mkdir -p $out/iso
        cp --reflink=auto ${signedIso}/iso/${isoName} $out/iso/${isoName}
        chmod u+w $out/iso/${isoName}
        lba=$(xorriso -indev $out/iso/${isoName} -find /nix-store.squashfs -exec report_lba -- 2>/dev/null \
          | awk -F, '$1 ~ /File data lba/ { gsub(/ /, "", $2); print $2; exit }')
        [ -n "$lba" ] || { echo "no /nix-store.squashfs in the ISO" >&2; exit 1; }
        python3 - $out/iso/${isoName} "$((lba * 2048 + 1048576))" <<'EOF'
        import sys
        path, at = sys.argv[1], int(sys.argv[2])
        with open(path, "r+b") as f:
            f.seek(at)
            b = f.read(1)
            f.seek(at)
            f.write(bytes([b[0] ^ 0x01]))
        print(f"flipped one bit at offset {at}")
        EOF
        # The loader is untouched: it still verifies.
        losos-sign-iso verify --cert ${keys}/db.pem $out/iso/${isoName}
      '';

  qemu = "${pkgs.qemu_test}/bin/qemu-system-x86_64";
in
pkgs.testers.nixosTest {
  name = "losos-secure-boot";

  nodes = { };

  testScript = ''
    import os
    import shutil
    import time

    SB_VAR = "/sys/firmware/efi/efivars/SecureBoot-8be4df61-93ca-11d2-aa0d-00e098032b8c"
    STUB_VAR = "/sys/firmware/efi/efivars/StubInfo-4a67b082-0a4c-41cf-b6c7-440b29bb8c4f"


    def firmware(iso, vars_template, name):
        """A machine booting `iso` from a USB stick under OVMF with Secure Boot.

        The variable store is copied, because the firmware writes to it (boot
        entries) and the template lives in the store. SMM and the secure
        pflash property are what OVMF's Secure Boot build needs; without
        them the firmware never leaves its own setup."""
        vars_file = os.path.join(os.getcwd(), f"{name}-vars.fd")
        shutil.copyfile(vars_template, vars_file)
        os.chmod(vars_file, 0o644)
        cmd = (
            "${qemu} -m 3072 -smp 2 -machine q35,smm=on,accel=kvm:tcg -cpu max"
            " -global driver=cfi.pflash01,property=secure,value=on"
            f" -drive if=pflash,format=raw,unit=0,readonly=on,file=${ovmf.firmware}"
            f" -drive if=pflash,format=raw,unit=1,file={vars_file}"
            f" -drive if=none,id=stick,format=raw,readonly=on,file={iso}"
            " -device qemu-xhci -device usb-storage,drive=stick,bootindex=0"
        )
        return create_machine(cmd, name=name)


    def refused(iso, vars_template, name, shot):
        m = firmware(iso, vars_template, name)
        m.start()
        m.wait_for_console_text("Access Denied")
        m.screenshot(shot)
        m.crash()


    signed_iso = "${signedIso}/iso/${isoName}"

    with subtest("the signed medium boots under Secure Boot and says so from inside"):
        m = firmware(signed_iso, "${lososVars}", "signed")
        m.start()
        m.wait_for_unit("multi-user.target")
        # The installer's menu on tty1, with its Secure Boot line.
        m.wait_until_tty_matches("1", "Choose the firmware mode")
        m.wait_until_tty_matches("1", "Secure Boot: enabled")
        m.screenshot("01-signed-tty1-installer")

        status = m.succeed("bootctl status")
        print(status)
        assert "Secure Boot: enabled (user)" in status, status
        # The firmware's own word: SecureBoot = 1 (byte after the 4 attribute bytes).
        assert m.succeed(f"od -An -tu1 -j4 -N1 {SB_VAR}").split() == ["1"]
        # The loader the firmware verified was the UKI's systemd-stub, not GRUB.
        m.succeed(f"test -e {STUB_VAR}")
        assert "systemd-stub" in status, status
        # The kernel saw the same.
        m.succeed("dmesg | grep -i 'secure boot enabled'")
        # Stage 1 hashed the system image against the signed command line.
        m.succeed("grep -q losos.medium.sha256= /proc/cmdline")
        m.succeed("journalctl -b -u losos-verify-medium | grep 'matches the signed hash'")
        # The certificate in db is the test signer, by name.
        db = m.succeed("efi-readvar -v db")
        print(db)
        assert "LosOS test db" in db, db

        # The same facts on the screen, for the record. tty8: the medium
        # autologs root in on every console logind spawns a getty for
        # (tty1-tty6), and agetty hangs the tty up as it starts, which
        # turns a write already in flight on tty2 into EIO.
        m.succeed(
            "{ echo '# bootctl status | head -12'; bootctl status 2>/dev/null | head -12;"
            " echo; echo '# od -An -tu1 -j4 -N1 " + SB_VAR + "'; od -An -tu1 -j4 -N1 " + SB_VAR + ";"
            " echo; echo '# efi-readvar -v db | grep -A1 -m1 Subject'; efi-readvar -v db | grep -A1 -m1 Subject; } > /dev/tty8"
        )
        m.succeed("chvt 8")
        m.wait_until_tty_matches("8", "Secure Boot: enabled")
        m.screenshot("02-signed-tty8-proof")
        m.shutdown()

    with subtest("a firmware with only Microsoft's keys refuses the signed medium"):
        refused(signed_iso, "${msVars}", "foreign", "03-refused-microsoft-keys")

    with subtest("a tampered copy of the signed medium is refused"):
        refused("${tamperedIso}/iso/${isoName}", "${lososVars}", "tampered", "04-refused-tampered")

    with subtest("the unsigned nix build output is refused"):
        refused("${testIso}/iso/${isoName}", "${lososVars}", "unsigned", "05-refused-unsigned")

    with subtest("a signed medium whose system image was altered is refused by stage 1"):
        m = firmware("${tamperedStoreIso}/iso/${isoName}", "${lososVars}", "tampered-store")
        m.start()
        # The firmware starts it (the loader is intact) ...
        m.wait_for_console_text("verifying the medium's system image")
        # ... and stage 1 stops it.
        m.wait_for_console_text("THE MEDIUM HAS BEEN ALTERED")
        # Let the framebuffer catch up with the serial line before the shot
        # (time.sleep: the machine's own sleep() runs a command in a guest
        # that is powering off).
        time.sleep(2)
        m.screenshot("06-refused-tampered-store")
        m.wait_for_shutdown()
  '';
}
