# nixos-test-vms for `losos-install --tang-edge`: the box's edge is its Tang
# server (edge/registrar/src/tang.rs, modules/network-unlock.nix), and the
# owner of a stolen box can make the edge refuse it.
#
# The same seam as nix/tests/network-unlock.nix (the installer's
# --disko-script mode on a swtpm VM, then a reboot into a specialisation with
# the production disko.nix + boot.nix + network-unlock.nix initrd). The edge
# runs the registrar as modules/edge.nix starts it, less what this test does
# not need, and hands out addresses (dnsmasq). Asserted:
#   1. `losos-install --tang-edge URL` asks the edge for a key of this box's
#      own and writes it into the target file as the edge; without the TPM,
#      or with no edge answering, nothing is touched,
#   2. the disk is bound to the TPM2 AND the edge, no systemd TPM2 token,
#   3. the next boot opens /persist unattended with the edge's answer,
#   4. once the box has a recovery code, it tells the edge the hash of the
#      owner's lost-box secret, which needs the installer's claim secret,
#   5. the owner reports the box stolen on the edge with the recovery code:
#      the edge answers for the box no more, the boot stops at the console,
#      and the recovery code still opens it there,
#   6. the owner takes the report back and the box opens unattended again.
{
  pkgs,
  disko,
}:

let
  lososPkgs = import ../packages.nix { inherit pkgs; };
  port = 8443;
  # Machines are numbered in name order: edge is .1, machine is .2.
  edgeUrl = "http://192.168.1.1:${toString port}";
  code = "6f1c2a8e-4b3d-4f0a-9c1e-2d7b5a9e8f10";
