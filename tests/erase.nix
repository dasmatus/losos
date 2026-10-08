# Backup to a bucket, erase, restore: the whole round trip on one box.
#
# The box is laid out like the real one: a tmpfs root, /persist on its own
# disk, and the persisted directories bind-mounted back by impermanence, so
# the boot-time wipe deletes from the same /persist paths it deletes from on
# hardware. /persist carries ext4's `encrypt` feature, as disko formats it,
# so the shared domain is a real fscrypt policy. The bucket is a MinIO on the
# same machine, under /var/lib/minio, which the wipe does not touch (a
# bucket somewhere else is the point of the feature).
#
# What is asserted, in order:
#
#   1. a backup through lososd's API reaches the bucket, and the bucket holds
#      ciphertext only: a grep for a sentence from the owner's files finds it
#      on the box (the positive control) and not in MinIO's data;
#   2. a locked fscrypt domain is opened for the copy and locked again;
#   3. an erase can be cancelled during its countdown, and a cancelled one
#      changes nothing;
#   4. an erase that runs to its end restarts the box, and the box comes up
#      empty and unclaimed: files, database rows, the shared folder and the
#      secrets are gone, the disk key and the bucket are not;
#   5. a restore with the wrong recovery code is refused in plain words, and
#      with the right one the files, the database rows and the shared folder
#      come back, and the box takes the old recovery code again.
{ pkgs, impermanence }:

let
  lososPkgs = import ../flake/packages.nix { inherit pkgs; };
  lososPersistence =
    (import ../modules/impermanence.nix {
      config = { };
      inherit (pkgs) lib;
      inherit pkgs;
    }).environment.persistence."/persist";
  persistedDirs = builtins.filter (d: d != "/nix") lososPersistence.directories;
in

