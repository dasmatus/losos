# nixos-test-vms config for LosOS Cloud's files from a computer on the LAN:
# WebDAV, the sync clients' sign-in, and the entry a file manager's Network
# view shows.
#
# The box runs LosOS Cloud as it ships: the pod's own entrypoint, Apache
# and php-fpm configuration and mounted config files (nix/images.nix,
# modules/workloads.nix), with the Nextcloud package and apps a box serves,
# behind the real front vhost (modules/containers.nix, modules/tls.nix) and
# LosOS Login (modules/identity.nix), with lososd claiming the box. What is
# not real is the kubelet: nix/tests/kubelet-stand-in.py starts the pod from
# its manifest as a systemd unit, with the manifest's mounts, uid and
# dropped capabilities, and a stand-in `crictl` on lososd's path execs into
# it the same way. k3s and the 2.6 GiB image would test k3s
# (nix/tests/cluster-vm.nix does that, with a probe image).
#
# The laptop is a Linux desktop as far as the network sees it: Avahi
# browses the LAN the way GNOME Files and KDE Dolphin do, it resolves
# mattbox.local over mDNS, and it trusts the box's certificate as the
# first-run page tells an owner to. It then does what a person's computer
# does:
#
#   * finds "LosOS Cloud on mattbox" among the LAN's WebDAV shares and
#     follows it to the share;
#   * lists, uploads, downloads and compares, makes a folder, moves,
#     copies, deletes, and sends a large file in chunks as the desktop
#     client does;
#   * opens the share with LosOS Login names and passwords, the owner's
#     and that of a person who never opened LosOS Cloud before, which
#     lososd checks for nginx (backend/src/cloud_share.rs);
#   * gets an app password through Login Flow v2 (the desktop client) and
#     v1 (the phone apps), meeting LosOS Login on the way, for the owner and
#     for a second person;
#   * syncs a folder both ways with nextcloudcmd, the desktop client's own
#     sync engine.
#
# Slow without KVM: Nextcloud's first start under TCG takes a while.
{ pkgs }:

let
  inherit (pkgs) lib;
  lososPkgs = import ../packages.nix { inherit pkgs; };
  images = import ../images.nix { inherit pkgs; };
  nc = import ../../modules/nextcloud-stack.nix { inherit pkgs lib; };

  kubelet = pkgs.writeScriptBin "kubelet-stand-in" ''
    #!${pkgs.python3.withPackages (ps: [ ps.pyyaml ])}/bin/python3
    ${builtins.readFile ./kubelet-stand-in.py}
  '';
  kubeletEnv = {
    IMAGE_ROOT = "${images.losos-image-nextcloud.root}";
    IMAGE_WORKDIR = nc.home;
  };

  # crictl as lososd calls it (backend/src/setup.rs): find the running
  # Nextcloud container, exec into it, read its last words.
  crictl = pkgs.writeShellScriptBin "crictl" ''
    set -eu
    while [ $# -gt 0 ]; do
      case $1 in
        --runtime-endpoint | -r) shift 2 ;;
        *) break ;;
      esac
    done
    verb=$1
    shift
    case $verb in
      ps)
        case " $* " in
          *" --all "*) echo pod-nextcloud ;;
          *) if systemctl is-active -q pod-nextcloud; then echo pod-nextcloud; fi ;;
        esac
        ;;
      exec)
        [ "$1" = --sync ] && shift
        [ "$1" = pod-nextcloud ] || { echo "no container $1" >&2; exit 1; }
        shift
        ${lib.concatStrings (lib.mapAttrsToList (k: v: "export ${k}=${v}\n") kubeletEnv)}
        exec ${kubelet}/bin/kubelet-stand-in exec "$@"
        ;;
      logs) journalctl -u pod-nextcloud -n 20 -o cat ;;
      *) echo "stand-in crictl: $verb" >&2; exit 1 ;;
    esac
  '';

  client = pkgs.writeScriptBin "cloud-client" ''
    #!${pkgs.python3.withPackages (ps: [ ps.requests ])}/bin/python3
    ${builtins.readFile ./cloud-client.py}
  '';

  # The admin pages themselves are not under test; one file stands in.
  stubUi = pkgs.runCommand "stub-admin-ui" { } ''
    mkdir -p $out
    echo "admin ui" > $out/index.html
  '';
in