in
pkgs.testers.nixosTest {
  name = "losos-edge-unlock";

  nodes.edge =
    { lib, ... }:
    {
      environment.etc."losos-test/tenants.json".text = "{}";
      systemd.tmpfiles.rules = [ "d /var/lib/reg 0700 root root -" ];
      # The registrar as modules/edge.nix starts it with losos.edge.tang:
      # no Traefik, no tunnel, no mesh.
      systemd.services.registrar = {
        wantedBy = [ "multi-user.target" ];
        after = [ "systemd-tmpfiles-setup.service" ];
        environment.RUST_LOG = "info";
        serviceConfig.ExecStart = lib.escapeShellArgs [
          "${lososPkgs.losos-registrar}/bin/losos-registrar"
          "serve"
          "--listen"
          "0.0.0.0:${toString port}"
          "--port-range"
          "20000-20010"
          "--registry"
          "/var/lib/reg/registry.json"
          "--traefik-provider-listen"
          "127.0.0.1:8090"
          "--tunnel-config"
          "/var/lib/reg/tunnel.json"
          "--tenants-file"
          "/etc/losos-test/tenants.json"
          "--tang-dir"
          "/var/lib/reg/tang"
          "--tangd"
          "${pkgs.tang}/libexec/tangd"
          "--tangd-keygen"
          "${pkgs.tang}/libexec/tangd-keygen"
        ];
      };
      services.dnsmasq = {
        enable = true;
        settings = {
          interface = "eth1";
          bind-interfaces = true;
          port = 0;
          dhcp-range = [ "192.168.1.100,192.168.1.200,255.255.255.0,1h" ];
        };
      };
      networking.firewall.allowedTCPPorts = [ port ];
      networking.firewall.allowedUDPPorts = [ 67 ];
      environment.systemPackages = [
        pkgs.curl
        pkgs.jq
      ];
    };

  nodes.machine =
    {
      config,
      lib,
      pkgs,
      ...
    }:
    {
      imports = [
        # nix/tests/virtiofs.nix: the closure this node installs is too many
        # inodes for virtiofsd under a 20000-descriptor limit.
        ./virtiofs.nix
        disko.nixosModules.disko
        ../../modules/options.nix
        ../../modules/disko.nix
        ../../modules/boot.nix
        ../../modules/network-unlock.nix
        ../../modules/installer.nix
      ];

      losos.installer.package = lososPkgs.losos-ctl;
      losos.targetDrives = [ "/dev/vdb" ];
      # The medium's edge, which `--tang-edge` with no address asks.
      losos.proxy.registrarUrl = edgeUrl;

      disko.enableConfig = false;

      specialisation.unlock.configuration =
        { config, lib, ... }:
        let
          generated = config.disko.devices._config;
          persistFs = map (c: c."/persist") (
            builtins.filter (c: c ? "/persist") generated.fileSystems.contents
          );
        in
        {
          # The installed box's install-target.nix. The id and thumbprint are
          # stand-ins of the right shape: the edge makes them at run time.
          # The installed system reads from this entry only that the edge is
          # a server; the JWE carries the real key, and
          # /etc/keys/tang-edge.json the real address.
          losos.tpm.tang = [
            {
              url = "${edgeUrl}/tang/00000000000000000000000000000000";
              thumbprint = "known-only-at-run-time-0000";
              edge = true;
            }
          ];
          virtualisation.fileSystems."/persist" = lib.mkMerge (persistFs ++ [ { neededForBoot = true; } ]);
          boot.initrd.luks.devices = lib.mkMerge (
            map (
              c: lib.mkVMOverride (lib.attrByPath [ "initrd" "luks" "devices" ] { } c)
            ) generated.boot.contents
          );
        };

      boot.loader.efi.canTouchEfiVariables = lib.mkForce false;
      system.switch.enable = true;

      environment.etc."losos/disko-script".source = config.system.build.diskoScript;

      services.lvm.enable = true;
      environment.systemPackages = with pkgs; [
        lvm2
        cryptsetup
        util-linux
        clevis
        jose
        jq
      ];

      virtualisation = {
        memorySize = 2048;
        cores = 2;
        useBootLoader = true;
        useEFIBoot = true;
        tpm.enable = true;
        emptyDiskImages = [ 4096 ];
      };
    };

  testScript = ''
    import base64
    import hashlib
    import json
    import re

    code = "${code}"
    # What the edge's /tang/lost page computes in the owner's browser.
    secret = hashlib.sha256(f"losos-tang-lost\n{code}".encode()).hexdigest()
    lost_hash = hashlib.sha256(secret.encode()).hexdigest()

    def switch(which):
        return edge.succeed(
            f"curl -s -o /dev/null -w '%{{http_code}}' -H 'Content-Type: application/json' "
            f"-d '{{\"secret\":\"{secret}\"}}' ${edgeUrl}/tang/{which}"
        ).strip()

    edge.start()
    edge.wait_for_unit("dnsmasq.service")
    edge.wait_for_unit("registrar.service")
    edge.wait_until_succeeds("curl -sf ${edgeUrl}/health")

    with subtest("the owner's page runs only its own script"):
        headers = edge.succeed("curl -sfD - -o /tmp/lost.html ${edgeUrl}/tang/lost")
        assert re.search(r"script-src 'sha256-[A-Za-z0-9+/=]+'", headers), headers
        script_src = re.search(r"script-src [^;]*", headers)
        assert script_src and "unsafe" not in script_src.group(0), headers
        edge.succeed("grep -q 'Report a stolen box' /tmp/lost.html")
        sk = edge.succeed("curl -sf -H 'Accept-Language: sk' ${edgeUrl}/tang/lost")
        assert "Nahlásiť ukradnuté zariadenie" in sk, sk

    machine.start()
    machine.wait_for_unit("multi-user.target")

    with subtest("--tang-edge asks the edge, needs the TPM, and stops on an edge that is not there"):
        machine.succeed("losos-install --tang-edge --emit-target /tmp/edge.nix")
        target = machine.succeed("cat /tmp/edge.nix")
        m = re.search(r'\{ url = "${edgeUrl}/tang/([0-9a-f]{32})"; thumbprint = "([A-Za-z0-9_-]+)"; edge = true; \}', target)
        assert m, target
        edge.succeed(f"test -s /var/lib/reg/tang/{m.group(1)}/keys/{m.group(2)}.jwk")
        machine.fail("losos-install --no-tpm --tang-edge ${edgeUrl} --emit-target /tmp/no.nix")
        machine.fail("losos-install --tang-edge http://192.168.1.1:9 --emit-target /tmp/gone.nix")
        machine.fail("test -e /tmp/gone.nix")

    with subtest("format, mount and bind to the TPM and the edge, with nothing typed"):
        machine.succeed("losos-install --tang-edge ${edgeUrl} --disko-script /etc/losos/disko-script < /dev/null")
        key = json.loads(machine.succeed("cat /etc/keys/tang-edge.json"))
        assert machine.succeed("stat -c %a /etc/keys/tang-edge.json").strip() == "600"
        url = re.fullmatch(r"${edgeUrl}/tang/([0-9a-f]{32})", key["url"])
        assert url, key
        box = url.group(1)
        assert re.fullmatch(r"[0-9a-f]{64}", key["claim"]), key
        # The edge keeps the claim's hash, never the claim.
        stored = edge.succeed(f"cat /var/lib/reg/tang/{box}/claim").strip()
        assert stored == hashlib.sha256(key["claim"].encode()).hexdigest(), stored
        dump = machine.succeed("cryptsetup luksDump /dev/persist-vg/persist")
        assert "systemd-tpm2" not in dump, dump
        machine.succeed("clevis decrypt < /etc/keys/persist.jwe | cmp - /etc/keys/persist-keyfile")
        # The policy: the TPM2 AND the edge, both required.
        protected = machine.succeed("cat /etc/keys/persist.jwe").strip().split(".")[0]
        header = json.loads(base64.urlsafe_b64decode(protected + "=" * (-len(protected) % 4)))
        sss = header["clevis"]["sss"]
        assert sss["t"] == 2 and len(sss["jwe"]) == 2, sss
        # With the edge off the network the same JWE does not open.
        edge.succeed("systemctl stop registrar.service")
        machine.fail("timeout 30 clevis decrypt < /etc/keys/persist.jwe > /dev/null")
        edge.succeed("systemctl start registrar.service")
        edge.wait_until_succeeds("curl -sf ${edgeUrl}/health")
        machine.succeed(
            "install -d -m 700 /mnt/persist/etc/keys",
            "install -m 600 /etc/keys/persist-keyfile /mnt/persist/etc/keys/persist-keyfile",
            "install -m 600 /etc/keys/persist.jwe /mnt/persist/etc/keys/persist.jwe",
            "install -m 600 /etc/keys/tang-edge.json /mnt/persist/etc/keys/tang-edge.json",
            "sync",
            "umount -R /mnt",
            "cryptsetup close persist",
        )

    with subtest("the JWE rides in the unlock entry's initrd"):
        machine.succeed("/run/current-system/bin/switch-to-configuration boot")
        entry = machine.succeed("grep -l '^title NixOS (unlock)$' /boot/loader/entries/*.conf").strip()
        assert entry.count("\n") == 0, entry
        entry_id = entry.rsplit("/", 1)[1]
        machine.succeed(f"bootctl set-default {entry_id}")
        machine.succeed(f"sed -i 's|^default .*|default {entry_id}|' /boot/loader/loader.conf")
        machine.succeed("sync")

    def opened_unattended():
        machine.wait_for_unit("multi-user.target")
        assert machine.succeed("findmnt -no SOURCE /persist").strip() == "/dev/mapper/persist"
        clevis_log = machine.succeed("journalctl -b -u cryptsetup-clevis-persist.service")
        assert "Finished" in clevis_log, clevis_log
        assert "the disk asks" not in clevis_log, clevis_log

    with subtest("with the edge answering the box opens unattended"):
        machine.crash()
        machine.start()
        opened_unattended()

    with subtest("once it has a recovery code, the box tells the edge how its owner reports it stolen"):
        # Nothing yet: the switch finds no box.
        assert switch("lost") == "404"
        # Someone without the claim cannot name an owner.
        edge.succeed(
            f"test $(curl -s -o /dev/null -w '%{{http_code}}' -H 'Content-Type: application/json' "
            f"-d '{{\"claim\":\"{'0' * 64}\",\"lost\":\"{'1' * 64}\"}}' ${edgeUrl}/tang/{box}/owner) = 403"
        )
        machine.succeed("install -d -m 700 /var/secrets")
        machine.succeed(f"printf '%s' {code} > /var/secrets/losos-recovery-code")
        machine.wait_until_succeeds("test -s /var/lib/losos-unlock/edge-owner")
        assert machine.succeed("cat /var/lib/losos-unlock/edge-owner").strip() == lost_hash
        assert edge.succeed(f"cat /var/lib/reg/tang/{box}/lost-hash").strip() == lost_hash
        # The code is the console passphrase too (losos-unlock-recovery).
        machine.wait_until_succeeds(
            f"printf '%s' {code} | cryptsetup open --test-passphrase --key-file=- /dev/persist-vg/persist"
        )

    with subtest("reported stolen, the edge answers no one and the disk asks at the console"):
        assert switch("lost") == "204"
        edge.succeed(f"test -e /var/lib/reg/tang/{box}/lost")
        assert edge.succeed(
            f"curl -s -o /dev/null -w '%{{http_code}}' ${edgeUrl}/tang/{box}/adv"
        ).strip() == "403"
        machine.crash()
        machine.start()
        machine.wait_for_console_text("Please enter")
        machine.send_console(f"{code}\n")
        machine.wait_for_unit("multi-user.target")
        assert machine.succeed("findmnt -no SOURCE /persist").strip() == "/dev/mapper/persist"
        clevis_log = machine.succeed("journalctl -b -u cryptsetup-clevis-persist.service")
        assert "no Tang server answered" in clevis_log, clevis_log

    with subtest("the owner takes the report back and the box opens unattended again"):
        assert switch("found") == "204"
        edge.succeed(f"test ! -e /var/lib/reg/tang/{box}/lost")
        machine.crash()
        machine.start()
        opened_unattended()
  '';
}
