# The appliance installed the way an owner installs it, and booted.
#
# The installer medium's system boots in a VM with one empty disk. The real
# `losos-install` (no --disko-script seam, no --drives) finds the disk,
# clones this tree from LOSOS_FLAKE_URL, writes modules/install-target.nix,
# formats with disko from the clone and runs nixos-install from the clone.
# The VM then powers off, the disk boots on its own (SeaBIOS, GRUB, LUKS
# opened by the keyfile in the initrd, since the VM has no TPM), the
# box's front door answers on port 80 and lososd answers behind it. That is the check the other install
# tests leave out: install.nix formats through a prebuilt disko script and
# installs the test system, tpm.nix the same with swtpm, and CI's iso job
# boots the medium but stops at the installer's first DNS query.
#
# It runs offline, in the Nix sandbox or a session without network. Three
# things make that work:
#
#   * LOSOS_FLAKE_URL is a bare git repository of this tree in the store, so
#     the clone reads no network;
#   * every flake input's source is in the VM's store under the NAR hash
#     flake.lock pins, so evaluating the clone fetches nothing;
#   * the system the clone evaluates to is built here, from the same tree
#     plus the install-target.nix the installer writes for this VM, and put
#     in the VM's store with the disko script, so nixos-install copies it
#     and builds nothing. The guest has no substituters: if the clone
#     evaluates to anything else, the install fails on the first build, and
#     the log names the derivation.
#
# The medium's own image modules (installation-cd's ISO image, the signed
# loader and its stage-1 hash check, the live branding) are left out: they
# build and check the medium, which CI's iso job boots under OVMF and
# SeaBIOS. Everything the installer runs on is the medium's: the installer
# module, losos-ctl, disko, the cache settings and the boot screen.
#
#   nix build .#checks.x86_64-linux.losos-iso-install -L
{
  pkgs,
  self,
}:
let
  inherit (pkgs) lib;

  # What losos-install writes for this VM (installer.rs, render_target):
  # the one empty disk, no TPM, BIOS firmware.
  drive = "/dev/vda";
  target = {
    losos.targetDrives = [ drive ];
    losos.tpm.enable = false;
    losos.bios = true;
  };
  installed = self.nixosConfigurations.install.extendModules { modules = [ target ]; };

  # This tree as a repository to clone: what `git clone` of the branch gives.
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

  # Every input's source, inputs of inputs included.
  sources =
    let
      walk = inputs: lib.concatMap (i: [ i.outPath ] ++ walk (i.inputs or { })) (lib.attrValues inputs);
    in
    lib.unique (walk (removeAttrs self.inputs [ "self" ]));
