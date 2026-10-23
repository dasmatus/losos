# QEMU integration test for persistent systemd-sysupdate downloads and extraction.
#
# The guest's virtual-disk root is the target filesystem. A local fixture server
# avoids network/release dependencies while exercising the real systemd-sysupdate
# url-file download/checksum path and LosOS's safe extractor after reboot.
{ pkgs }:

let
  fixture = pkgs.runCommand "losos-sysupdate-fixture" {
    nativeBuildInputs = [ pkgs.coreutils pkgs.gnutar pkgs.xz ];
  } ''
    mkdir -p "$out" payload/cache
    printf '%s\n' persistent-update-payload > payload/cache/marker.txt
    printf 'StoreDir: /nix/store\nWantMassQuery: 1\nPriority: 50\n' > payload/cache/nix-cache-info
    printf 'StorePath: /nix/store/%s-losOS-test\n' "$(printf 'a%.0s' $(seq 1 32))" > "payload/cache/$(printf 'a%.0s' $(seq 1 32)).narinfo"
    printf '%s\n' '{"format":1,"version":42,"sourceVersion":"v-test","commit":"0123456789abcdef","storePaths":[]}' > payload/manifest.json
    tar --sort=name --mtime='UTC 1970-01-01' --owner=0 --group=0 --numeric-owner \
      -C payload -cf - manifest.json cache | xz -T0 -6 > "$out/losos-components_42.tar.xz"
    (cd "$out" && sha256sum losos-components_42.tar.xz > SHA256SUMS)
  '';
in
pkgs.testers.nixosTest {
  name = "losos-sysupdate-persistent";

  nodes.machine = { lib, pkgs, ... }: {
    imports = [ ../../modules/sysupdate.nix ];

    # Replace only the source URL; the production url-file source and
    # regular-file target remain the same and write into persistent /var.
    systemd.sysupdate.transfers."50-losos-components".Source = lib.mkForce {
      Type = "url-file";
      Path = "http://127.0.0.1:18991/";
      MatchPattern = "losos-components_@v.tar.xz";
    };

    systemd.services.losos-sysupdate-proxy.enable = lib.mkForce false;
    systemd.services.systemd-sysupdate-update.serviceConfig.ExecStartPost = lib.mkForce [ ];
    systemd.timers.systemd-sysupdate-update.enable = lib.mkForce false;

    # The test exercises extraction separately from importing the intentionally
    # unsigned dummy cache, which cannot satisfy production Nix signatures.
    environment.etc."losos-sysupdate-apply.py".source = ../../tools/losos-sysupdate-apply.py;

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

    machine.succeed("systemd-sysupdate list")
    machine.succeed("systemd-sysupdate update")
    machine.succeed("test -s /var/lib/losos-sysupdate/archives/losos-components_42.tar.xz")
    machine.succeed("python3 /etc/losos-sysupdate-apply.py --extract-only")
    machine.succeed("grep -Fx persistent-update-payload /var/lib/losos-sysupdate/slots/losos-components_42/cache/marker.txt")

    # Both the downloaded archive and extracted bundle must live on the VM's
    # disk-backed filesystem, never on /tmp, /run, or another tmpfs.
    machine.succeed("test \"$(findmnt -n -o FSTYPE --target /var/lib/losos-sysupdate/archives)\" != tmpfs")
    machine.succeed("test \"$(findmnt -n -o FSTYPE --target /var/lib/losos-sysupdate/slots)\" != tmpfs")
    machine.succeed("test ! -e /tmp/losos-components_42.tar.xz")

    machine.reboot()
    machine.wait_for_unit("multi-user.target")
    machine.succeed("grep -Fx persistent-update-payload /var/lib/losos-sysupdate/slots/losos-components_42/cache/marker.txt")
    machine.succeed("test \"$(findmnt -n -o FSTYPE --target /var/lib/losos-sysupdate/slots)\" != tmpfs")
  '';
}
