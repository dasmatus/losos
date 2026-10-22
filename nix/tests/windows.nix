# nixos-test-vms config for the Windows network share (modules/windows.nix,
# backend/src/windows.rs, backend/src/cloud/watch.rs): the box under
# Network on the computers around it, and each person's LosOS Cloud files
# as an SMB share opened with their LosOS name and password.
#
# There is no Windows here. The laptop stands in for one with the same
# protocols: wsdd's discovery client sends the WS-Discovery probe File
# Explorer sends, smbclient speaks SMB3 as Windows does, NTLMv2 included,
# and LLMNR answers the bare name. Then, for Finder and Linux file
# managers, Avahi and GNOME's own file system layer (gvfs) look at the
# Network view and must find the box exactly once.
#
# Also proved: the owner's password works the moment the box is claimed, a
# second person's from when they are added; each person sees and opens only
# their own share; files written over SMB show up in LosOS Cloud with a new
# ETag and the other way round; SMB1, guests, wrong passwords, the box
# itself and an old password after a change are refused; and switched off,
# the share, the discovery and the NT hashes are gone and the WebDAV entry
# is back.
{ pkgs }:

let
  inherit (pkgs) lib;
  lososPkgs = import ../packages.nix { inherit pkgs; };
  screens = import ./screens { inherit pkgs; };

  plan = name: shots: pkgs.writeText "windows-${name}.json" (builtins.toJSON {
    width = 1280;
    height = 860;
    inherit shots;
  });
  network = {
    name = "network";
    path = "/settings/network";
    steps = [
      { waitFor.text = "Windows, Mac and Linux"; }
    ];
    fullPage = true;
  };
  account = {
    name = "account";
    path = "/settings/account";
    steps = [
      { waitFor.selector = "[data-testid=account-windows-address]"; }
    ];
    fullPage = true;
  };
in

