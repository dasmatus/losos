# nixos-test-vms config for the signed installer medium under Secure Boot
# (modules/secure-boot.nix, losos-sign-iso).
#
# Boots the real installer ISO — the same `nixosConfigurations.iso` the
# release ships, extended only with the test backdoor so the script can look
# inside — from a virtual USB stick, under OVMF's Secure Boot build with SMM.
#
# The firmware under test holds the LosOS certificate *and* Microsoft's: its
# KEK has the test KEK beside Microsoft's two KEK CAs, its db the test db
# certificate beside Microsoft's five (the two Windows CAs, the two
# third-party UEFI CAs, the 2023 option ROM CA). That is the state the
# handbook tells an owner to leave their PC in, because Windows, other
# distributions' shim and the option ROMs on graphics and network cards are
# signed under Microsoft's certificates, and a db without them stops those.
#
#   1. signed, firmware trusts both                  → boots; from inside, the
#      firmware says Secure Boot is on, the loader was systemd-stub, and db
#      and KEK list the test certificates beside Microsoft's;
#   2. a Microsoft-signed loader (Ubuntu's shim)     → the same firmware
#      starts it: enrolling LosOS took nothing away;
#   3. the same shim, firmware with LosOS keys only  → refused, which is
#      what replacing Microsoft's keys instead of adding to them costs;
#   4. signed, firmware holds only Microsoft's keys  → refused ("Access
#      Denied"), which is what a stock PC does before its owner enrols the
#      LosOS certificate;
#   5. signed, one byte of the loader changed        → refused: the signature
#      covers the whole image, so a tampered medium does not start;
#   6. unsigned (the plain `nix build` output)       → refused;
#   7. signed, one byte of the system image changed  → the firmware starts
#      it (the loader is intact), and stage 1 refuses it: the squashfs no
#      longer matches the hash the signed command line carries.
#
# The LosOS keys are THROWAWAY: a PK, a KEK and a db certificate generated in
# the build sandbox below, enrolled into a copy of OVMF's variable store with
# virt-fw-vars, and used for nothing else. Microsoft's certificates are the
# public ones virt-firmware ships. They are not the production key,
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
        # Path arithmetic, not "${pkgs.path}/…": interpolating copies the
        # nixpkgs source into a second store path, which CI's evaluator (with
        # nixpkgs fetched as a github: tarball) then fails to realise.
        ({ modulesPath, ... }: { imports = [ (modulesPath + "/testing/test-instrumentation.nix") ]; })
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

  # Owner GUID the LosOS certificates carry; any fixed value works except
  # Microsoft's own (77fa9abd-…), which their certificates carry.
  guid = "a58ba000-6821-482f-acda-601cb601d98e";
  msGuid = "77fa9abd-0359-4d32-bd60-28f4e78f784b";

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

  # OVMF's empty variable store with the throwaway PK/KEK/db enrolled,
  # Microsoft's KEK and db certificates added beside them, and Secure Boot
  # switched on: a PC whose owner followed the handbook.
  lososVars = ovmfVars "ovmf-vars-losos-and-microsoft" true;

  # The same without Microsoft's certificates: what `sbctl enroll-keys`
  # without `--microsoft`, or a firmware's "delete all keys" before adding
  # LosOS, leaves behind. Only subtest 3 uses it.
  lososOnlyVars = ovmfVars "ovmf-vars-losos-only" false;

  ovmfVars =
    name: withMicrosoft:
    pkgs.runCommand name
      {
        nativeBuildInputs = [ pkgs.python3Packages.virt-firmware ];
      }
      ''
        args=(
          --set-pk ${guid} ${keys}/PK.pem
          --add-kek ${guid} ${keys}/KEK.pem
          --add-db ${guid} ${keys}/db.pem
        )
        ${pkgs.lib.optionalString withMicrosoft ''
          # The public certificates virt-firmware carries, by its own names.
          ms=$(python3 -c 'import os, virt.firmware.efi.certs as c; print(os.path.dirname(c.MS_KEK_2011))')
          for f in ms-kek-2011 ms-kek-2023; do
            args+=(--add-kek ${msGuid} "$ms/$f.pem")
          done
          for f in windows-2011 windows-2023 ms-uefi-2011 ms-uefi-2023 ms-uefi-rom-2023; do
            args+=(--add-db ${msGuid} "$ms/$f.pem")
          done
        ''}
        virt-fw-vars --input ${ovmf.variables} --output $out "''${args[@]}" --secure-boot
        virt-fw-vars --input $out --print
      '';

  # A loader Microsoft signed: Ubuntu's shim, signed with Microsoft's
  # third-party UEFI CA, on an EFI system partition of its own as the
  # removable-media path \EFI\BOOT\BOOTX64.EFI. There is no grubx64.efi
  # beside it, so once the firmware has let it run it says it cannot find
  # its second stage, which is the proof that it ran.
  shimDeb = pkgs.fetchurl {
    urls = [
      "https://archive.ubuntu.com/ubuntu/pool/main/s/shim-signed/shim-signed_1.59+15.8-0ubuntu2_amd64.deb"
      "https://launchpad.net/ubuntu/+archive/primary/+files/shim-signed_1.59+15.8-0ubuntu2_amd64.deb"
    ];
    hash = "sha256-+O1xzi2RowS21euEmX+EbzMbVUV4vALb/njhOtisgak=";
  };

  shimDisk =
    pkgs.runCommand "microsoft-signed-shim-disk"
      {
        nativeBuildInputs = [
          pkgs.binutils
          pkgs.dosfstools
          pkgs.mtools
          pkgs.python3Packages.virt-firmware
          pkgs.sbsigntool
          pkgs.util-linux
          pkgs.xz
        ];
      }
      ''
        ar x ${shimDeb} data.tar.xz
        tar -xJf data.tar.xz --wildcards './usr/lib/shim/shimx64.efi.signed*'
        shim=$(ls usr/lib/shim/shimx64.efi.signed.latest 2>/dev/null || ls usr/lib/shim/shimx64.efi.signed* | head -1)
        echo "using $shim"
        # Signed by Microsoft, by one of the two third-party UEFI CAs.
        sbverify --list "$shim"
        ms=$(python3 -c 'import os, virt.firmware.efi.certs as c; print(os.path.dirname(c.MS_KEK_2011))')
        sbverify --cert "$ms/ms-uefi-2011.pem" "$shim" || sbverify --cert "$ms/ms-uefi-2023.pem" "$shim"

        truncate -s 64M disk.img
        echo 'label: gpt
        start=2048, type=C12A7328-F81F-11D2-BA4B-00A0C93EC93B' | sfdisk disk.img
        truncate -s 62M esp.img
        mkfs.vfat -n SHIM esp.img
        mmd -i esp.img ::/EFI ::/EFI/BOOT
        mcopy -i esp.img "$shim" ::/EFI/BOOT/BOOTX64.EFI
        dd if=esp.img of=disk.img bs=1M seek=1 conv=notrunc
        mv disk.img $out
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
        # db and KEK hold the test signer and Microsoft's certificates, by name.
        db = m.succeed("efi-readvar -v db")
        print(db)
        for cn in [
            "LosOS test db",
            "Microsoft Windows Production PCA 2011",
            "Windows UEFI CA 2023",
            "Microsoft Corporation UEFI CA 2011",
            "Microsoft UEFI CA 2023",
            "Microsoft Option ROM UEFI CA 2023",
        ]:
            assert cn in db, (cn, db)
        kek = m.succeed("efi-readvar -v KEK")
        print(kek)
        for cn in ["LosOS test KEK", "Microsoft Corporation KEK CA 2011", "Microsoft Corporation KEK 2K CA 2023"]:
            assert cn in kek, (cn, kek)

        # The same facts on the screen, for the record. tty8: the medium
        # autologs root in on every console logind spawns a getty for
        # (tty1-tty6), and agetty hangs the tty up as it starts, which
        # turns a write already in flight on tty2 into EIO.
        # Each certificate's subject is on the line after "Subject:"; the
        # screen shows only the CN, so every line fits.
        cns = "grep -A1 Subject: | grep -o 'CN=[^,]*'"
        m.succeed(
            "{ echo '# bootctl status | head -12'; bootctl status 2>/dev/null | head -12;"
            " echo; echo '# od -An -tu1 -j4 -N1 " + SB_VAR + "'; od -An -tu1 -j4 -N1 " + SB_VAR + ";"
            " echo; echo '# efi-readvar -v KEK (subjects)'; efi-readvar -v KEK | " + cns + ";"
            " echo; echo '# efi-readvar -v db (subjects)'; efi-readvar -v db | " + cns + "; } > /dev/tty8"
        )
        m.succeed("chvt 8")
        m.wait_until_tty_matches("8", "Secure Boot: enabled")
        m.screenshot("02-signed-tty8-proof")
        m.shutdown()

    with subtest("a Microsoft-signed loader still starts on the same firmware"):
        m = firmware("${shimDisk}", "${lososVars}", "shim")
        m.start()
        # shim ran: it looks for its second stage, which the disk lacks.
        m.wait_for_console_text("grubx64.efi")
        time.sleep(3)
        m.screenshot("07-microsoft-signed-shim-starts")
        m.crash()

    with subtest("without Microsoft's certificates the same loader is refused"):
        refused("${shimDisk}", "${lososOnlyVars}", "shim-losos-only", "08-shim-refused-losos-only")

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
        time.sleep(3)
        m.screenshot("06-refused-tampered-store")
        m.wait_for_shutdown()
  '';
}
