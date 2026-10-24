# Exercises the same systemd-sysupdate directory-transfer mode as LosOS against
# a real virtio block device formatted ext4 and mounted at /persist. The archive
# is sourced from the immutable Nix store, but its extracted version must land
# on the virtual hard drive, never /tmp or a tmpfs.
{ pkgs }:

let
  archive = pkgs.runCommand "losos-staging_v1.tar" { } ''
    mkdir payload
    printf '%s\n' 'persistent-update-payload' > payload/SYSUPDATE_TEST_SENTINEL
    tar -C payload -cf - SYSUPDATE_TEST_SENTINEL | xz -6 -c > "$out"
  '';
  sourceDir = pkgs.runCommand "losos-sysupdate-test-source" { } ''
    mkdir -p "$out"
    cp ${archive} "$out/losos-staging_v1.tar.xz"
  '';
in
pkgs.testers.nixosTest {
  name = "losos-sysupdate-staging-on-disk";

  nodes.machine = { pkgs, ... }: {
    virtualisation.emptyDiskImages = [ 2048 ];

    # The NixOS test harness attaches the extra image as /dev/vdb. Format it
    # in the systemd initrd before switching to stage 2, where /persist mounts.
    boot.initrd.supportedFilesystems = [ "ext4" ];
    boot.initrd.systemd.extraBin.mke2fs = "${pkgs.e2fsprogs}/bin/mke2fs";
    boot.initrd.systemd.services.format-persist-test-disk = {
      description = "Format the sysupdate staging test disk";
      wantedBy = [ "initrd.target" ];
      requires = [ "dev-vdb.device" ];
      after = [ "dev-vdb.device" ];
      unitConfig.DefaultDependencies = false;
      serviceConfig.Type = "oneshot";
      script = ''
        mke2fs -t ext4 -F /dev/vdb
      '';
    };
    fileSystems."/persist" = {
      device = "/dev/vdb";
      fsType = "ext4";
    };

    environment.systemPackages = [ pkgs.systemd pkgs.e2fsprogs pkgs.util-linux ];
    environment.etc."sysupdate-test-source".source = sourceDir;
    environment.etc."sysupdate-test.d/50-test.transfer".text = ''
      [Transfer]
      [Source]
      Type=tar
      Path=/etc/sysupdate-test-source
      MatchPattern=losos-staging_@v.tar.xz

      [Target]
      Type=directory
      Path=/persist/var/lib/losos-sysupdate-test/versions
      MatchPattern=losos-staging_@v
      InstancesMax=2
    '';
    systemd.tmpfiles.rules = [
      "d /persist/var/lib/losos-sysupdate-test 0755 root root - -"
      "d /persist/var/lib/losos-sysupdate-test/versions 0755 root root - -"
    ];

    system.stateVersion = "25.05";
  };

  testScript = ''
    machine.start()
    machine.wait_for_unit("multi-user.target")
    machine.succeed("test \"$(findmnt -n -o FSTYPE --target /persist)\" = ext4")
    machine.succeed("systemd-sysupdate --definitions=/etc/sysupdate-test.d update")
    machine.succeed("test -f /persist/var/lib/losos-sysupdate-test/versions/losos-staging_v1/SYSUPDATE_TEST_SENTINEL")
    machine.succeed("test \"$(cat /persist/var/lib/losos-sysupdate-test/versions/losos-staging_v1/SYSUPDATE_TEST_SENTINEL)\" = persistent-update-payload")
    machine.succeed("test \"$(findmnt -n -o FSTYPE --target /persist/var/lib/losos-sysupdate-test/versions/losos-staging_v1)\" = ext4")
    machine.succeed("test ! -e /tmp/SYSUPDATE_TEST_SENTINEL")
  '';
}
