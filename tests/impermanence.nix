# nixos-test-vms config for the stateless-by-impermanence model
# (modules/impermanence.nix + modules/configuration.nix).
#
# The whole appliance rests on one claim: the root is a tmpfs rebuilt every
# boot, and *only* the paths listed in modules/impermanence.nix come back from
# /persist. Nothing tested it, so a dropped entry — the failure mode CLAUDE.md
# warns about, "it silently vanishes on the next boot" — would surface as data
# loss on a box with no shell to notice it from.
#
# The VM boots with a tmpfs root and a real /persist, writes markers, is shut
# down and started again on the same disk image, and then asserts:
#   1. everything the appliance must keep is still there — lososd's state.json,
#      the admin token, the two service state dirs, both data homes, machine-id;
#   2. a file written outside the persisted set is *gone*, which is what proves
#      the root is genuinely ephemeral and the test is not passing on a VM that
#      simply never threw anything away;
#   3. permissions survive: the admin token is still 0600 and the two data
#      homes are still 700, owned by their respective accounts.
#
# The last two subtests measure the isolation claim itself. It did not hold
# when this test was written: both accounts were isNormalUser with no `group`,
# so nixpkgs put both in `users`, and the homes' 750 granted r-x to exactly
# that group — `shared` could read /home/notshared/data. The fix was a
# per-account primary group and 700 homes, and these subtests are what stop it
# regressing.
#
# Two deliberate departures from the installed system, both about the VM rather
# than the property under test:
#
#   * /persist is a plain ext4 disk, not LUKS-on-LVM. The encryption is
#     disko.nix's and boot.nix's business (and tests/install.nix already drives
#     the real format path); what is under test here is the bind-mount
#     persistence, which is identical either way.
#   * /nix is dropped from the persisted set. A nixos-test VM gets its store
#     from the host over 9p, mounted at /nix/.ro-store and bind-mounted to
#     /nix/store before switch-root; bind-mounting /persist/nix over /nix would
#     hide it and the VM could not exec its own init. The list is *derived*
#     from modules/impermanence.nix and filtered, not retyped, so an entry
#     dropped there still fails this test.
{ pkgs, impermanence }:

let
  # The real persistence spec, read straight out of the module. It is a
  # `_: { ... }` module with no dependency on its arguments, so it can be
  # applied to an empty attrset and inspected.
  lososPersistence = (import ../modules/impermanence.nix { }).environment.persistence."/persist";
  persistedDirs = builtins.filter (d: d != "/nix") lososPersistence.directories;
in

