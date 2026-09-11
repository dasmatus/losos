# nixos-test-vms for the appliance's keyring (modules/keyring.nix).
#
# A keyring that quietly falls back to leaving the secret in plaintext is worse
# than no keyring, because someone believes it. So the assertions here are not
# "the file exists". They are:
#
#   1. the secret round-trips. The value read after a full power cycle is byte
#      for byte the value read before it, which is the only proof that the seal
#      and the unseal are carrying the real secret rather than each minting a
#      fresh one;
#   2. the plaintext is not where it used to be. /var/secrets/nextcloud-admin-pass
#      was a 0600 file on /persist; it is now a link into tmpfs, and a recursive
#      grep for the password over /var/secrets finds nothing. That grep runs
#      after a positive control, because a grep that errors out exits non-zero
#      and would otherwise "pass" this test for free;
#   3. an unprivileged process cannot read it, and the one uid that must can.
#
# Three nodes, because the three interesting configurations are mutually
# exclusive. `tpmbox` has a TPM and seals to it. `hostkeybox` has none and
# seals to /var/lib/systemd/credential.secret, the fallback that keeps
# losos.tpm.enable = false from being a brick. `offbox` has the keyring off
# and exists only to prove the option is reversible: a box that sealed once and
# then turned the keyring off must not be left with a link into a /run
# directory nothing creates any more.
#
# The two real consumers are NOT run here. modules/workloads.nix mounts the
# password into the Nextcloud pod with hostPath `type: File`, and
# services.nextcloud loads it as a systemd credential; booting k3s and a
# Nextcloud image to see that would cost more than tests/cluster-vm.nix already
# spends. What this test does instead is exercise the two *mechanisms* those
# consumers use against the symlink: a bind mount of the path (which is what
# containerd does to put the file in the pod, and the reason the runtime
# directory can stay 0700 root while the pod runs as 1002) and a systemd
# LoadCredential= of the path.
{ pkgs }:

let
  # Reads back a credential the service manager loaded for it. Stands in for
  # the native Nextcloud path, where PID 1 opens losos.nextcloud.adminpassFile
  # and copies the contents into the unit's ramfs credential directory.
  #
  # cat by store path, not by name: a transient unit inherits systemd's default
  # PATH, which on NixOS points at directories that do not exist. A bare `cat`
  # here exits 127 and looks exactly like a credential that failed to load.
  credProbe = pkgs.writeShellScriptBin "cred-probe" ''
    exec ${pkgs.coreutils}/bin/cat "$CREDENTIALS_DIRECTORY/ap"
  '';

  # A module, not an attrset spliced in with `//`: two of the three nodes set
  # `virtualisation.tpm.enable`, and a shallow merge would drop memorySize
  # along with it.
  common = _: {
    virtualisation.memorySize = 1024;
    environment.systemPackages = [ credProbe ];
  };

  keyringNode = {
    imports = [
      common
      ../modules/options.nix
      ../modules/keyring.nix
    ];
  };
in

