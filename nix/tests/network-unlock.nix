# nixos-test-vms for losos.tpm.tang: the disk opens with the TPM2 AND a Tang
# server on the box's network, and without the server only with the recovery
# code typed at the console.
#
# The same seam as nix/tests/tpm.nix (the installer's --disko-script mode on
# a swtpm VM, then a reboot into a specialisation carrying the production
# disko.nix + boot.nix + network-unlock.nix initrd), plus a second machine on
# the test network that plays the home router: it hands out addresses
# (dnsmasq) and runs Tang. Asserted:
#   1. `losos-install --tang URL#THUMBPRINT` writes the server into the
#      target file, binds the key with Clevis, and enrols no systemd TPM2
#      token (the header carries only the keyfile slot),
#   2. the next boot opens /persist unattended: the initrd took an address by
#      DHCP without sending a host name, and the Clevis unit got the key,
#   3. losos-unlock-recovery adds the recovery code as a passphrase as soon
#      as the code file exists, and swaps it when the code changes,
#   4. with the router off the network, the boot stops at the console prompt
#      and the recovery code opens it.
{
  pkgs,
  disko,
}:

let
  tangPort = 7500;
  # The address the router gets on the test network (machines are numbered
  # in name order: machine is .1, router is .2).
  tangUrl = "http://192.168.1.2:${toString tangPort}";
