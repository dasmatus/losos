# nixos-test-vms config for managing LosOS Cloud's apps from the admin UI's
# Apps pane, the owner's account being a standard one, and a federated share
# between two boxes.
#
# lososd's side (which app may change, the refusals, the store search) is
# unit-tested in backend/src/cloud_apps.rs. What a unit test cannot reach is
# the tool lososd runs, `losos-nextcloud-apps` (modules/nextcloud-stack.nix,
# `appsTool`), against a real Nextcloud: that its list is what Nextcloud has,
# that an app turned off stays off when nextcloud-setup runs again (every
# start turns the bundled apps on, and the off-list has to win), and that
# only an app from the store can be removed. Then what Matus asked for
# alongside it: the owner's account (`notshared`) is not in Nextcloud's admin
# group after a start, and two boxes that both share their disk send each
# other a file with Federated Cloud Sharing.
#
# Two VMs, each the real native stack (modules/services.nix with
# losos.nextcloud.mode = "native", the same module set and occ lines the pod
# runs), with disk sharing on so federation is open. They reach each other
# by bare names on a private network, over https with certificates they
# made themselves: the case Nextcloud refuses unless the box allows local
# servers and self-signed certificates (modules/nextcloud-stack.nix,
# `federation.system`), which is what LAN boxes look like.
{ pkgs }:

let
  inherit (pkgs) lib;
  nc = import ../../modules/nextcloud-stack.nix { inherit pkgs lib; };

  # A stand-in for an app added from the app store: the VMs have no
  # internet, so it is copied into store-apps the way app:install leaves
  # one. An info.xml is all Nextcloud needs to list and enable an app.
  helloApp = pkgs.writeTextDir "hello/appinfo/info.xml" ''
    <?xml version="1.0"?>
    <info>
      <id>hello</id>
      <name>Hello</name>
      <summary>Says hello</summary>
      <description>A stand-in for an app from the store.</description>
      <version>1.0.0</version>
      <licence>agpl</licence>
      <author>LosOS</author>
      <namespace>Hello</namespace>
      <category>tools</category>
      <dependencies>
        <nextcloud min-version="30" max-version="40"/>
      </dependencies>
    </info>
  '';

  # Each box's own self-signed certificate, the way modules/tls.nix makes
  # one on first boot: nothing on the other box trusts it.
  cert =
    name:
    pkgs.runCommand "losos-test-cert-${name}" { nativeBuildInputs = [ pkgs.openssl ]; } ''
      mkdir $out
      openssl req -x509 -newkey rsa:2048 -nodes -days 365 -subj /CN=${name} \
        -addext subjectAltName=DNS:${name} -keyout $out/key.pem -out $out/cert.pem
    '';

  box =
    name:
    { config, ... }:
    {
      imports = [
        ../../modules/options.nix
        ../../modules/services.nix
        ../../modules/nextcloud-common.nix
      ];

      losos = {
        hostName = name;
        nextcloud.mode = "native";
        nextcloud.hostName = name;
        # Federation runs only while the box shares its disk.
        sharingMyStorage = true;
      };

      # Two of the curated apps download from hosts a build sandbox may not
      # reach; neither has anything to do with what is tested here.
      services.nextcloud.extraApps = lib.mkForce (
        removeAttrs nc.apps [
          "files_linkeditor"
          "memories"
        ]
      );

      services.nginx.virtualHosts.${name} = {
        addSSL = true;
        sslCertificate = "${cert name}/cert.pem";
        sslCertificateKey = "${cert name}/key.pem";
      };
      networking.firewall.allowedTCPPorts = [
        80
        443
      ];

      environment.systemPackages = [
        pkgs.curl
        pkgs.jq
        # The tool lososd runs natively (modules/daemon.nix), built the same way.
        (nc.appsTool {
          occ = "${config.services.nextcloud.occ}/bin/nextcloud-occ";
          webroot = config.services.nextcloud.finalPackage;
        })
      ];
      systemd.services.postgresql.serviceConfig.TimeoutStartSec = "15min";
      systemd.services.nextcloud-setup.serviceConfig.TimeoutStartSec = "60min";
      virtualisation = {
        memorySize = 3072;
        cores = 2;
        diskSize = 4096;
      };
    };
in