pkgs.testers.nixosTest {
  name = "losos-cloud-webdav";

  nodes = {
    box = _: {
      imports = [
        ../../modules/options.nix
        ../../modules/mdns.nix
        ../../modules/services.nix
        ../../modules/nextcloud-common.nix
        ../../modules/containers.nix
        ../../modules/tls.nix
        ../../modules/daemon.nix
        ../../modules/identity.nix
        ../../modules/workloads.nix
      ];
      networking.hostName = "mattbox";
      losos = {
        hostName = "mattbox";
        backend.package = lososPkgs.losos-ctl;
        admin.ui = stubUi;
        nextcloud.mode = "container";
        forgejo.enable = false;
        # What modules/configuration.nix gives the real box.
        mdns.enable = true;
        # The manifest names the image; the stand-in kubelet runs its root.
        workloads.nextcloudImage = pkgs.hello // {
          imageName = "losos.local/nextcloud";
          imageTag = "test";
        };
      };
      # The uids and gids modules/configuration.nix pins.
      users.groups.nextcloud.gid = 1002;
      users.users.nextcloud = {
        uid = 1002;
        group = "nextcloud";
        isSystemUser = true;
        extraGroups = [ "redis-nextcloud" ];
      };
      users.groups."redis-nextcloud".gid = 1004;

      # Where k3s would start the pod: after what modules/workloads.nix
      # orders k3s after.
      systemd.services.losos-test-kubelet = {
        wantedBy = [ "multi-user.target" ];
        after = [
          "postgresql.service"
          "postgresql-setup.service"
          "redis-nextcloud.service"
          "losos-nextcloud-adminpass.service"
          "systemd-tmpfiles-setup.service"
        ];
        requires = [ "losos-nextcloud-adminpass.service" ];
        wants = [
          "postgresql.service"
          "postgresql-setup.service"
          "redis-nextcloud.service"
        ];
        environment = kubeletEnv;
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
          ExecStart = "${kubelet}/bin/kubelet-stand-in run";
        };
      };
      systemd.services.lososd.path = lib.mkBefore [ crictl ];

      environment.systemPackages = [
        pkgs.jq
        pkgs.curl
      ];
      systemd.services.postgresql.serviceConfig.TimeoutStartSec = "15min";
      virtualisation = {
        memorySize = 3072;
        cores = 2;
        diskSize = 4096;
        vlans = [ 1 ];
      };
    };

    laptop = _: {
      services.avahi = {
        enable = true;
        nssmdns4 = true;
      };
      environment.systemPackages = [
        pkgs.curl
        pkgs.jq
        pkgs.avahi
        pkgs.nextcloud-client
        client
      ];
      environment.variables.SSL_CERT_FILE = "/etc/losos-box.pem";
      virtualisation = {
        memorySize = 1536;
        vlans = [ 1 ];
      };
    };
  };

  testScript = ''
    import json
    import re

    HOST = "mattbox.local"
    BASE = f"https://{HOST}"
    CA = "/etc/losos-box.pem"
    PASSWORD = "Correct-Horse-Battery-9"
    EVA = "Eva-Long-Password-3"

    start_all()
    box.wait_for_unit("nginx.service")
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("losos-test-kubelet.service")

    with subtest("LosOS Cloud comes up in its pod"):
        # The entrypoint installs, upgrades and enables the apps before it
        # starts Apache, so status.php answering means all of that is done.
        box.wait_until_succeeds(
            "curl -sf http://127.0.0.1/nextcloud/status.php | grep -q '\"installed\":true'",
            timeout=3600,
        )
        print(box.succeed("journalctl -u pod-nextcloud -o cat | tail -n 30"))

    # The owner's computer trusts the box's certificate (first run, step 1).
    cert = box.succeed("cat /var/lib/losos-tls/cert.pem")
    laptop.succeed(f"cat > {CA} <<'EOF'\n{cert}EOF")

    def curl(args):
        return laptop.succeed(f"curl -s --cacert {CA} {args}")

    def code(args):
        return int(curl(f"-o /dev/null -w '%{{http_code}}' {args}"))

    with subtest("the laptop finds the box by name"):
        laptop.wait_until_succeeds(f"getent hosts {HOST}", timeout=120)

    with subtest("the owner claims the box"):
        laptop.wait_until_succeeds(
            f"curl -sf --cacert {CA} {BASE}/api/setup/claim | jq -e .ready", timeout=600
        )
        out = curl(
            f"-w '\\n%{{http_code}}' -X POST {BASE}/api/setup/claim"
            " -H 'Content-Type: application/json'"
            f" -d '{json.dumps({"name": "matus", "password": PASSWORD})}'"
        )
        body, status = out.rsplit("\n", 1)
        assert status == "200", out
        box.wait_for_unit("authelia-losos.service")
        box.wait_for_open_port(9091)

    with subtest("a second person, as the identity test adds one"):
        box.succeed(
            "${pkgs.writeShellScript "add-eva" ''
              set -eu
              hash=$(${pkgs.authelia}/bin/authelia crypto hash generate argon2 \
                --password 'Eva-Long-Password-3' --no-confirm | sed 's/^Digest: //')
              ${pkgs.jq}/bin/jq --arg h "$hash" \
                '.users.eva = {disabled: false, displayname: "Eva", email: "eva@mattbox.local",
                  groups: ["losos-viewers"], password: $h}' \
                /var/lib/losos-identity/users.yml > /tmp/users.yml
              install -m 0640 -g authelia-losos /tmp/users.yml /var/lib/losos-identity/.users.tmp
              mv /var/lib/losos-identity/.users.tmp /var/lib/losos-identity/users.yml
            ''}"
        )
        box.wait_until_succeeds(
            "journalctl -u authelia-losos | grep -c 'Reloaded successfully' | grep -qv '^0$'",
            timeout=60,
        )

    with subtest("the Network view lists LosOS Cloud"):
        found = laptop.wait_until_succeeds(
            "avahi-browse -rpt _webdavs._tcp | grep '^='", timeout=120
        )
        print(found)
        fields = found.splitlines()[0].split(";")
        name, host, port, txt = fields[3], fields[6], fields[8], fields[9]
        assert name == r"LosOS\032Cloud\032on\032mattbox", name
        assert host == HOST and port == "443", fields
        m = re.search(r'"path=([^"]+)"', txt)
        assert m, txt
        DAV = f"https://{host}:{port}{m.group(1)}"
        assert DAV == f"{BASE}:443/nextcloud/remote.php/webdav/", DAV
        # No password in the clear: nothing is announced on plain http.
        assert "LosOS" not in laptop.succeed("avahi-browse -rpt _webdav._tcp")

    def dav(user, password, method, path, extra=""):
        return curl(
            f"-u '{user}:{password}' -X {method} -w '\\n%{{http_code}}' {extra}"
            f" '{DAV}{path}'"
        ).rsplit("\n", 1)

    def files_round_trip(user, password, tag):
        """What a file manager does with the share."""
        body, status = dav(user, password, "PROPFIND", "", "-H 'Depth: 1'")
        assert status == "207", (status, body[:400])
        laptop.succeed(f"head -c 3000000 /dev/urandom > /tmp/{tag}.bin")
        _, status = dav(user, password, "PUT", f"{tag}.bin", f"-T /tmp/{tag}.bin")
        assert status in ("201", "204"), status
        laptop.succeed(
            f"curl -sf --cacert {CA} -u '{user}:{password}' -o /tmp/{tag}.back '{DAV}{tag}.bin'"
            f" && cmp /tmp/{tag}.bin /tmp/{tag}.back"
        )
        _, status = dav(user, password, "MKCOL", f"{tag}-dir")
        assert status == "201", status
        # Destination is an absolute URL on the address the client used.
        _, status = dav(
            user, password, "MOVE", f"{tag}.bin", f"-H 'Destination: {DAV}{tag}-dir/moved.bin'"
        )
        assert status == "201", status
        _, status = dav(
            user, password, "COPY", f"{tag}-dir/moved.bin",
            f"-H 'Destination: {DAV}{tag}-copy.bin'",
        )
        assert status == "201", status
        body, status = dav(user, password, "PROPFIND", f"{tag}-dir/", "-H 'Depth: 1'")
        assert status == "207" and "moved.bin" in body, (status, body[:400])
        _, status = dav(user, password, "DELETE", f"{tag}-copy.bin")
        assert status == "204", status
        _, status = dav(user, password, "GET", f"{tag}-copy.bin", "-o /dev/null")
        assert status == "404", status

    with subtest("the owner's share opens with the owner's password"):
        # LosOS Cloud's name for the owner, the one their files are under.
        files_round_trip("notshared", PASSWORD, "owner")

    with subtest("no password, a wrong one, or a made-up header is refused"):
        assert code(f"-X PROPFIND '{DAV}'") == 401
        assert code(f"-u notshared:not-the-password -X PROPFIND '{DAV}'") == 401
        forged = "-H 'X-Losos-Cloud-User: notshared' -H 'Remote-User: matus'"
        assert code(f"{forged} -X PROPFIND '{DAV}'") == 401

    with subtest("a LosOS Login name and password open the share too"):
        # What someone types into the file manager's prompt after clicking
        # the entry: the name and password they sign in with everywhere
        # else. The owner's files are the same ones as under notshared.
        files_round_trip("matus", PASSWORD, "owner-name")
        dav("matus", PASSWORD, "PUT", "by-name.txt", "--data-binary 'typed as matus'")
        assert curl(f"-f -u 'notshared:{PASSWORD}' '{DAV}by-name.txt'") == "typed as matus"
        # Eva has never opened LosOS Cloud: the share makes her account.
        files_round_trip("eva", EVA, "eva-name")
        assert code(f"-u 'eva:{EVA}' -X PROPFIND '{BASE}/nextcloud/remote.php/dav/files/notshared/'") in (401, 403, 404)
        # A wrong LosOS password is LosOS Cloud's 401, as before.
        assert code(f"-u 'eva:not-her-password' -X PROPFIND '{DAV}'") == 401
        assert code(f"-u 'matus:not-his-password' -X PROPFIND '{DAV}'") == 401
        # lososd checked each password once; the rest were remembered.
        print(box.succeed("journalctl -u lososd -o cat | tail -n 20"))

    with subtest("a large file goes up in chunks, as the desktop client sends it"):
        up = f"{BASE}/nextcloud/remote.php/dav/uploads/notshared/web-test-1"
        dest = f"{BASE}/nextcloud/remote.php/dav/files/notshared/big.bin"
        auth = f"-u 'notshared:{PASSWORD}'"
        laptop.succeed("head -c 25000000 /dev/urandom > /tmp/big.bin && split -b 10000000 -d -a 5 /tmp/big.bin /tmp/big.part.")
        assert code(f"{auth} -X MKCOL -H 'Destination: {dest}' '{up}'") == 201
        parts = sorted(laptop.succeed("ls /tmp/big.part.*").split())
        for i, part in enumerate(parts, 1):
            assert code(
                f"{auth} -X PUT -H 'Destination: {dest}' -H 'OC-Total-Length: 25000000'"
                f" -T {part} '{up}/{i:05d}'"
            ) == 201, part
        assert code(
            f"{auth} -X MOVE -H 'Destination: {dest}' -H 'OC-Total-Length: 25000000' '{up}/.file'"
        ) in (201, 204)
        laptop.succeed(
            f"curl -sf --cacert {CA} {auth} -o /tmp/big.back '{dest}' && cmp /tmp/big.bin /tmp/big.back"
        )

    def flow(kind, name, password):
        out = json.loads(laptop.succeed(f"cloud-client {kind} {BASE} {name} '{password}'"))
        print(json.dumps(out, indent=1))
        return out

    with subtest("the desktop client signs the owner in through LosOS Login"):
        got = flow("flow2", "matus", PASSWORD)
        # Addresses the client keeps: the box's name, https, the prefix.
        assert got["init"]["login"].startswith(f"{BASE}/nextcloud/"), got
        assert got["init"]["poll"]["endpoint"] == f"{BASE}/nextcloud/login/v2/poll", got
        assert got["server"] == f"{BASE}/nextcloud", got
        assert got["loginName"] == "notshared" and got["grantUser"] == "notshared", got
        assert any(u.startswith(f"{BASE}/auth/") for u in got["seen"]), got["seen"]
        owner_app = got["appPassword"]
        files_round_trip("notshared", owner_app, "owner-app")

    with subtest("the phone app signs a second person in"):
        got = flow("flow1", "eva", EVA)
        assert got["server"] == f"{BASE}/nextcloud", got
        assert got["loginName"] == "eva" and got["grantUser"] == "eva", got
        eva_app = got["appPassword"]
        files_round_trip("eva", eva_app, "eva")
        # Each person's share is their own.
        assert code(f"-u 'eva:{eva_app}' -X PROPFIND '{BASE}/nextcloud/remote.php/dav/files/notshared/'") in (401, 403, 404)

    with subtest("the desktop client's sync engine syncs a folder both ways"):
        laptop.succeed("mkdir -p /tmp/sync && echo 'from the laptop' > /tmp/sync/laptop.txt")
        sync = (
            f"nextcloudcmd --non-interactive -u notshared -p '{owner_app}'"
            f" /tmp/sync {BASE}/nextcloud"
        )
        laptop.succeed(sync)
        assert curl(f"-f -u 'notshared:{owner_app}' '{DAV}laptop.txt'") == "from the laptop\n"
        # And back: the big file uploaded above lands in the folder.
        laptop.succeed("cmp /tmp/sync/big.bin /tmp/big.bin")
        dav("notshared", owner_app, "PUT", "from-box.txt", "--data-binary 'from the box'")
        laptop.succeed(sync)
        assert laptop.succeed("cat /tmp/sync/from-box.txt") == "from the box"

  '';
}
