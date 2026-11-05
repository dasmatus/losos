# nixos-test-vms for the edge-prefilled installer medium
# (nixosConfigurations.iso-edge, losos.installer.tangEdge + autorun).
#
# The seam of nix/tests/iso-install.nix (the installer walks its real clone +
# disko path, offline, because the tree and its inputs ride in the store) on
# the edge machinery of nix/tests/edge-unlock.nix (a registrar serving Tang
# on the 192.168.1/24, the medium node on .2). What is new here is the
# medium: nobody types anything, the firmware menu times out into
# autodetect, and the `--tang-edge` prefilled into the autorun unit does the
# asking. Asserted:
#   1. the tty1 unit really carries the prefilled flag,
#   2. the unattended install binds the target disk to the TPM2 AND the
#      medium's edge, whose registry names the box's claim hash, and whose
#      answer is required both at boot and at the `clevis decrypt` beside it,
#   3. with the edge off the network, that same JWE opens no more,
#   4. the same flavor on a chip-less box stops with its reason and touches
#      no disk and no registry entry, instead of installing weaker.
#
# The medium's prints are read from the virtual console (/dev/vcs1, the
# screen of tty1, where the autorun unit runs): a unit writing to a tty
# reaches neither the guest journal nor the serial line the test driver
# reads, so the installer's own words on screen are the observable, kept
# brief enough to survive a row wrap intact.
#
# The test stops once the disk is bound; nixos-install's tail drives no
# assertion here (the closed loop of an install that boots the installed
# disk is nix/tests/iso-install.nix's, and of a disk that opens through its
# edge afterwards is nix/tests/edge-unlock.nix's).
{
  pkgs,
  self,
}:

let
  inherit (pkgs) lib;

  lososPkgs = import ../packages.nix { inherit pkgs; };

  # Machines are numbered in name order: edge is .1, machine is .2, tpmless
  # is .3 — the same convention nix/tests/edge-unlock.nix takes, and the
  # reason the chip-less node is *not* read `chipless`.
  port = 8443;
  edgeUrl = "http://192.168.1.1:${toString port}";

  # This tree as a repository to clone, and every flake input's source under
  # the NAR hash its flake.lock pins: the clone and its evaluation read no
  # network (nix/tests/iso-install.nix says why both exist).
  repo = pkgs.runCommand "losos-repo" { nativeBuildInputs = [ pkgs.git ]; } ''
    cp -r ${self} tree
    chmod -R u+w tree
    cd tree
    git init -q -b main
    # A commit starts `git maintenance run --auto --detach`, whose repack
    # renames packs under the clone below ("failed to copy file to …tmp_pack").
    git config maintenance.auto false
    git add -A
    git -c user.name=losos -c user.email=losos@local commit -qm tree
    git clone -q --bare . $out
  '';

  sources =
    let
      walk = inputs: lib.concatMap (i: [ i.outPath ] ++ walk (i.inputs or { })) (lib.attrValues inputs);
    in
    lib.unique (walk (removeAttrs self.inputs [ "self" ]));

  # The medium node's module stack: edge-unlock.nix's, since the flavor's
  # subject is exactly what that stack realizes — an installer whose target
  # is written for the TPM path and whose boot opens through the edge.
  # `tangEdge` turns the medium's autorun from the plain installer into the
  # edge-prefilled one; the chip is the only thing that varies between the
  # two nodes built from this module.
  medium = tpm: { lib, pkgs, ... }: {
    imports = [
      # The system the clone evaluates, as in iso-install.nix: too many
      # inodes for virtiofsd's default descriptor limit.
      (import ./virtiofs.nix pkgs)
      self.inputs.disko.nixosModules.disko
      ../../modules/options.nix
      ../../modules/disko.nix
      ../../modules/boot.nix
      ../../modules/network-unlock.nix
      ../../modules/installer.nix
    ];

    losos.installer.package = lososPkgs.losos-ctl;
    losos.targetDrives = [ "/dev/vdb" ];
    losos.installer.autorun = true;
    losos.installer.tangEdge = true;
    # The medium's edge, which the prefilled `--tang-edge` with no
    # address asks.
    losos.proxy.registrarUrl = edgeUrl;

    disko.enableConfig = false;
    boot.loader.efi.canTouchEfiVariables = lib.mkForce false;
    system.switch.enable = true;

    services.lvm.enable = true;
    environment.systemPackages = with pkgs; [
      clevis
      cryptsetup
      lvm2
      util-linux
    ];

    nix.settings = {
      experimental-features = [
        "nix-command"
        "flakes"
      ];
      substituters = lib.mkForce [ ];
    };

    virtualisation = {
      # Evaluating the clone disko runs against takes a few GiB, as
      # iso-install.nix says for the same reason.
      memorySize = 6144;
      cores = 2;
      useBootLoader = true;
      useEFIBoot = true;
      # The chip when there is one, and the disk the flavor binds to it and
      # to the edge. The root is a separate drive detection skips
      # (edge-unlock.nix).
      tpm.enable = tpm;
      emptyDiskImages = [ 4096 ];
    };

    virtualisation.additionalPaths = [ repo ] ++ sources;

    # iso-install.nix's offline clone, but on the autorun unit (which is
    # the flavor's subject) instead of on a hand-typed command: without it
    # the unattended install clones the public repo, dies on the network
    # this test has none of, and the format never runs.
    systemd.services.losos-installer.environment.LOSOS_FLAKE_URL = "file://${repo}";
  };