pkgs.testers.nixosTest {
  name = "losos-keyring";

  nodes = {
    # The supported path: losos.tpm.enable defaults to true and the LUKS
    # volume is already unlocked by the same chip.
    tpmbox = _: {
      imports = [ keyringNode ];

      losos.tpm.enable = true;
      # Container mode, so the materialised secret is chowned to the uid the
      # Nextcloud pod runs as. That ownership is the half modules/workloads.nix
      # can no longer provide: it fixes it with a tmpfiles `z` rule, and
      # tmpfiles.d(5) says `z` does not follow symlinks.
      losos.nextcloud.mode = "container";

      virtualisation.tpm.enable = true;

      # The same fixed ids modules/configuration.nix pins, for the same
      # reason: the pod's runAsUser is a number, and Postgres peer auth
      # compares it.
      users.groups.nextcloud.gid = 1002;
      users.users.nextcloud = {
        uid = 1002;
        group = "nextcloud";
        isSystemUser = true;
      };
    };

    # No TPM. A design that only supported TPM2 would brick every
    # losos.tpm.enable = false machine, so this path has to work too.
    hostkeybox = _: {
      imports = [ keyringNode ];

      losos.tpm.enable = false;
      losos.nextcloud.mode = "native";
      virtualisation.tpm.enable = false;
    };

    # Keyring off. Nothing seals here; the test plants a blob and a link by
    # hand to model a box that had the keyring on and then turned it off.
    offbox = _: {
      imports = [ keyringNode ];

      losos.keyring.enable = false;
      losos.tpm.enable = false;
      losos.nextcloud.mode = "native";
      virtualisation.tpm.enable = false;
    };
  };

  testScript = ''
    P = "/var/secrets/nextcloud-admin-pass"
    BLOB = P + ".cred"
    RUN = "/run/losos-keyring/nextcloud-admin-pass"
    CRED = "losos-nextcloud-admin-pass"

    start_all()

    for m in (tpmbox, hostkeybox):
        m.wait_for_unit("multi-user.target")
        m.wait_for_unit("losos-keyring.service")
    offbox.wait_for_unit("multi-user.target")

    with subtest("the two key modes are actually different"):
        # Without this the rest of the test would pass on two identical boxes.
        tpmbox.succeed("test -e /dev/tpmrm0")
        hostkeybox.fail("test -e /dev/tpmrm0")

    with subtest("the keyring materialises a secret at the path consumers name"):
        pw = tpmbox.succeed(f"cat {P}").strip()
        # 24 random bytes, base64 — the same secret modules/nextcloud-common.nix
        # mints. If this is empty, flake/images.nix refuses to install Nextcloud
        # at all rather than installing it with a blank admin password.
        assert len(pw) == 32, f"the minted password looks wrong: {pw!r}"
        tpmbox.succeed(f"test -L {P}")
        assert tpmbox.succeed(f"readlink {P}").strip() == RUN
        # What the kubelet's hostPath `type: File` check does: os.Stat, which
        # follows the link. A link that resolves to nothing is a pod that never
        # starts.
        tpmbox.succeed(f"test -f {P}")

    with subtest("the durable copy is ciphertext and the plaintext is on tmpfs"):
        assert tpmbox.succeed(f"stat -f -c %T {P}").strip() == "tmpfs", \
            "the decrypted secret is not on tmpfs — it is on /persist"
        assert tpmbox.succeed(f"stat -f -c %T {BLOB}").strip() != "tmpfs", \
            "the sealed blob is on tmpfs and would not survive a reboot"

        # Positive control. `fail()` is satisfied by any non-zero exit, and a
        # grep that cannot run exits 2, so without this the assertion below
        # would pass whether or not the password is on disk.
        tpmbox.succeed(f"cp {P} /var/secrets/probe")
        tpmbox.succeed(f"grep -rqFf {P} /var/secrets")
        tpmbox.succeed("rm -f /var/secrets/probe")

        # -r does not follow symlinks it finds while recursing, so this reads
        # the sealed blob and skips the link. The password must not be in it.
        tpmbox.fail(f"grep -rqFf {P} /var/secrets /var/lib")

    with subtest("an unprivileged process cannot read the secret"):
        assert tpmbox.succeed("stat -c %a /run/losos-keyring").strip() == "700"
        tpmbox.fail(f"runuser -u nobody -- cat {P}")
        tpmbox.fail("runuser -u nobody -- cat " + RUN)

    with subtest("the pod's uid can read it the way the pod does"):
        # The runtime directory is 0700 root, so uid 1002 cannot walk into it.
        # It does not have to: containerd resolves the host path as root and
        # bind-mounts the file into the pod, where only the file's own mode and
        # owner apply. This is that, without booting k3s.
        #
        # -L is load-bearing. `stat` lstat()s by default, so without it this
        # reports the symlink's own 0:0 and passes whatever the secret is
        # owned by. (`stat -f` above is the other way round: statfs() always
        # resolves, which is why it sees the tmpfs and not /var's ext4.)
        assert tpmbox.succeed(f"stat -L -c %u:%g {P}").strip() == "1002:1002"
        tpmbox.fail("runuser -u nextcloud -- cat " + RUN)

        tpmbox.succeed("touch /tmp/probe")
        tpmbox.succeed(f"mount --bind {P} /tmp/probe")
        assert tpmbox.succeed("cat /tmp/probe").strip() == pw
        assert tpmbox.succeed("runuser -u nextcloud -- cat /tmp/probe").strip() == pw, \
            "the Nextcloud pod could not read its own admin password"
        tpmbox.succeed("umount /tmp/probe")

    with subtest("systemd can load it as a credential through the link"):
        # The native path: services.nextcloud passes adminpassFile to
        # LoadCredential=, and PID 1 opens it as root before the unit's own
        # process exists.
        native_pw = hostkeybox.succeed(f"cat {P}").strip()
        assert hostkeybox.succeed(f"stat -L -c %u:%g {P}").strip() == "0:0", \
            "in native mode the secret should stay root-owned"
        out = hostkeybox.succeed(
            f"systemd-run --wait --pipe -q -p LoadCredential=ap:{P}"
            " /run/current-system/sw/bin/cred-probe"
        )
        assert out.strip() == native_pw, \
            f"LoadCredential= did not see the secret through the link: {out!r}"

    with subtest("the key each box seals with follows the option, not the hardware"):
        # The discriminating check. --with-key=tpm2 never touches the host key
        # file, and --with-key=host is the only thing here that creates it.
        tpmbox.fail("test -e /var/lib/systemd/credential.secret")
        hostkeybox.succeed("test -e /var/lib/systemd/credential.secret")
        assert hostkeybox.succeed(
            "stat -c %a /var/lib/systemd/credential.secret"
        ).strip() == "400", "the host credential key is not root-only"

    with subtest("the secret survives a power cycle, on both key modes"):
        # The assertion the whole module exists for. A full shutdown, not a
        # soft reboot: /run is thrown away and the only thing carried over is
        # the disk image and, on tpmbox, the swtpm state directory.
        tpm_blob = tpmbox.succeed(f"sha256sum {BLOB}").split()[0]
        host_pw = hostkeybox.succeed(f"cat {P}").strip()
        host_blob = hostkeybox.succeed(f"sha256sum {BLOB}").split()[0]

        for m in (tpmbox, hostkeybox):
            m.shutdown()
            m.start()
            m.wait_for_unit("losos-keyring.service")

        assert tpmbox.succeed(f"cat {P}").strip() == pw, \
            "the TPM-sealed secret did not survive the power cycle"
        assert hostkeybox.succeed(f"cat {P}").strip() == host_pw, \
            "the host-key-sealed secret did not survive the power cycle"

        # And it was unsealed, not re-minted: a second seal would rewrite the
        # blob, and a box that re-mints every boot has a keyring in name only.
        assert tpmbox.succeed(f"sha256sum {BLOB}").split()[0] == tpm_blob
        assert hostkeybox.succeed(f"sha256sum {BLOB}").split()[0] == host_blob

    with subtest("a box upgraded from the plaintext era keeps its password"):
        # The migration case. An installed box already has Nextcloud set up with
        # the password in that file; minting a new one instead of adopting it
        # would leave the admin account on a password nothing knows.
        hostkeybox.succeed("systemctl stop losos-keyring.service")
        # RUN too: RuntimeDirectoryPreserve=yes keeps the materialised secret
        # across a stop, and leaving it there would hide the adoption branch.
        hostkeybox.succeed(f"rm -f {BLOB} {P} {RUN}")
        hostkeybox.succeed(f"install -m 0600 /dev/null {P}")
        hostkeybox.succeed(f"echo legacy-secret > {P}")
        hostkeybox.succeed("systemctl start losos-keyring.service")

        assert hostkeybox.succeed(f"cat {P}").strip() == "legacy-secret", \
            "the keyring discarded the password an installed box was using"
        hostkeybox.succeed(f"test -L {P}")
        hostkeybox.succeed(f"test -f {BLOB}")
        hostkeybox.fail(f"grep -qF legacy-secret {BLOB}")

    with subtest("a blob that will not decrypt is kept, said out loud, and replaced"):
        # TPM cleared, host key lost, blob truncated by a bad restore. There is
        # no shell to recover from, so the box re-mints and keeps the old
        # ciphertext rather than deleting it. Safe for this secret in
        # particular: Nextcloud reads it at install time only, and
        # `losos-ctl set-password` can reset the account afterwards.
        hostkeybox.succeed("systemctl stop losos-keyring.service")
        hostkeybox.succeed(f"rm -f {RUN}")
        hostkeybox.succeed(f"printf 'not a credential\\n' > {BLOB}")
        hostkeybox.succeed("systemctl start losos-keyring.service")

        assert hostkeybox.succeed(f"cat {P}").strip() not in ("legacy-secret", "not a credential")
        hostkeybox.succeed(f"ls {BLOB}.unreadable.*")
        hostkeybox.succeed(f"test -f {BLOB}")
        journal = hostkeybox.succeed("journalctl -u losos-keyring.service --no-pager")
        assert "did not decrypt" in journal, \
            "the failure was silent; a keyring nobody can see fail is not a keyring"

    with subtest("turning the keyring off restores the plaintext instead of bricking"):
        # A dangling link at this path is not cosmetic: the generator in
        # modules/nextcloud-common.nix would write through it into a directory
        # that no longer exists, fail, and take the kubelet down with it
        # (modules/workloads.nix makes k3s Require that unit).
        offbox.succeed("install -d -m 0700 /var/secrets")
        offbox.succeed("printf 'restored-secret\\n' > /run/plain")
        offbox.succeed(f"systemd-creds --with-key=host encrypt --name={CRED} /run/plain {BLOB}")
        offbox.succeed("rm -f /run/plain")
        offbox.succeed(f"ln -sfn {RUN} {P}")
        offbox.succeed(f"test ! -e {P}")  # the link dangles: /run/losos-keyring is not there

        offbox.succeed("systemctl restart losos-keyring-restore.service")

        offbox.fail(f"test -L {P}")
        assert offbox.succeed(f"cat {P}").strip() == "restored-secret", \
            "the way back off the keyring lost the secret"
        assert offbox.succeed(f"stat -c %a {P}").strip() == "600"
  '';
}