pkgs.testers.nixosTest {
  name = "losos-windows";

  nodes = {
    box = _: {
      imports = [
        ../../modules/options.nix
        ../../modules/mdns.nix
        ../../modules/containers.nix
        ../../modules/tls.nix
        ../../modules/daemon.nix
        ../../modules/identity.nix
        ../../modules/cloud.nix
        ../../modules/windows.nix
        # The option document, which draws the switch on the Network pane.
        ../../modules/config-repo.nix
      ];
      networking.hostName = "mattbox";
      losos = {
        hostName = "mattbox";
        backend.package = lososPkgs.losos-ctl;
        admin.ui = lososPkgs.losos-admin-ui;
        cloud.enable = true;
        cloud.windows.enable = true;
        forgejo.enable = false;
        configRepo.enable = false;
        mdns.enable = true;
      };
      # The same box with the share off.
      specialisation.off.configuration.losos.cloud.windows.enable = lib.mkForce false;
      environment.systemPackages = [
        pkgs.jq
        pkgs.curl
        pkgs.samba
      ];
      virtualisation = {
        memorySize = 2048;
        cores = 2;
        diskSize = 4096;
        vlans = [ 1 ];
      };
    };

    # A computer on the same LAN, with what Windows, a Mac and GNOME use to
    # look around.
    laptop = _: {
      imports = [ screens.browser ];
      services.avahi = {
        enable = true;
        nssmdns4 = true;
      };
      services.gvfs.enable = true;
      # Discovery answers come back to ports nothing asked for first.
      networking.firewall.enable = false;
      environment.systemPackages = [
        pkgs.curl
        pkgs.jq
        pkgs.avahi
        pkgs.samba
        pkgs.wsdd
        pkgs.socat
        pkgs.python3
        pkgs.glib
        pkgs.gsettings-desktop-schemas
      ];
      virtualisation = {
        memorySize = 2048;
        cores = 2;
        vlans = [ 1 ];
      };
    };
  };

  testScript = ''
    import json
    import re

    HOST = "mattbox.local"
    BASE = f"https://{HOST}"
    DAV = f"{BASE}/remote.php/webdav/"
    CA = "/etc/losos-box.pem"
    PASSWORD = "Correct-Horse-Battery-9"
    EVA = "Eva-Long-Password-3"
    EVA_NEW = "Eva-Second-Password-4"

    # One after the other: two VMs booting at once under emulation can miss
    # the driver's wait for a shell.
    box.start()
    box.wait_for_unit("nginx.service")
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("losos-cloud.service")
    box.wait_for_unit("samba-smbd.service")
    box.wait_for_unit("samba-wsdd.service")
    box.wait_for_unit("network-addresses-eth1.service")
    box_ip = box.succeed("ip -4 -o addr show eth1 | awk '{print $4}' | cut -d/ -f1").strip()
    laptop.wait_for_unit("network-addresses-eth1.service")
    laptop_ip = laptop.succeed("ip -4 -o addr show eth1 | awk '{print $4}' | cut -d/ -f1").strip()

    cert = box.succeed("cat /var/lib/losos-tls/cert.pem")
    laptop.succeed(f"cat > {CA} <<'EOF'\n{cert}EOF")

    def curl(args):
        return laptop.succeed(f"curl -s --cacert {CA} {args}")

    def code(args):
        return int(curl(f"-o /dev/null -w '%{{http_code}}' {args}"))

    def smb(args, user=None, password=None):
        auth = f"-U '{user}%{password}'" if user else "-N"
        return laptop.execute(f"smbclient {auth} {args} 2>&1")

    with subtest("the owner claims the box, and Windows takes their password at once"):
        laptop.wait_until_succeeds(f"getent hosts {HOST}", timeout=120)
        laptop.wait_until_succeeds(
            f"curl -sf --cacert {CA} {BASE}/api/setup/claim | jq -e .ready", timeout=600
        )
        out = curl(
            f"-w '\\n%{{http_code}}' -X POST {BASE}/api/setup/claim"
            " -H 'Content-Type: application/json'"
            f" -d '{json.dumps({"name": "matus", "password": PASSWORD})}'"
        )
        assert out.rsplit("\n", 1)[1] == "200", out
        box.wait_for_unit("authelia-losos.service")
        box.wait_for_open_port(9091)
        print(box.succeed("cat /var/lib/losos-windows/users.map /var/lib/losos-windows/shares.conf"))
        box.succeed("test $(stat -c %a /var/lib/losos-windows/smbpasswd) = 600")
        # Samba's Unix account, from the userdb record lososd wrote.
        print(box.succeed("getent passwd losos-win-matus"))
        # Samba looks the account up by uid too, at every sign-in.
        box.succeed("getent passwd $(id -u losos-win-matus) | grep -q ^losos-win-matus:")

    def sign_in(name, password):
        body = json.dumps({"username": name, "password": password})
        head = curl(
            f"-D - -o /dev/null {BASE}/auth/api/firstfactor "
            f"-H 'Content-Type: application/json' -d '{body}'"
        )
        m = re.search(r"(?im)^set-cookie: (losos_session=[^;]+)", head)
        assert m, f"no session cookie for {name}: {head}"
        return m.group(1)

    with subtest("a second person, added on the Users pane"):
        cookie = sign_in("matus", PASSWORD)
        person = {"name": "eva", "displayName": "Eva", "email": "eva@mattbox.local", "password": EVA, "groups": ["losos-account"]}
        out = curl(
            f"-w '\\n%{{http_code}}' -X POST {BASE}/api/people -H 'Cookie: {cookie}'"
            f" -H 'Content-Type: application/json' -d '{json.dumps(person)}'"
        )
        assert out.rsplit("\n", 1)[1] == "200", out
        me = json.loads(curl(f"-f {BASE}/api/windows -H 'Cookie: {cookie}'"))
        assert me == {"on": True, "ready": True, "share": "matus", "host": "mattbox"}, me

    with subtest("WS-Discovery: the probe File Explorer sends finds the box"):
        laptop.succeed("wsdd --discovery --no-host --listen /run/wsdd.socket >/tmp/wsdd.log 2>&1 &")
        found = laptop.wait_until_succeeds(
            "echo list | socat -t 5 - UNIX-CONNECT:/run/wsdd.socket | grep -i mattbox", timeout=120
        )
        print(found)
        assert box_ip in found, found
        # Windows turns the bare name into an address with LLMNR.
        out = laptop.wait_until_succeeds(
            "python3 -c \"import socket,struct,os;"
            "q=struct.pack('>HHHHHH',0x1234,0,1,0,0,0)+bytes([7])+b'mattbox'+bytes([0])+struct.pack('>HH',1,1);"
            "s=socket.socket(socket.AF_INET,socket.SOCK_DGRAM);s.settimeout(3);"
            # Out of the test LAN's interface, not the VM's default route.
            f"s.setsockopt(socket.IPPROTO_IP,socket.IP_MULTICAST_IF,socket.inet_aton('{laptop_ip}'));"
            "s.sendto(q,('224.0.0.252',5355));d,a=s.recvfrom(512);print(a[0],socket.inet_ntoa(d[-4:]))\"",
            timeout=60,
        )
        assert out.split() == [box_ip, box_ip], out

    with subtest("Finder and Linux file managers see the box once, as a share"):
        found = laptop.wait_until_succeeds("avahi-browse -rpt _smb._tcp | grep '^='", timeout=120)
        fields = found.splitlines()[0].split(";")
        assert fields[3] == "mattbox" and fields[8] == "445", fields
        # The WebDAV entry it replaces is gone.
        laptop.succeed("sleep 5")
        assert laptop.succeed("avahi-browse -pt _webdavs._tcp; avahi-browse -pt _webdav._tcp").strip() == ""

    with subtest("GNOME's Network view lists the box exactly once"):
        # gvfs merges what Avahi and wsdd find into network:///, the
        # Network entry of Files' sidebar.
        out = laptop.succeed(
            "timeout 300 dbus-run-session -- sh -c '"
            "timeout 60 gio list network:/// >/dev/null 2>&1; sleep 20; "
            "for d in dns-sd://local/ wsdd:/// network:///; do echo \"== $d\"; "
            "timeout 60 gio list -a standard::display-name,standard::target-uri $d; done'"
        )
        print(out)
        out = out.split("== network:///", 1)[1]
        boxes = [line for line in out.splitlines() if "mattbox" in line.lower()]
        assert len(boxes) == 1, out

    with subtest("each person sees and opens only their own share"):
        # smbd starts the helpers that list shares on the first ask, which
        # under emulation outlasts smbclient's wait.
        laptop.wait_until_succeeds(f"smbclient -U 'matus%{PASSWORD}' -L //{box_ip} 2>&1 | grep -q Disk", timeout=120)
        rc, out = smb(f"-L //{box_ip}", "matus", PASSWORD)
        assert rc == 0 and re.search(r"(?m)^\s+matus\s+Disk", out) and "eva" not in out, out
        rc, out = smb(f"-L //{box_ip}", "eva", EVA)
        assert rc == 0 and re.search(r"(?m)^\s+eva\s+Disk", out) and "matus" not in out, out
        rc, out = smb(f"//{box_ip}/matus -c ls", "eva", EVA)
        assert rc != 0, out

    def etag(propfind):
        m = re.search(r"<d:getetag>([^<]+)</d:getetag>", propfind)
        assert m, propfind
        return m.group(1)

    with subtest("SMB3, encrypted, and a file written over it lands in LosOS Cloud"):
        before = curl(f"-u 'matus:{PASSWORD}' -X PROPFIND -H 'Depth: 0' '{DAV}'")
        root_etag = etag(before)
        laptop.succeed("head -c 2000000 /dev/urandom > /tmp/report.bin")
        # Nothing below SMB3, and the client refuses to go on unencrypted.
        rc, out = smb(
            f"//{box_ip}/matus --client-protection=encrypt --option='client min protocol=SMB3'"
            " -c 'mkdir Reports; put /tmp/report.bin Reports\\report.bin'",
            "matus",
            PASSWORD,
        )
        assert rc == 0, out
        box.succeed("test $(stat -c %U /var/lib/losos-cloud/notshared/files/Reports/report.bin) = losos-cloud")
        # Picked up by LosOS Cloud: a row, a file id, and a new ETag up the chain.
        props = laptop.wait_until_succeeds(
            f"curl -sf --cacert {CA} -u 'matus:{PASSWORD}' -X PROPFIND -H 'Depth: 1' '{DAV}Reports/'"
            " | grep -q report.bin && "
            f"curl -sf --cacert {CA} -u 'matus:{PASSWORD}' -X PROPFIND -H 'Depth: 0' '{DAV}'",
            timeout=60,
        )
        assert etag(props) != root_etag, props
        laptop.succeed(
            f"curl -sf --cacert {CA} -u 'matus:{PASSWORD}' -o /tmp/report.back '{DAV}Reports/report.bin'"
            " && cmp /tmp/report.bin /tmp/report.back"
        )

    with subtest("and a file written to LosOS Cloud is there over SMB"):
        assert code(f"-u 'matus:{PASSWORD}' -X PUT --data-binary 'from the cloud' '{DAV}cloud.txt'") in (201, 204)
        rc, out = smb(f"//{box_ip}/matus -c 'get cloud.txt /tmp/cloud.txt'", "matus", PASSWORD)
        assert rc == 0, out
        assert laptop.succeed("cat /tmp/cloud.txt") == "from the cloud"
        # Deleted over SMB: gone from LosOS Cloud too.
        rc, out = smb(f"//{box_ip}/matus -c 'del cloud.txt'", "matus", PASSWORD)
        assert rc == 0, out
        laptop.wait_until_succeeds(
            f"test $(curl -s --cacert {CA} -o /dev/null -w '%{{http_code}}' -u 'matus:{PASSWORD}' '{DAV}cloud.txt') = 404",
            timeout=60,
        )

    with subtest("a second person's share is their own LosOS Cloud files"):
        rc, out = smb(f"//{box_ip}/eva -c 'put /tmp/report.bin eva.bin'", "eva", EVA)
        assert rc == 0, out
        box.succeed("test -f /var/lib/losos-cloud/eva/files/eva.bin")
        laptop.wait_until_succeeds(
            f"curl -sf --cacert {CA} -u 'eva:{EVA}' -o /tmp/eva.back '{DAV}eva.bin' && cmp /tmp/report.bin /tmp/eva.back",
            timeout=60,
        )

    with subtest("refused: SMB1, guests, a wrong password, the box itself"):
        rc, out = smb(f"-m NT1 -L //{box_ip} --option='client min protocol=NT1'", "matus", PASSWORD)
        assert rc != 0, out
        rc, out = smb(f"-L //{box_ip}")
        assert rc != 0 or "matus" not in out, out
        rc, out = smb(f"//{box_ip}/matus -c ls")
        assert rc != 0, out
        rc, out = smb(f"//{box_ip}/matus -c ls", "matus", "not-the-password")
        assert rc != 0 and "LOGON_FAILURE" in out, out
        rc, out = box.execute(f"smbclient -U 'matus%{PASSWORD}' //127.0.0.1/matus -c ls 2>&1")
        assert rc != 0, out

    with subtest("a new password replaces the old one on Windows too"):
        cookie = sign_in("matus", PASSWORD)
        out = curl(
            f"-w '\\n%{{http_code}}' -X POST {BASE}/api/people/password -H 'Cookie: {cookie}'"
            f" -H 'Content-Type: application/json' -d '{json.dumps({"name": "eva", "password": EVA_NEW})}'"
        )
        assert out.rsplit("\n", 1)[1] == "200", out
        rc, out = smb(f"//{box_ip}/eva -c ls", "eva", EVA)
        assert rc != 0, out
        rc, out = smb(f"//{box_ip}/eva -c ls", "eva", EVA_NEW)
        assert rc == 0 and "eva.bin" in out, out

    with subtest("someone whose password the box has not seen types it once"):
        # As on a box that had people before the share came on.
        box.succeed("rm /var/lib/losos-windows/learned.json && systemctl restart lososd")
        box.wait_for_unit("lososd.service")
        rc, out = smb(f"//{box_ip}/eva -c ls", "eva", EVA_NEW)
        assert rc != 0, out
        eva = sign_in("eva", EVA_NEW)
        me = json.loads(curl(f"-f {BASE}/api/windows -H 'Cookie: {eva}'"))
        assert me["ready"] is False, me
        assert code(f"-X POST {BASE}/api/windows -H 'Cookie: {eva}' -H 'Content-Type: application/json' -d '{{\"password\": \"wrong-password-1\"}}'") == 403
        me = json.loads(curl(f"-f -X POST {BASE}/api/windows -H 'Cookie: {eva}' -H 'Content-Type: application/json' -d '{json.dumps({"password": EVA_NEW})}'"))
        assert me["ready"] is True, me
        rc, out = smb(f"//{box_ip}/eva -c ls", "eva", EVA_NEW)
        assert rc == 0, out
        # The owner's comes back from the LAN WebDAV share's check.
        curl(f"-f -u 'matus:{PASSWORD}' -X PROPFIND -H 'Depth: 0' '{DAV}'")
        rc, out = smb(f"//{box_ip}/matus -c ls", "matus", PASSWORD)
        assert rc == 0, out

    with subtest("the pictures: the Network and Account panes"):
        laptop.succeed(
            f"losos-capture --base {BASE} --plan ${
              plan "after" [
                network
                account
              ]
            } --out /tmp/shots/after --user matus --password '{PASSWORD}'"
        )
        laptop.succeed(
            f"losos-capture --base {BASE} --plan ${plan "eva" [ account ]}"
            f" --out /tmp/shots/after-eva --user eva --password '{EVA_NEW}'"
        )
        laptop.copy_from_machine("/tmp/shots", "")

    with subtest("off: no share, no discovery, no NT hashes, the WebDAV entry back"):
        box.succeed("/run/current-system/specialisation/off/bin/switch-to-configuration test")
        box.wait_until_fails("systemctl is-active samba-smbd.service")
        box.fail("systemctl is-active samba-wsdd.service")
        box.succeed("test ! -e /var/lib/losos-windows")
        box.succeed("! ls /run/userdb/losos-win-* 2>/dev/null")
        rc, out = smb(f"//{box_ip}/matus -c ls", "matus", PASSWORD)
        assert rc != 0, out
        laptop.wait_until_succeeds("avahi-browse -rpt _webdavs._tcp | grep '^='", timeout=120)
        me = json.loads(curl(f"-f {BASE}/api/windows -H 'Cookie: {sign_in("matus", PASSWORD)}'"))
        assert me == {"on": False}, me
  '';
}