in
pkgs.testers.runNixOSTest {
  name = "losos-installer-edge";
  # With KVM the test takes minutes. Under TCG (a runner without /dev/kvm)
  # the VM's evaluation of the clone alone passes the default hour.
  globalTimeout = 4 * 3600;

  nodes.edge =
    { lib, ... }:
    {
      environment.etc."losos-test/tenants.json".text = "{}";
      systemd.tmpfiles.rules = [ "d /var/lib/reg 0700 root root -" ];
      # The registrar as modules/edge.nix starts it with losos.edge.tang,
      # stripped of what this test does not need (edge-unlock.nix).
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
      environment.systemPackages = [ pkgs.curl ];
    };

  nodes.machine = medium true;

  # The same medium with the chip gone from the slot. Nothing here needs a
  # target beyond the one it will never touch: what this node asserts is
  # that with no chip the flavor stops with its reason, before anything was
  # proposed, let alone wiped.
  nodes.tpmless = medium false;

  testScript = ''
    import base64
    import hashlib
    import json
    import re

    # The visible screen of tty1, where the autorun unit runs: the one
    # place a unit writing to a tty can be asserted on (see the header).
    onScreen = "cat /dev/vcs1 | grep -aq"

    edge.start()
    edge.wait_for_unit("registrar.service")
    edge.wait_until_succeeds("curl -sf ${edgeUrl}/health")

    machine.start()
    machine.wait_for_unit("multi-user.target")

    with subtest("the tty1 unit really carries the prefilled unlock"):
        unit = machine.succeed("systemctl cat losos-installer.service")
        assert "--tang-edge" in unit, unit
        # The flavor's own words on the medium's screen: the plain chipful
        # default would print a line of its own about the chip alone.
        machine.wait_until_succeeds(
            f"{onScreen} 'unlock: TPM2 and a Tang server on this network'",
            timeout=600,
        )

    with subtest("the unattended install binds the disk to the TPM2 AND the edge"):
        machine.wait_until_succeeds("test -s /etc/keys/tang-edge.json", timeout=1800)
        key = json.loads(machine.succeed("cat /etc/keys/tang-edge.json"))
        assert machine.succeed("stat -c %a /etc/keys/tang-edge.json").strip() == "600"
        m = re.fullmatch(r"${edgeUrl}/tang/([0-9a-f]{32})", key["url"])
        assert m, key
        box = m.group(1)
        assert re.fullmatch("[0-9a-f]{64}", key["claim"]), key
        # The edge keeps the claim's hash, never the claim.
        stored = edge.succeed(f"cat /var/lib/reg/tang/{box}/claim").strip()
        assert stored == hashlib.sha256(key["claim"].encode()).hexdigest(), stored
        dump = machine.succeed("cryptsetup luksDump /dev/persist-vg/persist")
        assert "systemd-tpm2" not in dump, dump
        # The policy: the TPM2 AND the edge, both required (sss t of 2).
        protected = machine.succeed("cat /etc/keys/persist.jwe").strip().split(".")[0]
        header = json.loads(base64.urlsafe_b64decode(protected + "=" * (-len(protected) % 4)))
        sss = header["clevis"]["sss"]
        assert sss["t"] == 2 and len(sss["jwe"]) == 2, sss
        machine.succeed("clevis decrypt < /etc/keys/persist.jwe | cmp - /etc/keys/persist-keyfile")

    with subtest("with the edge off the network that same JWE opens no more"):
        edge.succeed("systemctl stop registrar.service")
        machine.fail("timeout 30 clevis decrypt < /etc/keys/persist.jwe > /dev/null")
        edge.succeed("systemctl start registrar.service")
        edge.wait_until_succeeds("curl -sf ${edgeUrl}/health")

    with subtest("the same flavor on a chip-less box stops with its reason"):
        tpmless.start()
        tpmless.wait_for_unit("multi-user.target")
        # The wrapper's shell under the stopped installer: the box had its
        # whole unattended window (the menu, the chip check and the stop),
        # and what follows holds only within that window checked below.
        tpmless.wait_until_succeeds(
            f"{onScreen} 'The installer has stopped'",
            timeout=600,
        )
        tpmless.succeed(f"{onScreen} 'needs the TPM2'")
        # Nothing weaker went ahead of the chip: no staged keys, no
        # partitions on the target, and on the edge's registry no key but
        # the live box's own.
        tpmless.succeed("test ! -e /etc/keys/persist.jwe")
        tpmless.succeed("test ! -e /etc/keys/tang-edge.json")
        tpmless.succeed("test -z \"$(blkid /dev/vdb 2>/dev/null)\"")
        entries = edge.succeed("ls /var/lib/reg/tang").strip().splitlines()
        assert len(entries) == 1 and entries[0] == box, entries
  '';
}
