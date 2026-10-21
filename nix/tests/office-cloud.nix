# nixos-test-vms config for Office over LosOS Cloud's files
# (modules/office.nix without `losos.office.folder`, modules/cloud.nix,
# backend/src/cloud/office.rs).
#
# LosOS Cloud runs with no network, so Office reaches it the long way round,
# and only a VM shows that the pieces meet:
#
#   * coolwsd's discovery document is saved under /run once coolwsd is up,
#     and the cloud reads the types it opens from that copy;
#   * the web app's call opens one of the person's own cloud files, and
#     nobody else's;
#   * coolwsd loads, edits and saves the document through nginx's loopback
#     bridge to the cloud's socket, and the saved .docx is in the person's
#     cloud, over WebDAV, with the earlier contents kept as a version;
#   * the bridge passes /wopi/files/ and nothing else, names nobody, and
#     wants the token; the front door routes no /wopi/ at all.
#
# The browser half (Documents, the editor bars) is nix/tests/office.nix;
# here coolwsd is driven the way its editor page drives it
# (nix/tests/office-client.py).
{ pkgs }:

let
  inherit (pkgs) lib;
  lososPkgs = import ../packages.nix { inherit pkgs; };
  python = pkgs.python3.withPackages (p: [ p.websocket-client ]);
  client = pkgs.writeShellScript "office-client" ''
    exec ${python}/bin/python3 ${./office-client.py} "$@"
  '';
  stubOcc = pkgs.writeShellScriptBin "nextcloud-occ" ''
    case "$1" in
      status) echo '{"installed":true,"maintenance":false,"needsDbUpgrade":false}' ;;
      user:resetpassword) echo "Successfully reset password for $2" ;;
      *) echo "stub occ: $*" >&2; exit 1 ;;
    esac
  '';
  plan = pkgs.writeText "plan.fodt" ''
    <?xml version="1.0" encoding="UTF-8"?>
    <office:document xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" office:version="1.3" office:mimetype="application/vnd.oasis.opendocument.text">
     <office:body>
      <office:text>
       <text:p>Tatras, 14 to 17 August.</text:p>
      </office:text>
     </office:body>
    </office:document>
  '';
in