in
pkgs.testers.runNixOSTest {
  name = "losos-iso-install";
  # The installed closure is too many inodes for virtiofsd under a
  # 20000-descriptor limit (nix/tests/virtiofs.nix).
  defaults.imports = [ ./virtiofs.nix ];
  # installation-device.nix adds an overlay, as the medium's system does.
  node.pkgsReadOnly = false;

  nodes.installer =
    { modulesPath, ... }:
    {
      imports = [
        self.inputs.impermanence.nixosModules.impermanence
        self.inputs.disko.nixosModules.disko
        "${modulesPath}/profiles/installation-device.nix"
        ../../modules/options.nix
        ../../modules/cache.nix
        ../../modules/disko.nix
        ../../modules/installer.nix
        ../../modules/branding.nix
        ../../modules/splash.nix
      ];
      environment.systemPackages = [
        pkgs.disko
        pkgs.cryptsetup
        pkgs.git
      ];
      losos.installer.package = self.packages.${pkgs.stdenv.hostPlatform.system}.losos-ctl;
      # The test runs losos-install itself, so its exit code and output
      # reach the log; the tty1 unit runs the same command under the splash.
      losos.installer.autorun = false;
      disko.enableConfig = false;

      nix.settings = {
        substituters = lib.mkForce [ ];
        # Most of the installed system is small derivations that nix never
        # substitutes (allowSubstitutes = false: units, /etc entries). They
        # are in this store already, prebuilt; without this nixos-install
        # rebuilds them, and with no network that means stdenv from source.
        always-allow-substitutes = true;
        experimental-features = [
          "nix-command"
          "flakes"
        ];
      };

      # The empty disk is the main one, handed to the box below, so it is
      # /dev/vda during the install and after it. The installer's own root
      # is a small second disk, mounted, so detection skips it.
      virtualisation.diskImage = "./target.qcow2";
      virtualisation.diskSize = 24 * 1024;
      virtualisation.emptyDiskImages = [ 4096 ];
      virtualisation.rootDevice = "/dev/vdb";
      # The initrd is systemd's, so systemd-makefs formats it on first boot.
      virtualisation.fileSystems."/".autoFormat = true;
      # The store writes (the clone's source, the .drv files of the whole
      # system) go to that disk, not to RAM.
      virtualisation.writableStoreUseTmpfs = false;
      # Evaluating the appliance takes a few GiB.
      virtualisation.memorySize = 6144;
      virtualisation.cores = 4;

      virtualisation.additionalPaths = [
        installed.config.system.build.toplevel
        installed.config.system.build.destroyFormatMount
        repo
      ]
      ++ sources;
    };

  # The installed disk, booted by its own boot loader. The node's NixOS
  # configuration only sets up QEMU: what boots is what the installer wrote.
  nodes.box = {
    virtualisation.diskImage = "./target.qcow2";
    virtualisation.diskSize = 24 * 1024;
    virtualisation.useBootLoader = true;
    virtualisation.useDefaultFilesystems = false;
    virtualisation.fileSystems."/" = {
      device = "/dev/disk/by-label/unused";
      fsType = "ext4";
    };
    virtualisation.memorySize = 4096;
    virtualisation.cores = 4;
    virtualisation.forwardPorts = [
      {
        from = "host";
        host.port = 18080;
        guest.port = 80;
      }
    ];
  };

  testScript = ''
    import json
    import time
    import urllib.error
    import urllib.request

    installer.start()
    installer.wait_for_unit("multi-user.target")

    with subtest("the installer clones this tree, formats the disk and installs it"):
        installer.succeed(
            "LOSOS_FLAKE_URL=file://${repo} losos-install --bios >&2"
        )

    with subtest("it installed onto the disk it found, from the target it wrote"):
        target = installer.succeed("cat /mnt/persist/etc/nixos/modules/install-target.nix")
        assert 'losos.targetDrives = [ "${drive}" ];' in target, target
        assert "losos.tpm.enable = false;" in target, target
        assert "losos.bios = true;" in target, target
        installer.succeed("git -C /mnt/persist/etc/nixos ls-files --error-unmatch modules/install-target.nix")
        installer.succeed("test -e /mnt/persist/nix/var/nix/profiles/system")

    installer.shutdown()
    # Each machine keeps its disk in its own state directory: hand the
    # installed one over.
    (installer.state_dir / "target.qcow2").rename(box.state_dir / "target.qcow2")

    # No test backdoor on the installed box: every answer comes over the
    # network, through QEMU's port forward. The admin pages are LAN only,
    # and the forward arrives from 10.0.2.2, a LAN address.
    def answer(path, check, within):
        deadline = time.monotonic() + within
        last = None
        while True:
            try:
                with urllib.request.urlopen(f"http://127.0.0.1:18080{path}", timeout=10) as r:
                    body = r.read().decode(errors="replace")
                    print(f"GET {path} -> {r.status}")
                    check(r.status, body)
                    return body
            except (OSError, AssertionError, ValueError) as e:
                if str(e) != str(last):
                    print(f"GET {path}: {e}")
                    last = e
            if time.monotonic() > deadline:
                raise Exception(f"GET {path}: no good answer within {within} s: {last}")
            time.sleep(10)

    def front_door(status, body):
        assert status == 200, status
        assert "<title>LosOS</title>" in body, body[:400]

    # nginx serves the page whether lososd runs or not; the claim route is
    # lososd's own answer (nginx gives 502 while it is down), and it is one
    # of the routes that needs no sign-in.
    def claim(status, body):
        assert status == 200, status
        state = json.loads(body)
        assert state["claimed"] is False, state

    with subtest("the installed disk boots and the box answers on its front door"):
        box.start()
        answer("/", front_door, 3600)
        box.screenshot("installed")

    with subtest("lososd answers behind it, on a box nobody has claimed yet"):
        print(answer("/api/setup/claim", claim, 600))
  '';
}
