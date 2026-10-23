# QEMU integration test for systemd-sysupdate's persistent target.
#
# The test uses a virtual machine's disk-backed root and a local fixture server
# to exercise the real url-tar extraction path without relying on the public
# update endpoint. It verifies the extracted file survives a reboot and its
# target filesystem is not tmpfs. The signed-cache importer and proxy policy
# have stdlib unit coverage in tools/test_losos_sysupdate.py.
{ pkgs }:

let
  fixture = pkgs.runCommand "losos-sysupdate-fixture" {
    nativeBuildInputs = [ pkgs.coreutils pkgs.gnutar pkgs.xz ];
  } ''
    mkdir -p "$out" payload
    printf '%s\n' persistent-update-payload > payload/marker.txt
    tar -C payload -cJf "$out/losos-components_42.tar.xz" marker.txt
    (cd "$out" && sha256sum losos-components_42.tar.xz > SHA256SUMS)
  '';
in
pkgs.testers.nixosTest {
  name = "losos-sysupdate-persistent";

  nodes.machine = { lib, pkgs, ... }: {
    imports = [ ../../modules/sysupdate.nix ];

    # Use a fixed local HTTP origin in this test so the actual systemd
    # url-tar reader/extractor can be tested without depending on Internet or
    # the production proxy's HTTPS endpoint.
    systemd.sysupdate.transfers."50-losos-components".Source = lib.mkForce {
      Type = "url-tar";
      Path = "http://127.0.0.1:18991/";
      MatchPattern = "losos-components_@v.tar.xz";
    };

    systemd.services.losos-sysupdate-proxy.enable = lib.mkForce false;
    systemd.services.systemd-sysupdate = {
      wants = lib.mkForce [ ];
      after = lib.mkForce [ "network.target" ];
      # This fixture tests extraction to disk, not importing an intentionally
      # unsigned fake Nix cache. That import gate has separate unit tests.
      serviceConfig.ExecStartPost = lib.mkForce [ ];
    };
    systemd.timers.systemd-sysupdate.enable = lib.mkForce false;

    systemd.services.sysupdate-test-httpd = {
      description = "Local immutable sysupdate fixture";
      wantedBy = [ "multi-user.target" ];
      after = [ "network.target" ];
      serviceConfig = {
        Type = "simple";
        ExecStart = "${pkgs.python3}/bin/python3 -m http.server 18991 --bind 127.0.0.1 --directory ${fixture}";
        Restart = "on-failure";
      };
    };

    virtualisation.memorySize = 1024;
    virtualisation.cores = 2;
    environment.systemPackages = [ pkgs.findutils pkgs.util-linux ];
  };

  testScript = ''
    machine.start()
    machine.wait_for_unit("multi-user.target")
    machine.wait_for_open_port(18991)

    # This command parses the same SHA256SUMS manifest and uses the same
    # url-tar implementation as a real component download.
    machine.succeed("systemd-sysupdate list")
    machine.succeed("systemd-sysupdate update")
    machine.succeed("grep -Fx persistent-update-payload /var/lib/losos-sysupdate/slots/losos-components_42/marker.txt")

    # The payload is extracted beneath /var/lib, on the VM's disk-backed
    # root, not into /tmp, /run or another tmpfs.
    machine.succeed("test \"$(findmnt -n -o FSTYPE --target /var/lib/losos-sysupdate/slots)\" != tmpfs")
    machine.succeed("test ! -e /tmp/losos-components_42/marker.txt")

    machine.reboot()
    machine.wait_for_unit("multi-user.target")
    machine.succeed("grep -Fx persistent-update-payload /var/lib/losos-sysupdate/slots/losos-components_42/marker.txt")
    machine.succeed("test \"$(findmnt -n -o FSTYPE --target /var/lib/losos-sysupdate/slots)\" != tmpfs")
  '';
}
