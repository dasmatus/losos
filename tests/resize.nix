# nixos-test-vms config for online /persist growth (`losos-ctl grow`).
#
# The appliance stores /nix on /persist — impermanence binds /persist/nix over
# /nix — so the store grows with every generation and the 03:00 unattended
# rebuild is what eventually fills it, on a box with no SSH and no shell. The
# layout is built to survive that: ext4 inside LUKS inside an LV, which is
# exactly the stack that grows without unmounting, and modules/disko.nix leaves
# part of the volume group unallocated (losos.storage.fillPercent) so there is
# something to grow into.
#
# None of that was ever exercised. This test is the gate.
#
# It builds the real stack at runtime rather than through disko, because what
# is under test is the resize path, not the partitioner — tests/install.nix
# already drives disko end to end. The three steps run through **lososd**, over
# D-Bus, via the same `losos-ctl grow` an admin would reach for, so the unit's
# PATH and the daemon's subprocess handling are covered too.
#
# The assertion that matters is not "the command exited 0". Every way this goes
# wrong goes wrong quietly:
#
#   * resize2fs asks the *mapping* how big it is. Run before `cryptsetup
#     resize` it reads the pre-grow size, prints "The filesystem is already N
#     blocks long. Nothing to do!" and exits 0.
#   * `lvextend -l 77` without the plus is an absolute extent count, so on a
#     volume that already has more it silently *shrinks* — under a mounted
#     ext4, that destroys it.
#
# So this asserts measured sizes, that /persist never left the mount table, and
# that a file written before the grow is still readable after it.
{ pkgs }:

let
  lososPkgs = import ../flake/packages.nix { inherit pkgs; };
in

pkgs.testers.nixosTest {
  name = "losos-resize";

  nodes.machine =
    { ... }:
    {
      imports = [
        ../modules/options.nix
        ../modules/daemon.nix
      ];

      losos.backend.package = lososPkgs.losos-ctl;

      # The no-TPM path, which is what makes modules/daemon.nix set
      # LOSOS_LUKS_KEYFILE=/etc/keys/persist-keyfile. Exercising the real
      # wiring rather than overriding the environment here is the point: the
      # key file and the unit that reads it have to agree, and a test that sets
      # its own path would not notice if they stopped agreeing.
      #
      # It also has to be a path lososd can actually reach. The unit runs with
      # ProtectHome=true — deliberately, so a compromised request handler
      # cannot read either data domain — and that hides /root as well as
      # /home. An earlier version of this test put the key in /root and the
      # daemon failed with "Failed to open key file", after lvextend had
      # already grown the volume.
      losos.tpm.enable = false;

      # The same tools modules/daemon.nix puts on lososd's PATH, for the setup
      # the test script does by hand before handing over to the daemon.
      environment.systemPackages = with pkgs; [
        lvm2
        cryptsetup
        e2fsprogs
      ];

      virtualisation = {
        memorySize = 1024;
        cores = 2;
        # One spare disk to build the volume group on. 4 GiB is enough to see a
        # grow that df reports in whole gigabytes.
        emptyDiskImages = [ 4096 ];
      };
    };

  testScript = ''
    machine.start()
    machine.wait_for_unit("multi-user.target")
    machine.wait_for_unit("lososd.service")
    machine.wait_until_succeeds("losos-ctl state --json")

    # ── Build the appliance's real stack: LVM -> LUKS -> ext4 ───────────
    # Deliberately NOT 100%FREE: the headroom is what the grow will claim, and
    # it is the whole reason modules/disko.nix stops short of the full volume
    # group.
    with subtest("an LVM volume group with headroom, LUKS, and ext4 on top"):
        machine.succeed("pvcreate -ff -y /dev/vdb")
        machine.succeed("vgcreate persist-vg /dev/vdb")
        machine.succeed("lvcreate -l 50%FREE -n persist persist-vg")

        # /etc/keys/persist-keyfile — the same path modules/disko.nix uses on
        # the no-TPM path and modules/daemon.nix hands to the daemon. Not
        # /root: lososd runs with ProtectHome=true and cannot see it there.
        machine.succeed("install -d -m 0700 /etc/keys")
        machine.succeed("echo -n hunter2 > /etc/keys/persist-keyfile")
        machine.succeed("chmod 0600 /etc/keys/persist-keyfile")
        machine.succeed(
            "cryptsetup luksFormat --batch-mode --key-file /etc/keys/persist-keyfile"
            " /dev/persist-vg/persist"
        )
        machine.succeed(
            "cryptsetup open --key-file /etc/keys/persist-keyfile"
            " /dev/persist-vg/persist persist"
        )
        # -O encrypt mirrors modules/disko.nix: fscrypt needs it and it can
        # only be set at mkfs time.
        machine.succeed("mkfs.ext4 -F -O encrypt /dev/mapper/persist")
        machine.succeed("mkdir -p /persist && mount /dev/mapper/persist /persist")
        machine.succeed("mountpoint -q /persist")

    with subtest("there is genuinely free space to grow into"):
        free = int(machine.succeed(
            "vgs --noheadings --nosuffix -o vg_free_count persist-vg"
        ).strip())
        assert free > 0, "the volume group has no free extents; the test proves nothing"

    # Data written before the grow. ext4 online resize is supposed to be
    # non-destructive; this is what checks that claim rather than assuming it.
    machine.succeed("echo persist-marker > /persist/marker")
    before = int(machine.succeed("df -B1 --output=size /persist | tail -1").strip())
    print(f"/persist before: {before} bytes")

    # ── The thing under test ────────────────────────────────────────────
    with subtest("losos-ctl grow extends the filesystem, online"):
        out = machine.succeed("losos-ctl grow")
        print("grow:", out)
        assert '"grew":true' in out.replace(" ", ""), \
            f"the daemon itself says nothing grew: {out!r}"

    with subtest("the filesystem really is bigger"):
        # Measured independently of the daemon's own report, because the
        # daemon's report is exactly what would be wrong if this were broken.
        after = int(machine.succeed("df -B1 --output=size /persist | tail -1").strip())
        print(f"/persist after: {after} bytes")
        assert after > before, f"/persist did not grow: {before} -> {after}"

    with subtest("it never left the mount table"):
        # An offline resize would have worked too, and would be useless on an
        # appliance that cannot be taken down to do it.
        machine.succeed("mountpoint -q /persist")

    with subtest("the data survived"):
        assert machine.succeed("cat /persist/marker").strip() == "persist-marker"

    with subtest("the volume group is now fully claimed"):
        free = int(machine.succeed(
            "vgs --noheadings --nosuffix -o vg_free_count persist-vg"
        ).strip())
        assert free == 0, f"{free} extents left unclaimed after a grow"

    with subtest("a second grow refuses instead of pretending"):
        # Nothing left to take. This must be an error, not a cheerful no-op —
        # an admin who is out of space needs to be told to add a disk, and
        # there is no shell here to go and work it out from.
        err = machine.fail("losos-ctl grow 2>&1")
        print("second grow:", err)
        assert "no free extents" in err, f"unhelpful refusal: {err!r}"
        assert "vgextend" in err or "fillPercent" in err, \
            f"the refusal does not say what to do about it: {err!r}"
  '';
}