pkgs.testers.nixosTest {
  name = "losos-cloud-apps";

  nodes = {
    mattbox = box "mattbox";
    evabox = box "evabox";
  };

  testScript =
    { nodes, ... }:
    ''
      import json

      def ready(m):
          # multi-user.target waits for nextcloud-setup, which installs and
          # enables some thirty apps: over ten minutes without KVM.
          m.wait_for_unit("multi-user.target", timeout=3600)
          m.wait_until_succeeds(
              "systemctl show -p Result --value nextcloud-setup | grep -qx success", timeout=3600
          )
          m.wait_for_unit("phpfpm-nextcloud.service")
          m.wait_for_open_port(80)

      def apps(m):
          return {a["id"]: a for a in json.loads(m.succeed("losos-nextcloud-apps list"))}

      def enabled(m, app):
          state = json.loads(m.succeed("nextcloud-occ app:list --output=json"))
          return app in state["enabled"]

      def password(m):
          return m.succeed("cat ${toString nodes.mattbox.losos.nextcloud.adminpassFile}").strip()

      start_all()
      ready(mattbox)
      ready(evabox)

      with subtest("the owner's account is a standard one"):
          for m in (mattbox, evabox):
              groups = json.loads(m.succeed("nextcloud-occ group:list --output=json"))
              assert "notshared" not in groups.get("admin", []), groups
              info = json.loads(m.succeed("nextcloud-occ user:info notshared --output=json"))
              assert "admin" not in info["groups"], info
              assert info["enabled"], info

      with subtest("the list is every app Nextcloud has, with its name and whether it may change"):
          listed = apps(mattbox)
          assert listed["calendar"]["kind"] == "bundled" and listed["calendar"]["enabled"], listed["calendar"]
          assert listed["calendar"]["name"] == "Calendar", listed["calendar"]
          assert listed["files"]["kind"] == "core" and listed["files"]["alwaysEnabled"], listed["files"]
          assert listed["activity"]["kind"] == "core", listed["activity"]
          assert listed["federation"]["enabled"], "federation is off on a box that shares its disk"

      with subtest("an app turned off stays off when nextcloud-setup runs again"):
          mattbox.succeed("losos-nextcloud-apps disable calendar")
          assert not enabled(mattbox, "calendar")
          assert mattbox.succeed("cat /var/lib/nextcloud/.losos-apps-off").split() == ["calendar"]
          mattbox.succeed("systemctl restart nextcloud-setup.service", timeout=3600)
          mattbox.wait_until_succeeds("systemctl show -p Result --value nextcloud-setup | grep -qx success")
          assert not enabled(mattbox, "calendar"), "a restart turned calendar back on"
          # The rest of the curated apps came back on as usual.
          assert enabled(mattbox, "contacts")

      with subtest("turned on again, it leaves the off-list"):
          mattbox.succeed("losos-nextcloud-apps enable calendar")
          assert enabled(mattbox, "calendar")
          assert mattbox.succeed("cat /var/lib/nextcloud/.losos-apps-off").strip() == ""
          mattbox.succeed("systemctl restart nextcloud-setup.service", timeout=3600)
          mattbox.wait_until_succeeds("systemctl show -p Result --value nextcloud-setup | grep -qx success")
          assert enabled(mattbox, "calendar")

      with subtest("only an app from the store can be removed"):
          code = mattbox.execute("losos-nextcloud-apps remove calendar")[0]
          assert code == 3, f"removing a bundled app exited {code}"
          assert enabled(mattbox, "calendar")
          mattbox.fail("losos-nextcloud-apps disable 'calendar;id'")
          mattbox.succeed(
              "cp -r ${helloApp}/hello /var/lib/nextcloud/store-apps/",
              "chown -R nextcloud:nextcloud /var/lib/nextcloud/store-apps/hello",
              "chmod -R u+w /var/lib/nextcloud/store-apps/hello",
          )
          mattbox.succeed("losos-nextcloud-apps enable hello")
          listed = apps(mattbox)
          assert listed["hello"]["kind"] == "store" and listed["hello"]["enabled"], listed["hello"]
          assert listed["hello"]["name"] == "Hello", listed["hello"]
          mattbox.succeed("losos-nextcloud-apps remove hello")
          mattbox.fail("test -e /var/lib/nextcloud/store-apps/hello")
          assert "hello" not in apps(mattbox)

      with subtest("a federated share goes from one box to the other over https"):
          # Both boxes answer https with a certificate the other does not
          # trust, and the address names no protocol, so Nextcloud uses https.
          a = password(mattbox)
          b = password(evabox)
          # Each box can find the other's OCM endpoint, through its
          # self-signed certificate. Nextcloud gives up on that call after
          # ten seconds, and a box's first PHP requests take longer than
          # that without KVM, so wait until both answer quickly. The first
          # answer makes the box's signing key, a public and a private file
          # written one after the other: one unhurried request per box first,
          # or the retries below race and leave halves of two different keys.
          for m in (mattbox, evabox):
              m.succeed(f"curl -sfkL https://{m.name}/.well-known/ocm | jq -e '.publicKey'", timeout=900)
          for m, other in ((mattbox, "evabox"), (evabox, "mattbox")):
              for path in ("/.well-known/ocm", "/ocm-provider/"):
                  m.wait_until_succeeds(
                      f"curl -sfkL --max-time 5 https://{other}{path} | jq -e '.enabled'",
                      timeout=900,
                  )
          # The share itself is one call to the other box that Nextcloud
          # gives five seconds, and a box receiving its first share takes
          # longer without KVM. A try that timed out may still have reached
          # the other box, so each try shares its own file and the test
          # accepts the offer for the one that went through.
          for attempt in range(10):
              name = f"hello-{attempt}.txt"
              # From a file: piped in, curl sends no length and Nextcloud
              # stores an empty file.
              mattbox.succeed(
                  "echo 'Hello from mattbox' > /tmp/hello.txt",
                  f"curl -sfk -u 'notshared:{a}' -T /tmp/hello.txt "
                  f"https://mattbox/remote.php/dav/files/notshared/{name}",
                  f"curl -sfk -u 'notshared:{a}' https://mattbox/remote.php/dav/files/notshared/{name}"
                  " | grep -qx 'Hello from mattbox'",
              )
              code, out = mattbox.execute(
                  f"curl -sfk -u 'notshared:{a}' -H 'OCS-APIRequest: true' "
                  f"-d path=/{name} -d shareType=6 -d shareWith=notshared@evabox "
                  "'https://mattbox/ocs/v2.php/apps/files_sharing/api/v1/shares?format=json'"
              )
              if code == 0:
                  break
          else:
              raise Exception("no federated share went through in ten tries")
          share = json.loads(out)["ocs"]
          assert share["meta"]["statuscode"] == 200, share
          assert share["data"]["share_with"] == "notshared@evabox", share

          pending = (
              f"curl -sfk -u 'notshared:{b}' -H 'OCS-APIRequest: true' "
              "'https://evabox/ocs/v2.php/apps/files_sharing/api/v1/remote_shares/pending?format=json'"
          )
          evabox.wait_until_succeeds(
              f"{pending} | jq -e --arg n {name} '.ocs.data | map(.name | ltrimstr(\"/\")) | any(. == $n)'",
              timeout=300,
          )
          offers = json.loads(evabox.succeed(pending))["ocs"]["data"]
          offer = next(o for o in offers if o["name"].lstrip("/") == name)
          assert "mattbox" in offer["remote"], offer
          # Everything the receiving box does with the share from here, in
          # the journal if the read below fails.
          evabox.succeed("nextcloud-occ log:manage --level=debug")
          evabox.succeed(
              f"curl -sfk -X POST -u 'notshared:{b}' -H 'OCS-APIRequest: true' "
              f"'https://evabox/ocs/v2.php/apps/files_sharing/api/v1/remote_shares/pending/{offer['id']}?format=json'"
          )
          # Accepted, it is a file in the other owner's LosOS Cloud, read
          # through from the first box.
          read = f"curl -sfk -u 'notshared:{b}' https://evabox/remote.php/dav/files/notshared/{name}"
          try:
              evabox.wait_until_succeeds(f"{read} | grep -qx 'Hello from mattbox'", timeout=300)
          except Exception:
              sql = "sudo -u postgres psql -d nextcloud -c "
              print(evabox.execute(read.replace("-sfk", "-sk -w '\\n%{http_code}\\n'"))[1])
              print(evabox.execute(f"{sql}'select id, remote, share_token, name, mountpoint, accepted from oc_share_external'")[1])
              print(mattbox.execute(f"{sql}'select id, share_type, share_with, file_target, token from oc_share'")[1])
              token = evabox.execute(
                  "sudo -u postgres psql -d nextcloud -tAc 'select share_token from oc_share_external order by id desc limit 1'"
              )[1].strip()
              print(evabox.execute(
                  f"curl -sk -X PROPFIND -u '{token}:' -w '\\n%{{http_code}}\\n' http://mattbox/public.php/webdav/"
              )[1])
              raise
    '';
}
