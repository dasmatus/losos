# nixos-test-vms config for how fast a box takes files in: LosOS Cloud
# (backend/src/cloud/dav.rs) and LosOS Git (backend/src/forge/transport.rs)
# on the smallest box LosOS is made for, two cores and 4 GiB, behind nginx
# with the same streaming settings as the front vhost (modules/cloud.nix,
# modules/containers.nix).
#
# nix/tests/upload-speed.py runs on the box and times
#
#   * one large file over WebDAV;
#   * 200 photo-sized files, four at a time;
#   * the same large file in 10 MiB chunks and the MOVE that joins them,
#     as the desktop client sends it;
#   * a git push of a repository with a large file in it;
#
# checks that every byte arrived, and keeps the numbers and each service's
# peak memory in $out/upload-speed.json. Nothing here is a threshold: run it
# on two trees to compare them.
{ pkgs }:

let
  lososPkgs = import ../packages.nix { inherit pkgs; };
  backend = lososPkgs.losos-ctl;
  git = "${pkgs.gitMinimal}/bin/git";
  measure = pkgs.writeScriptBin "upload-speed" ''
    #!${pkgs.python3}/bin/python3
    ${builtins.readFile ./upload-speed.py}
  '';
  # What the front vhost puts on every upload: streamed both ways, no size
  # cap, and the person LosOS Login named.
  streamed = ''
    client_max_body_size 0;
    proxy_request_buffering off;
    proxy_buffering off;
    proxy_http_version 1.1;
    proxy_read_timeout 1h;
    proxy_send_timeout 1h;
    proxy_set_header X-Losos-Cloud-User perf;
  '';
in

pkgs.testers.nixosTest {
  name = "losos-upload-speed";

  nodes.box = _: {
    virtualisation = {
      cores = 2;
      memorySize = 4096;
      diskSize = 8192;
    };
    environment.systemPackages = [
      measure
      pkgs.curl
      pkgs.gitMinimal
    ];

    users.groups.perf = { };
    users.users.perf = {
      isSystemUser = true;
      group = "perf";
    };
    users.users.nginx.extraGroups = [ "perf" ];

    systemd.services.perf-cloud = {
      wantedBy = [ "multi-user.target" ];
      serviceConfig = {
        User = "perf";
        Group = "perf";
        StateDirectory = "perf-cloud";
        RuntimeDirectory = "perf-cloud";
        RuntimeDirectoryMode = "0750";
        UMask = "0007";
        ExecStart = "${backend}/bin/losos-cloud --root /var/lib/perf-cloud serve --socket /run/perf-cloud/http.sock";
      };
    };
    systemd.services.perf-git = {
      wantedBy = [ "multi-user.target" ];
      path = [ pkgs.gitMinimal ];
      serviceConfig = {
        User = "perf";
        Group = "perf";
        StateDirectory = "perf-git";
        RuntimeDirectory = "perf-git";
        RuntimeDirectoryMode = "0750";
        UMask = "0007";
        ExecStart = "${backend}/bin/losos-git serve --state-dir /var/lib/perf-git --socket /run/perf-git/http.sock --git ${git} --push-create";
      };
    };

    services.nginx = {
      enable = true;
      virtualHosts.box = {
        default = true;
        locations."/remote.php/" = {
          proxyPass = "http://unix:/run/perf-cloud/http.sock";
          extraConfig = streamed;
        };
        locations."~ ^/git/[^/]+/[^/]+/(?:info/refs|git-upload-pack|git-receive-pack)$" = {
          proxyPass = "http://unix:/run/perf-git/http.sock";
          extraConfig = ''
            rewrite ^/git(/.*)$ $1 break;
            ${streamed}
          '';
        };
      };
    };
  };

  testScript = ''
    import json

    box.wait_for_unit("perf-cloud.service")
    box.wait_for_unit("perf-git.service")
    box.wait_for_unit("nginx.service")
    box.wait_until_succeeds("test -S /run/perf-cloud/http.sock")
    box.wait_until_succeeds("test -S /run/perf-git/http.sock")
    # The first request makes the person's home.
    box.wait_until_succeeds(
        "curl -sf -X PROPFIND -H 'Depth: 0' http://127.0.0.1/remote.php/dav/files/perf/ -o /dev/null"
    )

    box.succeed("upload-speed /tmp/upload-speed.json >&2")
    result = json.loads(box.succeed("cat /tmp/upload-speed.json"))
    box.copy_from_vm("/tmp/upload-speed.json", "")
    for name in ("large", "small", "chunkedSend", "chunkedJoin", "push"):
        print(f"{name}: {result[name]['median']} s, {result[name]['mibPerSecond']} MiB/s")
    print(f"peak memory: {result['peakMiB']}")
  '';
}