pkgs.testers.nixosTest {
  name = "losos-erase";

  nodes.machine =
    { lib, pkgs, ... }:
    {
      imports = [
        impermanence.nixosModules.impermanence
        ../modules/options.nix
        ../modules/configuration.nix
        ../modules/daemon.nix
        ../modules/fscrypt.nix
        ../modules/backup.nix
      ];

      environment.persistence."/persist" = {
        inherit (lososPersistence) hideMounts files;
        directories = persistedDirs;
      };

      losos.backend.package = lososPkgs.losos-ctl;
      # Sharing on, so the fscrypt unit opens the domain at boot and the
      # test can write into it; the test locks it again by hand before the
      # backup, which is the case the backup has to handle.
      losos.sharingMyStorage = true;
      # No TPM in this VM: the keyfile path, a supported configuration.
      losos.tpm.enable = false;
      # Container mode, the default. There is no k3s in this VM, so the
      # restore's stop and start of the local cluster find nothing to do.
      losos.nextcloud.mode = "container";
      losos.reset.graceMinutes = 1;
      losos.gpu.enable = false;

      networking.networkmanager.enable = lib.mkForce false;
      services.avahi.enable = lib.mkForce false;

      services.postgresql = {
        enable = true;
        ensureDatabases = [ "nextcloud" ];
        ensureUsers = [
          {
            name = "nextcloud";
            ensureDBOwnership = true;
          }
        ];
      };

      services.minio = {
        enable = true;
        # nixpkgs marks MinIO insecure since upstream stopped maintaining the
        # community server. Here it is a fixture on loopback inside a test
        # VM, standing in for whatever bucket the owner uses; nothing ships it.
        package = pkgs.minio.overrideAttrs (old: {
          meta = old.meta // {
            knownVulnerabilities = [ ];
          };
        });
        rootCredentialsFile = pkgs.writeText "minio-credentials" ''
          MINIO_ROOT_USER=boxbackup
          MINIO_ROOT_PASSWORD=boxbackup-secret-123
        '';
      };

      environment.systemPackages = [
        pkgs.minio-client
        pkgs.jq
        pkgs.fscrypt
      ];

      virtualisation = {
        memorySize = 2048;
        cores = 2;
        diskSize = 4096;
        fileSystems."/" = lib.mkForce {
          device = "tmpfs";
          fsType = "tmpfs";
          options = [ "mode=755" ];
        };
        fileSystems."/persist" = {
          device = "/dev/vda";
          fsType = "ext4";
          neededForBoot = true;
        };
      };

      # What disko and the installer do on a real box: ext4 with the encrypt
      # feature fscrypt needs, and the directories impermanence binds from.
      boot.initrd.systemd.initrdBin = [ pkgs.e2fsprogs ];
      boot.initrd.systemd.services.losos-test-persist = {
        description = "Give /persist the encrypt feature and seed it";
        wantedBy = [ "initrd.target" ];
        after = [ "dev-vda.device" ];
        requires = [ "dev-vda.device" ];
        # Before the fsck too: tune2fs writing the superblock while fsck
        # reads it leaves a bad checksum and the mount refuses the disk.
        before = [
          "sysroot-persist.mount"
          "systemd-fsck@dev-vda.service"
        ];
        unitConfig.DefaultDependencies = false;
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
        };
        script = ''
          # e2fsprogs is the only tool put in this initrd (no blkid, no grep),
          # so tune2fs both probes and reads the features. The test driver
          # hands over a disk that is already ext4, without `encrypt`.
          if ! features=$(tune2fs -l /dev/vda 2>/dev/null); then
            mkfs.ext4 -q -O encrypt /dev/vda
          else
            case "$features" in
              *"Filesystem features:"*" encrypt"*) ;;
              *) tune2fs -O encrypt /dev/vda ;;
            esac
          fi
        '';
      };
      boot.initrd.systemd.services.losos-test-persist-dirs = {
        description = "Seed /persist with the dirs nixos-install would have made";
        wantedBy = [ "initrd.target" ];
        requires = [ "sysroot-persist.mount" ];
        after = [ "sysroot-persist.mount" ];
        before = [ "sysroot-var.mount" ];
        unitConfig.DefaultDependencies = false;
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
        };
        script = "mkdir -p /sysroot/persist/var /sysroot/persist/etc/keys";
      };
    };

  testScript = ''
    import json
    import shlex

    NOTE = "the plate of salmon stays in the family"
    SHARED_NOTE = "lent to the mesh, kept in fscrypt"
    ROW = "a row the database must keep"

    def token():
        return machine.succeed("cat /var/secrets/losos-admin-token").strip()

    def api(method, path, body=None, expect=200):
        args = f"-s -o /tmp/api.out -w '%{{http_code}}' -X {method} -H 'Authorization: Bearer {token()}'"
        if body is not None:
            args += f" -H 'Content-Type: application/json' --data-binary {shlex.quote(json.dumps(body))}"
        code = machine.succeed(f"curl {args} http://127.0.0.1:8082{path}").strip()
        out = machine.succeed("cat /tmp/api.out")
        assert code == str(expect), f"{method} {path} answered {code}, expected {expect}: {out}"
        return json.loads(out) if out.strip() else None

    def wait_job(state):
        machine.wait_until_succeeds(
            f"curl -fsS -H 'Authorization: Bearer {token()}' http://127.0.0.1:8082/api/backup "
            f"| jq -e '.job.state == \"{state}\" or .job.state == \"failed\"'",
            timeout=300,
        )
        job = api("GET", "/api/backup")["job"]
        print("job:", job)
        assert job["state"] == state, f"job ended {job['state']}: {job.get('message')}"
        return job

    def policy():
        return machine.succeed(
            "echo /persist/home/shared/data/$(cat /etc/machine-id)/shared"
        ).strip()

    def set_bucket():
        api("POST", "/api/backup/target", {
            "endpoint": "http://127.0.0.1:9000",
            "bucket": "box-backups",
            "region": "us-east-1",
            "accessKeyId": "boxbackup",
            "secretAccessKey": "boxbackup-secret-123",
        })

    machine.start()
    machine.wait_for_unit("multi-user.target")
    machine.wait_for_unit("minio.service")
    machine.wait_for_unit("postgresql.service")
    machine.wait_until_succeeds("curl -fsS 127.0.0.1:8082/api/health")
    machine.wait_until_succeeds("test -s /var/secrets/losos-admin-token")

    with subtest("the box holds an owner's data"):
        machine.succeed(
            "mc alias set local http://127.0.0.1:9000 boxbackup boxbackup-secret-123",
            "mc mb local/box-backups",
            "install -d /var/lib/nextcloud/data/data/notshared/files",
            f"echo '{NOTE}' > /var/lib/nextcloud/data/data/notshared/files/note.txt",
            "runuser -u postgres -- psql -d nextcloud -c "
            f"\"create table notes (body text); insert into notes values ('{ROW}');\"",
        )
        machine.wait_for_unit("losos-fscrypt-shared.service")
        p = policy()
        machine.succeed(f"fscrypt status {p} | grep -c '^Unlocked: *Yes'")
        machine.succeed(f"echo '{SHARED_NOTE}' > {p}/lent.txt")
        # Lock it, as it is whenever sharing is off.
        machine.succeed(f"fscrypt lock {p} --quiet")
        machine.succeed(f"fscrypt status {p} | grep -c '^Unlocked: *No'")
        machine.fail(f"cat {p}/lent.txt")

    with subtest("the bucket's secret never comes back"):
        set_bucket()
        view = api("GET", "/api/backup")
        assert view["target"]["hasSecret"] is True, view
        assert "boxbackup-secret-123" not in json.dumps(view)
        machine.succeed("test \"$(stat -c %a /var/secrets/losos-backup.env)\" = 600")
        api("POST", "/api/backup/target", {
            "endpoint": "http://8.8.8.8:9000", "bucket": "box-backups", "accessKeyId": "boxbackup",
        }, expect=400)

    old_code = api("GET", "/api/recovery")["code"]

    with subtest("a backup reaches the bucket as ciphertext"):
        api("POST", "/api/backup/run")
        wait_job("done")
        last = api("GET", "/api/backup")["last"]
        assert last and len(last["snapshot"]) == 64, last
        # The locked domain was opened for the copy and locked again.
        machine.succeed(f"fscrypt status {policy()} | grep -c '^Unlocked: *No'")
        # Positive control first, so a grep that cannot read proves nothing.
        machine.succeed(f"grep -rqF '{NOTE}' /var/lib/nextcloud")
        machine.fail(f"grep -rqaF '{NOTE}' /var/lib/minio")
        machine.fail(f"grep -rqaF '{SHARED_NOTE}' /var/lib/minio")
        machine.fail(f"grep -rqaF '{ROW}' /var/lib/minio")
        machine.fail("test -e /var/lib/losos-backup/stage")

    with subtest("an erase can be cancelled during its countdown"):
        out = api("POST", "/api/erase", {"backup": False})
        assert out["erase"]["phase"] == "waiting" and out["erase"]["cancellable"], out
        api("POST", "/api/backup/run", expect=409)
        api("POST", "/api/erase/cancel")
        assert api("GET", "/api/backup")["erase"] is None
        machine.sleep(70)
        machine.succeed("test -e /var/lib/nextcloud/data/data/notshared/files/note.txt")

    old_token = token()

    with subtest("an erase that runs out restarts into the wipe"):
        api("POST", "/api/erase", {"backup": True})
        machine.wait_for_shutdown()

    machine.start()
    machine.wait_for_unit("multi-user.target")
    machine.wait_until_succeeds("curl -fsS 127.0.0.1:8082/api/health")
    machine.wait_until_succeeds("test -s /var/secrets/losos-admin-token")

    with subtest("the box comes back empty and unowned"):
        machine.succeed("journalctl -b -u losos-factory-wipe | grep -c 'deleting the owner'")
        machine.fail("test -e /persist/.losos-factory-wipe")
        machine.fail("test -e /var/lib/nextcloud/data/data/notshared/files/note.txt")
        machine.fail("test -e /var/secrets/losos-backup.env")
        assert token() != old_token, "the admin token survived the erase"
        claim = machine.succeed("curl -fsS 127.0.0.1:8082/api/setup/claim")
        assert '"claimed":false' in claim.replace(" ", ""), claim
        machine.wait_for_unit("postgresql.service")
        machine.fail("runuser -u postgres -- psql -d nextcloud -tAc 'select body from notes'")
        machine.wait_for_unit("losos-fscrypt-shared.service")
        machine.fail(f"test -e {policy()}/lent.txt")
        report = json.loads(machine.succeed("cat /var/lib/losos-erase/report.json"))
        assert report["hadEdge"] is False and report["erasedAt"] > 0, report
        # The bucket is somewhere the wipe does not reach.
        machine.succeed("mc ls local/box-backups | grep -c .")
        assert api("GET", "/api/recovery")["code"] != old_code

    with subtest("a restore with the wrong code is refused in plain words"):
        set_bucket()
        api("POST", "/api/backup/restore", {"code": "not a code"}, expect=400)
        api("POST", "/api/backup/restore", {"code": "0f8fad5b-d9cb-469f-a165-70867728950e"})
        job = machine.wait_until_succeeds(
            f"curl -fsS -H 'Authorization: Bearer {token()}' http://127.0.0.1:8082/api/backup "
            "| jq -e '.job.state == \"failed\"'",
            timeout=300,
        )
        job = api("GET", "/api/backup")["job"]
        assert job["message"] == "the recovery code does not open this backup", job

    with subtest("the right code brings everything back"):
        api("POST", "/api/backup/restore", {"code": old_code.upper()})
        wait_job("done")
        machine.succeed(
            f"grep -qF '{NOTE}' /var/lib/nextcloud/data/data/notshared/files/note.txt"
        )
        row = machine.succeed("runuser -u postgres -- psql -d nextcloud -tAc 'select body from notes'")
        assert row.strip() == ROW, row
        p = policy()
        machine.succeed(f"fscrypt status {p} | grep -c '^Unlocked: *Yes'")
        machine.succeed(f"grep -qF '{SHARED_NOTE}' {p}/lent.txt")
        assert api("GET", "/api/recovery")["code"] == old_code
        machine.fail("test -e /run/losos/restore-code")
        machine.fail("test -e /var/lib/losos-backup/restore")
  '';
}