pkgs.testers.nixosTest {
  name = "losos-impermanence";

  nodes.machine =
    { lib, ... }:
    {
      imports = [
        impermanence.nixosModules.impermanence
        ../modules/options.nix
        # The users, their uids, their groups and the 700 home modes are
        # declared here; the isolation claim is about this file as much as
        # about the persistence.
        ../modules/configuration.nix
      ];

      environment.persistence."/persist" = {
        inherit (lososPersistence) hideMounts files;
        directories = persistedDirs;
      };

      # /dev/dri does not exist in the VM, and the graphics stack is a large
      # closure for nothing here.
      losos.gpu.enable = false;

      # Neither is under test and both slow the boot down (NetworkManager also
      # fights the test framework's own interface setup).
      networking.networkmanager.enable = lib.mkForce false;
      services.avahi.enable = lib.mkForce false;

      # modules/configuration.nix only gives the two tahoe accounts a group —
      # the accounts themselves come from services.tahoe in modules/services.nix,
      # which is not imported here (it would pull in Nextcloud, Forgejo and a
      # tahoe-lafs rebuilt against Python 3.12). Without a kind, nixpkgs'
      # "exactly one of isSystemUser/isNormalUser" assertion fires.
      users.users."tahoe.shared".isSystemUser = true;
      users.users."tahoe.introducer-local".isSystemUser = true;

      # An unprivileged third account, to check the homes keep out someone who
      # is neither owner (see the isolation subtest).
      users.users.outsider = {
        isSystemUser = true;
        group = "nogroup";
      };

      virtualisation = {
        memorySize = 1024;
        cores = 2;
        # The root disk image is created pre-formatted ext4 and, unlike the
        # tmpfs root, is kept across a shutdown/start of the same test — so it
        # plays the part of the encrypted /persist volume. "/" is forced to
        # tmpfs on top of the default that would otherwise mount this disk
        # there.
        diskSize = 3072;
        fileSystems."/" = lib.mkForce {
          device = "tmpfs";
          fsType = "tmpfs";
          options = [ "mode=755" ];
        };
        fileSystems."/persist" = {
          device = "/dev/vda";
          fsType = "ext4";
          # impermanence resolves its bind mounts against /persist, so it has
          # to be there before the sysroot is populated — same reason
          # modules/boot.nix marks it neededForBoot on the real box.
          neededForBoot = true;
        };
      };

      # /var is one of NixOS' pathsNeededForBoot, so impermanence binds it from
      # the initrd — and its systemd-initrd path does not create the source
      # directory. On a real box `nixos-install` made /persist/var while
      # /persist was still just a mounted disk; a fresh test disk has nobody to
      # do that, so do it here. Only the empty directory: what goes *in* it is
      # exactly what the test is checking.
      boot.initrd.systemd.services.losos-test-persist-dirs = {
        description = "Seed /persist with the dirs nixos-install would have made";
        wantedBy = [ "initrd.target" ];
        requires = [ "sysroot-persist.mount" ];
        after = [ "sysroot-persist.mount" ];
        before = [ "sysroot-var.mount" ];
        unitConfig.DefaultDependencies = false;
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
        };
        script = "mkdir -p /sysroot/persist/var";
      };
    };

  testScript = ''
    machine.start()
    machine.wait_for_unit("multi-user.target")

    with subtest("the root really is a tmpfs"):
        assert machine.succeed("findmnt -no FSTYPE /").strip() == "tmpfs", \
            "/ is not tmpfs — the rest of this test would pass for free"
        machine.succeed("mountpoint -q /persist")

    with subtest("the persisted directories are bind mounts from /persist"):
        # impermanence realises every persisted directory as a bind mount, so a
        # path that is not a mountpoint is not coming back from /persist — say
        # that here rather than as a confusing "no such file" after the reboot.
        for d in ("/var", "/home/notshared/data", "/home/shared/data"):
            machine.succeed(f"mountpoint -q {d}")

    # Everything the appliance must not lose. The paths are the ones the
    # modules actually use: lososd's state file and the token it mints
    # (modules/daemon.nix), the two nspawn state dirs bind-mounted into the
    # containers (modules/containers.nix), and the two data homes.
    machine.succeed(
        "install -d -m 0755 /var/lib/losos",
        "echo '{\"mode\":\"local\"}' > /var/lib/losos/state.json",
        "install -d -m 0700 /var/secrets",
        "head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \\n' > /var/secrets/losos-admin-token",
        "chmod 0600 /var/secrets/losos-admin-token",
        "install -d /var/lib/nextcloud /var/lib/forgejo",
        "echo nextcloud-state > /var/lib/nextcloud/marker",
        "echo forgejo-state > /var/lib/forgejo/marker",
        "echo notshared-doc > /home/notshared/data/marker",
        "echo shared-doc > /home/shared/data/marker",
        # The rest of the module's list. /etc/keys carries the LUKS keyfile on
        # the no-TPM path and /etc/nixos the flake system.autoUpgrade rebuilds
        # from, so losing either bricks the box in a way nobody can log in to
        # diagnose.
        "echo host-key > /etc/ssh/marker",
        "echo luks-keyfile > /etc/keys/marker",
        "echo flake-source > /etc/nixos/marker",
    )

    # ... and files on the parts of the root that are *not* persisted. If these
    # came back, the VM would not be modelling the appliance at all.
    machine.succeed(
        "echo ephemeral > /root/scratch",
        "install -d /srv && echo ephemeral > /srv/scratch",
        "echo ephemeral > /etc/losos-scratch",
    )

    token = machine.succeed("cat /var/secrets/losos-admin-token").strip()
    machine_id = machine.succeed("cat /etc/machine-id").strip()
    assert len(machine_id) == 32, f"unexpected machine-id: {machine_id!r}"

    # A full power cycle, not just a `reboot`: the tmpfs root is thrown away
    # and rebuilt from the closure, and the only thing carried over is the
    # /persist disk image.
    machine.shutdown()
    machine.start()
    machine.wait_for_unit("multi-user.target")

    with subtest("the persisted set survives the reboot"):
        assert machine.succeed("cat /var/lib/losos/state.json").strip() == '{"mode":"local"}'
        assert machine.succeed("cat /var/lib/nextcloud/marker").strip() == "nextcloud-state"
        assert machine.succeed("cat /var/lib/forgejo/marker").strip() == "forgejo-state"
        assert machine.succeed("cat /home/notshared/data/marker").strip() == "notshared-doc"
        assert machine.succeed("cat /home/shared/data/marker").strip() == "shared-doc"
        assert machine.succeed("cat /var/secrets/losos-admin-token").strip() == token, \
            "the admin token changed across the reboot"
        assert machine.succeed("cat /etc/ssh/marker").strip() == "host-key"
        assert machine.succeed("cat /etc/keys/marker").strip() == "luks-keyfile"
        assert machine.succeed("cat /etc/nixos/marker").strip() == "flake-source"
        assert machine.succeed("cat /etc/machine-id").strip() == machine_id, \
            "/etc/machine-id was regenerated — it is in the persisted `files` list"

    with subtest("the volatile root is genuinely volatile"):
        for path in ("/root/scratch", "/srv/scratch", "/etc/losos-scratch"):
            machine.fail(f"test -e {path}")

    with subtest("permissions survive too"):
        mode = machine.succeed("stat -c %a /var/secrets/losos-admin-token").strip()
        assert mode == "600", f"admin token is mode {mode}, expected 600"
        for home in ("/home/notshared", "/home/shared"):
            mode = machine.succeed(f"stat -c %a {home}").strip()
            assert mode == "700", f"{home} is mode {mode}, expected 700"
        assert machine.succeed("stat -c %U /home/notshared").strip() == "notshared"
        assert machine.succeed("stat -c %U /home/shared").strip() == "shared"

    with subtest("the home modes keep every other account out"):
        machine.fail("runuser -u outsider -- cat /home/notshared/data/marker")
        machine.fail("runuser -u outsider -- cat /home/shared/data/marker")

    with subtest("the two data domains are isolated from each other"):
        # The headline claim: "the two users mutually unreadable".
        #
        # It did not hold. `isNormalUser` defaults the primary group to `users`
        # and `homeMode = "750"` grants r-x to the primary group, so both
        # accounts shared a group and each could read the other's data. The 750
        # kept out accounts *outside* `users` (subtest above) and nothing else.
        #
        # The fix is a per-account primary group, and this subtest is what stops
        # it regressing: drop `group = "notshared"` from configuration.nix and
        # the two `machine.fail` lines below start succeeding.
        assert machine.succeed("stat -c %G /home/notshared").strip() == "notshared"
        assert machine.succeed("stat -c %G /home/shared").strip() == "shared"
        machine.fail("runuser -u shared -- cat /home/notshared/data/marker")
        machine.fail("runuser -u notshared -- cat /home/shared/data/marker")
        # Each still reaches its own, so the isolation is not just a broken
        # permission that locks everyone out including the owner.
        machine.succeed("runuser -u notshared -- cat /home/notshared/data/marker")
        machine.succeed("runuser -u shared -- cat /home/shared/data/marker")
  '';
}