in
pkgs.testers.nixosTest {
  name = "losos-network-unlock";

  nodes.router = {
    services.tang = {
      enable = true;
      listenStream = [ (toString tangPort) ];
      # Loopback for tang-show-keys on the router itself.
      ipAddressAllow = [
        "127.0.0.1/32"
        "192.168.1.0/24"
      ];
    };
    services.dnsmasq = {
      enable = true;
      settings = {
        interface = "eth1";
        bind-interfaces = true;
        port = 0;
        dhcp-range = [ "192.168.1.100,192.168.1.200,255.255.255.0,1h" ];
        log-dhcp = true;
      };
    };
    networking.firewall.allowedTCPPorts = [ tangPort ];
    networking.firewall.allowedUDPPorts = [ 67 ];
    environment.systemPackages = [
      pkgs.tang
      pkgs.curl
    ];
  };

  nodes.machine =
    {
      config,
      lib,
      pkgs,
      ...
    }:
    let
      lososPkgs = import ../packages.nix { inherit pkgs; };
    in
    {
      imports = [
        # nix/tests/virtiofs.nix: the closure this node installs is too many
        # inodes for virtiofsd under a 20000-descriptor limit.
        (import ./virtiofs.nix pkgs)
        disko.nixosModules.disko
        ../../modules/options.nix
        ../../modules/disko.nix
        ../../modules/boot.nix
        ../../modules/network-unlock.nix
        ../../modules/installer.nix
      ];

      losos.installer.package = lososPkgs.losos-ctl;
      losos.targetDrives = [ "/dev/vdb" ];

      # As in tpm.nix: disko's generated config goes in through the VM's own
      # options at the VM's priority, in the specialisation only.
      disko.enableConfig = false;

      specialisation.unlock.configuration =
        { config, lib, ... }:
        let
          generated = config.disko.devices._config;
          persistFs = map (c: c."/persist") (
            builtins.filter (c: c ? "/persist") generated.fileSystems.contents
          );
        in
        {
          # The installed box's install-target.nix. The thumbprint here is a
          # stand-in of the right shape: the router makes its key at its first
          # start, so the real one is known only at run time, and the
          # installed system reads nothing from it but "there is a server"
          # (the JWE carries the real one). The installer, which binds to it,
          # gets the real one on its command line.
          losos.tpm.tang = [
            {
              url = tangUrl;
              thumbprint = "known-only-at-run-time-0000";
            }
          ];
          virtualisation.fileSystems."/persist" = lib.mkMerge (persistFs ++ [ { neededForBoot = true; } ]);
          boot.initrd.luks.devices = lib.mkMerge (
            map (
              c: lib.mkVMOverride (lib.attrByPath [ "initrd" "luks" "devices" ] { } c)
            ) generated.boot.contents
          );
        };

      boot.loader.efi.canTouchEfiVariables = lib.mkForce false;
      system.switch.enable = true;

      environment.etc."losos/disko-script".source = config.system.build.diskoScript;

      services.lvm.enable = true;
      environment.systemPackages = with pkgs; [
        lvm2
        cryptsetup
        util-linux
        clevis
        jose
      ];

      virtualisation = {
        memorySize = 2048;
        cores = 2;
        useBootLoader = true;
        useEFIBoot = true;
        tpm.enable = true;
        emptyDiskImages = [ 4096 ];
      };
    };

  testScript = ''
    import re

    router.start()
    router.wait_for_unit("dnsmasq.service")
    router.wait_for_unit("tangd.socket")
    router.wait_until_succeeds("curl -sf http://127.0.0.1:${toString tangPort}/adv")
    thumbprint = router.succeed("tang-show-keys ${toString tangPort}").strip()
    assert re.fullmatch(r"[A-Za-z0-9_-]{20,}", thumbprint), thumbprint
    tang = f"${tangUrl}#{thumbprint}"

    machine.start()
    machine.wait_for_unit("multi-user.target")

    with subtest("--tang goes into the target file, and needs the TPM"):
        machine.succeed(f"losos-install --tang '{tang}' --emit-target /tmp/tang.nix")
        target = machine.succeed("cat /tmp/tang.nix")
        assert f'losos.tpm.tang = [\n    {{ url = "${tangUrl}"; thumbprint = "{thumbprint}"; }}\n  ];' in target, target
        machine.fail(f"losos-install --no-tpm --tang '{tang}' --emit-target /tmp/no.nix")
        machine.fail("losos-install --tang http://192.168.1.2#short --emit-target /tmp/bad.nix")

    with subtest("format, mount and bind to the TPM and the server, with nothing typed"):
        machine.succeed(f"losos-install --tang '{tang}' --disko-script /etc/losos/disko-script < /dev/null")
        machine.succeed("test -s /etc/keys/persist.jwe")
        assert machine.succeed("stat -c %a /etc/keys/persist.jwe").strip() == "600"
        dump = machine.succeed("cryptsetup luksDump /dev/persist-vg/persist")
        assert "systemd-tpm2" not in dump, dump
        # The JWE gives back the very key the volume was formatted with.
        machine.succeed("clevis decrypt < /etc/keys/persist.jwe | cmp - /etc/keys/persist-keyfile")
        # What install_tail does on the real path, which the seam stops before.
        machine.succeed(
            "install -d -m 700 /mnt/persist/etc/keys",
            "install -m 600 /etc/keys/persist-keyfile /mnt/persist/etc/keys/persist-keyfile",
            "install -m 600 /etc/keys/persist.jwe /mnt/persist/etc/keys/persist.jwe",
            "sync",
            "umount -R /mnt",
            "cryptsetup close persist",
        )

    with subtest("the JWE rides in the unlock entry's initrd"):
        # The image build had no JWE to add; the next bootloader install does,
        # as a rebuild on the box does.
        machine.succeed("/run/current-system/bin/switch-to-configuration boot")
        entry = machine.succeed("grep -l '^title NixOS (unlock)$' /boot/loader/entries/*.conf").strip()
        assert entry.count("\n") == 0, entry
        entry_id = entry.rsplit("/", 1)[1]
        machine.succeed(f"bootctl set-default {entry_id}")
        machine.succeed(f"sed -i 's|^default .*|default {entry_id}|' /boot/loader/loader.conf")
        machine.succeed("sync")

    with subtest("with the server on the network the box opens unattended"):
        machine.crash()
        machine.start()
        machine.wait_for_unit("multi-user.target")
        assert machine.succeed("findmnt -no SOURCE /persist").strip() == "/dev/mapper/persist"
        clevis_log = machine.succeed("journalctl -b -u cryptsetup-clevis-persist.service")
        assert "Finished" in clevis_log, clevis_log
        assert "the disk asks" not in clevis_log, clevis_log
        unlock_log = machine.succeed("journalctl -b -u systemd-cryptsetup@persist.service")
        assert "Please enter" not in unlock_log, unlock_log
        # The router handed the initrd an address, and was not told a name.
        leases = router.succeed("journalctl -u dnsmasq.service")
        assert "DHCPACK" in leases, leases
        assert "client provides name" not in leases, leases
        assert "machine" not in leases, leases

    code = "6f1c2a8e-4b3d-4f0a-9c1e-2d7b5a9e8f10"
    new_code = "0b9d7e3c-1a2f-4c5d-8e6f-7a8b9c0d1e2f"

    with subtest("the recovery code becomes the console passphrase, and moves with it"):
        machine.succeed("install -d -m 700 /var/secrets")
        machine.succeed(f"printf '%s' {code} > /var/secrets/losos-recovery-code")
        machine.wait_until_succeeds("test -s /var/lib/losos-unlock/slot")
        machine.succeed(f"printf '%s' {code} | cryptsetup open --test-passphrase --key-file=- /dev/persist-vg/persist")
        first = machine.succeed("cat /var/lib/losos-unlock/slot").strip()
        machine.succeed(f"printf '%s' {new_code} > /var/secrets/losos-recovery-code")
        machine.wait_until_succeeds(
            f"printf '%s' {new_code} | cryptsetup open --test-passphrase --key-file=- /dev/persist-vg/persist"
        )
        machine.wait_until_fails(
            f"printf '%s' {code} | cryptsetup open --test-passphrase --key-file=- /dev/persist-vg/persist"
        )
        machine.log(f"recovery slot {first} -> " + machine.succeed("cat /var/lib/losos-unlock/slot").strip())
        # The keyfile slot is still there.
        machine.succeed("cryptsetup open --test-passphrase --key-file /etc/keys/persist-keyfile /dev/persist-vg/persist")

    with subtest("off the network the disk asks, and the recovery code opens it"):
        router.shutdown()
        machine.crash()
        machine.start()
        machine.wait_for_console_text("Please enter")
        machine.send_console(f"{new_code}\n")
        machine.wait_for_unit("multi-user.target")
        assert machine.succeed("findmnt -no SOURCE /persist").strip() == "/dev/mapper/persist"
        clevis_log = machine.succeed("journalctl -b -u cryptsetup-clevis-persist.service")
        assert "no Tang server answered" in clevis_log, clevis_log
  '';
}