pkgs.testers.nixosTest {
  name = "losos-office-cloud";

  nodes.mattbox = _: {
    imports = [
      ../../modules/options.nix
      ../../modules/mdns.nix
      ../../modules/containers.nix
      ../../modules/tls.nix
      ../../modules/daemon.nix
      ../../modules/identity.nix
      ../../modules/cloud.nix
      ../../modules/office.nix
    ];
    networking.hostName = "mattbox";
    losos = {
      hostName = "mattbox";
      backend.package = lososPkgs.losos-ctl;
      admin.ui = lososPkgs.losos-admin-ui;
      nextcloud.mode = "native";
      cloud.enable = true;
      forgejo.enable = false;
      office.enable = true;
    };
    systemd.services.lososd.path = lib.mkBefore [ stubOcc ];
    systemd.services.lososd.environment.LOSOS_NEXTCLOUD_MODE = lib.mkForce "native";
    environment.systemPackages = [
      pkgs.curl
      pkgs.file
      pkgs.unzip
    ];
    virtualisation = {
      memorySize = 3072;
      cores = 2;
      diskSize = 4096;
    };
  };

  testScript = ''
    import json

    client = "${client}"
    plan = "${plan}"
    SOCK = "--unix-socket /run/losos-cloud/http.sock"
    DAV = "http://box/remote.php/dav/files"

    def as_person(user):
        # What nginx says after LosOS Login, straight onto the cloud's socket.
        return f"-H 'X-Losos-Cloud-User: {user}' -H 'X-Losos-Cloud-Name: {user.title()}' -H 'Host: mattbox.local'"

    def cloud(user, args):
        return mattbox.succeed(f"curl -sf {SOCK} {as_person(user)} {args}")

    def code(cmd):
        return mattbox.succeed(f"curl -s -o /dev/null -w '%{{http_code}}' {cmd}").strip()

    def file_id(user, name):
        listing = json.loads(cloud(user, "http://box/cloud/api/files?path="))
        return next(e["id"] for e in listing["entries"] if e["name"] == name)

    def open_session(user, fid, out):
        body = json.dumps({"fileId": str(fid)})
        return json.loads(cloud(
            user,
            "-H 'Origin: https://mattbox.local' -H 'Content-Type: application/json'"
            f" -d '{body}' http://box/cloud/api/office/open | tee {out}",
        ))

    mattbox.start()
    mattbox.wait_for_unit("losos-cloud.service")
    mattbox.wait_for_unit("coolwsd.service")
    mattbox.wait_for_unit("nginx.service")

    with subtest("coolwsd's discovery is saved where the cloud reads it"):
        mattbox.wait_for_unit("losos-office-discovery.service", timeout=900)
        mattbox.succeed("grep -q 'ext=\"docx\"' /run/losos-office-discovery/discovery.xml")
        # The cloud still has no network of its own.
        mattbox.succeed("systemctl show losos-cloud -p PrivateNetwork --value | grep -qx yes")
        types = json.loads(cloud("matus", "http://box/cloud/api/office/extensions"))
        exts = {t["extension"]: t["edit"] for t in types["extensions"]}
        assert exts.get("docx") is True and exts.get("xlsx") is True, exts

    with subtest("Matus's Word file opens for Matus, and for nobody else"):
        mattbox.succeed(
            f"curl -sf -F 'data=@{plan}' http://127.0.0.1:9980/cool/convert-to/docx -o /tmp/plan.docx"
        )
        cloud("matus", f"-T /tmp/plan.docx '{DAV}/matus/Plan.docx'")
        fid = file_id("matus", "Plan.docx")
        session = open_session("matus", fid, "/tmp/matus.json")
        assert session["name"] == "Plan.docx" and session["writable"], session
        assert session["url"].startswith("/browser/"), session
        body = json.dumps({"fileId": str(fid)})
        refused = code(
            f"{SOCK} {as_person('eva')} -H 'Origin: https://mattbox.local'"
            f" -H 'Content-Type: application/json' -d '{body}' http://box/cloud/api/office/open"
        )
        assert refused == "404", refused
        nobody = code(
            f"{SOCK} -H 'Origin: https://mattbox.local' -H 'Content-Type: application/json'"
            f" -d '{body}' http://box/cloud/api/office/open"
        )
        assert nobody == "401", nobody

    with subtest("the loopback bridge passes /wopi/files/ only, with the token, naming nobody"):
        mattbox.wait_for_open_port(8093)
        wopi = f"http://127.0.0.1:8093/wopi/files/{fid}"
        token = session["accessToken"]
        info = json.loads(mattbox.succeed(f"curl -sf '{wopi}?access_token={token}'"))
        assert info["BaseFileName"] == "Plan.docx" and info["UserFriendlyName"] == "Matus", info
        assert code(f"'{wopi}'") == "401"
        assert code(f"-H 'X-Losos-Cloud-User: matus' '{wopi}/contents'") == "401"
        assert code("http://127.0.0.1:8093/cloud/api/files") == "404"
        assert code(f"-H 'X-Losos-Cloud-User: matus' http://127.0.0.1:8093/remote.php/dav/files/matus/") == "404"
        mattbox.fail("ss -ltn | grep ':8093' | grep -v 127.0.0.1")
        # The front door routes no WOPI.
        assert code(f"-k 'https://mattbox.local/wopi/files/{fid}?access_token={token}'") == "404"

    with subtest("coolwsd edits and saves the document in Matus's cloud"):
        mattbox.succeed(
            f"OFFICE_WS=ws://localhost {client} edit --session /tmp/matus.json"
            " --text 'Written in LosOS Cloud' --save"
        )
        mattbox.wait_until_succeeds(
            f"curl -sf {SOCK} {as_person('matus')} '{DAV}/matus/Plan.docx' -o /tmp/saved.docx"
            " && unzip -p /tmp/saved.docx word/document.xml | grep -q 'Written in LosOS Cloud'",
            timeout=300,
        )
        mattbox.succeed("file -b /tmp/saved.docx | grep -q 'Microsoft Word 2007+'")
        # The upload's contents stay as a version.
        versions = cloud("matus", f"-X PROPFIND -H 'Depth: 1' 'http://box/remote.php/dav/versions/matus/versions/{fid}/'")
        assert versions.count("<d:response>") >= 2, versions
  '';
}
