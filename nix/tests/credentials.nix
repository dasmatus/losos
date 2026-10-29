# nixos-test-vms config for the credential store (modules/credentials.nix,
# backend/src/credentials.rs, the Credentials pane).
#
# A credential store that quietly leaves a secret in plaintext is worse than
# none, because someone believes it. So nothing here asserts "the file
# exists". It asserts:
#
#   1. at rest there is only ciphertext. A recursive grep for each value over
#      the disk finds nothing, after a positive control (a grep that cannot
#      run exits non-zero too and would pass for free);
#   2. a service reads its secret through LoadCredentialEncrypted=, and a
#      process that is not that service cannot;
#   3. a box upgraded from an older build keeps every value it had. The
#      plaintext files are adopted byte for byte (the spare key still opens
#      the API, LosOS Login gets the keys it had) and then deleted, and the
#      old keyring's blob is adopted the same way;
#   4. a box without a TPM seals to the host key, and one with a TPM to the
#      chip, with no PCRs;
#   5. the values survive a power cycle unchanged, and a sealed file that no
#      longer opens is kept aside, said out loud and replaced;
#   6. the API lists every credential and none of their values, makes a new
#      value for one that can take it and restarts what reads it, and
#      refuses the rest.
#
# Three nodes. `box` has a TPM and is a real control plane: lososd, nginx,
# LosOS Login, the backup pane's routes and the built admin UI. `hostbox` has
# no TPM and only the store, with two probe secrets: one a DynamicUser unit
# reads by LoadCredentialEncrypted=, one delivered as a file the way the
# Nextcloud pod's password is. `laptop` signs in on the LAN and takes the
# pictures the pull request shows (the derivation's output, one folder per
# plan).
#
# `before`, a path to an earlier admin-ui/app, builds that UI instead and
# only takes the pictures, for the Before column:
#
#   nix build --impure --expr '(import ./nix/tests/credentials.nix {
#     pkgs = (builtins.getFlake (toString ./.)).inputs.nixpkgs.legacyPackages.x86_64-linux;
#     before = /path/to/main/admin-ui/app; })'
{
  pkgs,
  before ? null,
}:

let
  inherit (pkgs) lib;
  lososPkgs = import ../packages.nix { inherit pkgs; };
  screens = import ./screens { inherit pkgs; };
  after = before == null;

  # The admin UI as it ships, without the Lab page, which this test never
  # opens and whose WebAssembly takes longer to build than the test to run.
  ui = import ./admin-ui-without-lab.nix (
    # The earlier UI shares this one's lock file, so its node_modules too.
    lososPkgs.losos-admin-ui.overrideAttrs (_: lib.optionalAttrs (!after) { src = before; })
  );

  # What a reader of `probe` saw: a digest, so the value itself is never
  # written anywhere but the unit's own credential directory.
  probeReader = pkgs.writeShellScript "cred-probe" ''
    ${pkgs.coreutils}/bin/sha256sum < "$CREDENTIALS_DIRECTORY/probe" \
      | ${pkgs.coreutils}/bin/cut -d' ' -f1 > "$RUNTIME_DIRECTORY/seen"
  '';

  plan =
    name: attrs:
    pkgs.writeText "plan-${name}.json" (
      builtins.toJSON (
        {
          width = 1280;
          height = 800;
        }
        // attrs
      )
    );

  paneReady = {
    role = "heading";
    name = "Credentials";
  };
  # Drawn once the list has come back, with the bucket's row in it.
  listed = {
    selector = "[data-testid=\"credential\"][data-name=\"backup-bucket\"]";
  };
  securityReady = {
    role = "heading";
    name = "Security";
  };

  plans = {
    light = plan "light" {
      shots = [
        {
          name = "security";
          path = "/settings/security";
          waitFor = securityReady;
        }
      ]
      ++ lib.optionals after [
        {
          name = "credentials";
          path = "/settings/credentials";
          waitFor = paneReady;
          steps = [ { waitFor = listed; } ];
          fullPage = true;
        }
        {
          name = "credentials-rotate";
          path = "/settings/credentials";
          waitFor = paneReady;
          steps = [
            { waitFor = listed; }
            {
              click = {
                selector = "[data-name=\"authelia-session\"] button";
              };
            }
            {
              waitFor = {
                role = "dialog";
              };
            }
          ];
        }
      ];
    };
    dark = plan "dark" {
      colorScheme = "dark";
      shots = [
        {
          name = "security";
          path = "/settings/security";
          waitFor = securityReady;
        }
      ]
      ++ lib.optionals after [
        {
          name = "credentials";
          path = "/settings/credentials";
          waitFor = paneReady;
          steps = [ { waitFor = listed; } ];
          fullPage = true;
        }
      ];
    };
    phone = plan "phone" {
      width = 390;
      height = 844;
      shots = lib.optionals after [
        {
          name = "credentials";
          path = "/settings/credentials";
          waitFor = paneReady;
          steps = [ { waitFor = listed; } ];
        }
      ];
    };
  };
in

pkgs.testers.nixosTest {
  name = if after then "losos-credentials" else "losos-credentials-before";

  nodes = {
    box =
      { config, lib, ... }:
      {
        imports = [
          ../../modules/options.nix
          ../../modules/containers.nix
          ../../modules/tls.nix
          ../../modules/daemon.nix
          ../../modules/identity.nix
          ../../modules/fscrypt.nix
          ../../modules/backup.nix
        ];
        losos = {
          hostName = "mattbox";
          tpm.enable = true;
          shared.fscrypt.enable = false;
          backend.package = lososPkgs.losos-ctl;
          backend.plugins = lososPkgs.losos-plugins;
          admin.ui = ui;
          git.enable = false;
        };
        # systemd-creds and systemctl on it as on a real box, where NixOS
        # appends systemd to every unit's path; mkForce drops that default.
        systemd.services.lososd.path = lib.mkForce [
          pkgs.coreutils
          config.systemd.package
        ];
        # The backup scripts name its pg_dump; no database runs here.
        services.postgresql.package = pkgs.postgresql;
        environment.systemPackages = [ pkgs.jq ];
        virtualisation = {
          memorySize = 1536;
          cores = 2;
          vlans = [ 1 ];
          tpm.enable = true;
        };
      };

    # No TPM, the store and nothing else.
    hostbox =
      { ... }:
      {
        imports = [
          ../../modules/options.nix
          ../../modules/credentials.nix
        ];
        losos.tpm.enable = false;
        losos.credentials.entries = {
          probe = {
            app = "test";
            description = "Read by cred-probe.service.";
            format = "hex-32";
            services = [ "cred-probe" ];
            rotate = true;
          };
          # The Nextcloud password's shape: a path some reader has baked in,
          # once a plaintext file, then the old keyring's blob beside it.
          probe-file = {
            app = "test";
            description = "Delivered as a file.";
            format = "base64-24";
            legacy = [ "/var/secrets/probe-file" ];
            legacySealed = {
              path = "/var/secrets/probe-file.cred";
              name = "losos-probe-file";
            };
            file = {
              path = "/var/secrets/probe-file";
              owner = "1002";
              mode = "0400";
            };
          };
        };
        users.groups.probe.gid = 1002;
        users.users.probe = {
          uid = 1002;
          group = "probe";
          isSystemUser = true;
        };
        systemd.services.cred-probe = {
          wantedBy = [ "multi-user.target" ];
          serviceConfig = {
            Type = "oneshot";
            RemainAfterExit = true;
            DynamicUser = true;
            RuntimeDirectory = "cred-probe";
            RuntimeDirectoryPreserve = "yes";
            ExecStart = probeReader;
          };
        };
        virtualisation = {
          memorySize = 768;
          tpm.enable = false;
        };
      };

    laptop =
      { lib, ... }:
      {
        imports = [ screens.browser ];
        environment.systemPackages = [
          pkgs.curl
          pkgs.jq
        ];
        environment.etc = lib.mapAttrs' (
          name: file: lib.nameValuePair "losos-test/plan-${name}.json" { source = file; }
        ) plans;
        virtualisation.vlans = [ 1 ];
      };
  };

  testScript = ''
    import json
    import os
    import re
    import secrets
    import signal
    import time

    AFTER = ${if after then "True" else "False"}
    BOX = "192.168.1.1"
    PASSWORD = "Correct-Horse-Battery-9"

    ${screens.handbookScript}
    STORE = "/etc/credstore.encrypted"
    RUN = "/run/losos-credentials"

    def decrypt(m, name):
        return m.succeed(f"systemd-creds decrypt --name={name} {STORE}/{name} -")

    def sha(m, path):
        return m.succeed(f"sha256sum {path}").split()[0]

    def digest_of(m, value):
        return m.succeed(f"printf %s '{value}' | sha256sum | cut -d' ' -f1").strip()

    def power_cycle(m):
        """Shut the machine down and start it again on the same TPM state.

        The test VM's swtpm can outlive QEMU's exit, and the next start then
        reaches that old one, whose answer to the start-up is a TPM failure,
        so it is stopped first."""
        m.shutdown()
        pidfile = m.state_dir / f"{m.name}-swtpm" / "pid"
        if pidfile.exists():
            pid = int(pidfile.read_text().strip())
            try:
                os.kill(pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            for _ in range(60):
                if not os.path.exists(f"/proc/{pid}"):
                    break
                time.sleep(1)
        m.start()

    def nowhere_on_disk(m, value, where):
        """No file under `where` holds `value`, after proving grep finds one."""
        m.succeed(f"printf %s '{value}' > /var/lib/credentials-probe")
        m.succeed(f"grep -rqF -- '{value}' /var/lib/credentials-probe")
        m.succeed("rm -f /var/lib/credentials-probe")
        # -r does not follow the links it meets, so a link into /run is skipped.
        m.fail(f"grep -rqF -- '{value}' {where}")

    # ── The box without a TPM, first and alone: it is quick ──────────────
    if AFTER:
        hostbox.start()
        hostbox.wait_for_unit("losos-credentials.service")
        hostbox.wait_for_unit("cred-probe.service")

        with subtest("no TPM: sealed to the host key"):
            hostbox.fail("test -e /dev/tpmrm0")
            assert hostbox.succeed(f"cat {RUN}/key").strip() == "host"
            hostbox.succeed("test -s /var/lib/systemd/credential.secret")
            assert hostbox.succeed(
                "stat -c %a /var/lib/systemd/credential.secret"
            ).strip() == "400", "the host key is not root-only"
            assert hostbox.succeed(f"stat -c %a {STORE}").strip() == "700"
            assert hostbox.succeed(f"stat -c %a {STORE}/probe").strip() == "600"
            # The binding, not just the file: without the key nothing opens.
            hostbox.succeed("mv /var/lib/systemd/credential.secret /root/key-aside")
            hostbox.fail(f"systemd-creds decrypt --name=probe {STORE}/probe -")
            hostbox.succeed("mv /root/key-aside /var/lib/systemd/credential.secret")
            probe = decrypt(hostbox, "probe")
            assert re.fullmatch(r"[0-9a-f]{64}", probe), probe

        with subtest("the name sealed inside is checked"):
            hostbox.succeed(f"cp {STORE}/probe /root/renamed")
            hostbox.fail("systemd-creds decrypt --name=probe-file /root/renamed -")

        with subtest("a service reads its credential, and nothing else can"):
            seen = hostbox.succeed("cat /run/cred-probe/seen").strip()
            assert seen == digest_of(hostbox, probe), "cred-probe did not read the sealed value"
            hostbox.fail(f"runuser -u nobody -- cat {STORE}/probe")
            nowhere_on_disk(hostbox, probe, "/var /etc /root")

        with subtest("a file credential is a link into tmpfs, owned by its reader"):
            p = "/var/secrets/probe-file"
            hostbox.succeed(f"test -L {p}")
            assert hostbox.succeed(f"readlink {p}").strip() == f"{RUN}/probe-file"
            assert hostbox.succeed(f"stat -f -c %T {p}").strip() == "tmpfs"
            assert hostbox.succeed(f"stat -L -c %u:%a {p}").strip() == "1002:400"
            pf = hostbox.succeed(f"cat {p}").strip()
            assert pf == decrypt(hostbox, "probe-file").strip() and len(pf) == 32, pf
            hostbox.fail(f"runuser -u nobody -- cat {p}")
            nowhere_on_disk(hostbox, pf, "/var /etc /root")

        def reseal_from_scratch(name):
            hostbox.succeed("systemctl stop losos-credentials.service")
            hostbox.succeed(f"rm -f {STORE}/{name} {RUN}/{name} /var/secrets/{name}")

        with subtest("the old keyring's blob is adopted, then removed"):
            reseal_from_scratch("probe-file")
            hostbox.succeed("install -d -m 0700 /var/secrets")
            hostbox.succeed("printf 'from-the-old-keyring' > /root/plain")
            hostbox.succeed(
                "systemd-creds --with-key=host encrypt --name=losos-probe-file "
                "/root/plain /var/secrets/probe-file.cred"
            )
            hostbox.succeed("rm -f /root/plain")
            hostbox.succeed("systemctl start losos-credentials.service")
            assert decrypt(hostbox, "probe-file") == "from-the-old-keyring"
            assert hostbox.succeed("cat /var/secrets/probe-file") == "from-the-old-keyring"
            hostbox.fail("test -e /var/secrets/probe-file.cred")
            hostbox.succeed(f"test -e {RUN}/adopted/probe-file")

        with subtest("a plaintext file at the reader's path is adopted, then deleted"):
            reseal_from_scratch("probe-file")
            hostbox.succeed("install -m 0600 /dev/null /var/secrets/probe-file")
            hostbox.succeed("printf 'from-a-plain-file' > /var/secrets/probe-file")
            hostbox.succeed("systemctl start losos-credentials.service")
            assert decrypt(hostbox, "probe-file") == "from-a-plain-file"
            hostbox.succeed("test -L /var/secrets/probe-file")
            assert hostbox.succeed("cat /var/secrets/probe-file") == "from-a-plain-file"
            nowhere_on_disk(hostbox, "from-a-plain-file", "/var /etc /root")
            journal = hostbox.succeed("journalctl -u losos-credentials.service --no-pager")
            assert "deleted the plaintext at /var/secrets/probe-file" in journal, journal

        with subtest("a sealed file that no longer opens is kept, said, and replaced"):
            hostbox.succeed("systemctl stop losos-credentials.service")
            hostbox.succeed(f"printf 'not a credential' > {STORE}/probe")
            hostbox.succeed("systemctl start losos-credentials.service")
            hostbox.succeed(f"ls {STORE}/.unreadable-probe-*")
            fresh = decrypt(hostbox, "probe")
            assert re.fullmatch(r"[0-9a-f]{64}", fresh) and fresh != probe
            hostbox.succeed(f"test -e {RUN}/remade/probe")
            journal = hostbox.succeed("journalctl -u losos-credentials.service --no-pager")
            assert "did not open" in journal, "the failure was silent"
            hostbox.succeed("systemctl restart cred-probe.service")
            assert hostbox.succeed("cat /run/cred-probe/seen").strip() == digest_of(hostbox, fresh)

        with subtest("the values survive a power cycle unchanged"):
            blobs = {n: sha(hostbox, f"{STORE}/{n}") for n in ("probe", "probe-file")}
            hostbox.shutdown()
            hostbox.start()
            hostbox.wait_for_unit("losos-credentials.service")
            hostbox.wait_for_unit("cred-probe.service")
            for n, digest in blobs.items():
                assert sha(hostbox, f"{STORE}/{n}") == digest, f"{n} was sealed again"
            assert decrypt(hostbox, "probe") == fresh
            assert hostbox.succeed("cat /var/secrets/probe-file") == "from-a-plain-file"
            hostbox.fail(f"test -e {RUN}/adopted/probe-file")

        hostbox.shutdown()

    # ── The real control plane, with a TPM ───────────────────────────────
    box.start()
    laptop.start()
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("nginx.service")
    box.wait_for_open_port(443)
    laptop.wait_for_unit("multi-user.target")

    def curl(args):
        return laptop.succeed(f"curl -sk {args}")

    def sign_in(name, password):
        body = json.dumps({"username": name, "password": password})
        head = curl(
            f"-D - -o /dev/null https://{BOX}/auth/api/firstfactor "
            f"-H 'Content-Type: application/json' -d '{body}'"
        )
        m = re.search(r"(?im)^set-cookie: (losos_session=[^;]+)", head)
        assert m, f"no session cookie for {name}: {head}"
        return m.group(1)

    def api(method, path, cookie, body=None):
        data = f"-H 'Content-Type: application/json' -d '{json.dumps(body)}'" if body is not None else ""
        out = curl(
            f"-w '\\n%{{http_code}}' -X {method} https://{BOX}{path} -H 'Cookie: {cookie}' {data}"
        )
        text, code = out.rsplit("\n", 1)
        return int(code), text

    def capture(plan):
        laptop.succeed(
            f"losos-capture --base https://{BOX} --plan /etc/losos-test/plan-{plan}.json "
            f"--out /tmp/screens/{plan} --user matus --password '{PASSWORD}'",
            timeout=1800,
        )

    BOX_MADE = (
        "admin-token",
        "authelia-jwt",
        "authelia-session",
        "authelia-storage",
        "authelia-oidc-hmac",
        "authelia-oidc-jwks",
        "oidc-lososd",
    )
    DIGEST = "/var/secrets/losos-identity/oidc-lososd.digest"

    def matches_digest():
        out = box.succeed(
            "${pkgs.authelia}/bin/authelia crypto hash validate "
            f"--password \"$(systemd-creds decrypt --name=oidc-lososd {STORE}/oidc-lososd -)\" "
            f"-- \"$(cat {DIGEST})\""
        )
        return "matches" in out

    TOKEN = secrets.token_hex(32)
    HMAC = secrets.token_hex(64)
    OLD_CLIENT = secrets.token_hex(32)
    JWT = secrets.token_hex(32)
    SESSION = secrets.token_hex(32)
    STORAGE = secrets.token_hex(32)
    BUCKET_SECRET = "bucket-" + secrets.token_hex(16)

    if AFTER:
        with subtest("a new box makes its own values, sealed to the TPM"):
            box.succeed("test -e /dev/tpmrm0")
            assert box.succeed(f"cat {RUN}/key").strip() == "tpm2"
            for n in BOX_MADE:
                box.succeed(f"test -s {STORE}/{n}")
            assert decrypt(box, "authelia-oidc-jwks").startswith("-----BEGIN PRIVATE KEY-----")
            # lososd's client secret and the digest LosOS Login checks it by
            # are one pair.
            assert matches_digest()
            # tpm2, not host+tpm2: there is no host key to have used.
            box.fail("test -e /var/lib/systemd/credential.secret")
            box.fail("test -e /var/secrets/losos-admin-token")
            # The binding: a process with no TPM device cannot open them,
            # and the same process with one can.
            box.succeed(
                "systemd-run --wait --pipe -q "
                f"systemd-creds decrypt --name=admin-token {STORE}/admin-token -"
            )
            box.fail(
                "systemd-run --wait --pipe -q -p PrivateDevices=yes "
                f"systemd-creds decrypt --name=admin-token {STORE}/admin-token -"
            )

        with subtest("an older build's plaintext secrets, planted"):
            # What a box on the previous release has on its disk: the spare
            # key, LosOS Login's three keys and the bucket's keys, each a
            # plain file.
            box.succeed(
                "systemctl stop lososd.service authelia-losos.service losos-credentials.service"
            )
            box.succeed(f"rm -rf {STORE}/* {RUN}")
            box.succeed("install -d -m 0700 /var/secrets /var/secrets/losos-identity")
            for path, value in (
                ("/var/secrets/losos-admin-token", TOKEN),
                ("/var/secrets/losos-identity/jwt", JWT),
                ("/var/secrets/losos-identity/session", SESSION),
                ("/var/secrets/losos-identity/storage", STORAGE),
                ("/var/secrets/losos-identity/oidc-hmac", HMAC),
                ("/var/secrets/losos-identity/oidc-lososd", OLD_CLIENT),
            ):
                box.succeed(f"install -m 0600 /dev/null {path} && printf %s '{value}' > {path}")
            box.succeed(
                "${pkgs.openssl}/bin/openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 "
                "-out /var/secrets/losos-identity/oidc-jwks.pem"
            )
            jwks = box.succeed("cat /var/secrets/losos-identity/oidc-jwks.pem")
            JWKS_LINE = jwks.splitlines()[1]
            old_digest = box.succeed(f"cat {DIGEST}")
            target = {
                "endpoint": "https://s3.example.org",
                "bucket": "boxes",
                "prefix": "mattbox",
                "region": "",
                "accessKeyId": "AKIDEXAMPLE",
                "secretAccessKey": BUCKET_SECRET,
            }
            box.succeed(
                f"install -m 0600 /dev/null /var/secrets/losos-backup.json && "
                f"printf %s '{json.dumps(target)}' > /var/secrets/losos-backup.json"
            )
            box.succeed(
                "install -m 0600 /dev/null /var/secrets/losos-backup.env && printf '"
                "RESTIC_REPOSITORY=s3:https://s3.example.org/boxes/mattbox\\n"
                "AWS_ACCESS_KEY_ID=AKIDEXAMPLE\\n"
                f"AWS_SECRET_ACCESS_KEY={BUCKET_SECRET}\\n' > /var/secrets/losos-backup.env"
            )
            power_cycle(box)
            box.wait_for_unit("lososd.service")
            box.wait_for_open_port(443)

        with subtest("each value was adopted as it was, and its plaintext deleted"):
            for n, value in (
                ("admin-token", TOKEN),
                ("authelia-jwt", JWT),
                ("authelia-session", SESSION),
                ("authelia-storage", STORAGE),
                ("authelia-oidc-hmac", HMAC),
                ("authelia-oidc-jwks", jwks),
            ):
                assert decrypt(box, n) == value, f"{n} was not adopted"
                box.succeed(f"test -e {RUN}/adopted/{n}")
            # The client secret was adopted and then made again with its
            # digest, so the pair still matches and the old one is gone.
            assert decrypt(box, "oidc-lososd") != OLD_CLIENT
            assert box.succeed(f"cat {DIGEST}") != old_digest
            assert matches_digest()
            for f in (
                "/var/secrets/losos-admin-token",
                "/var/secrets/losos-identity/jwt",
                "/var/secrets/losos-identity/session",
                "/var/secrets/losos-identity/storage",
                "/var/secrets/losos-identity/oidc-hmac",
                "/var/secrets/losos-identity/oidc-jwks.pem",
                "/var/secrets/losos-identity/oidc-lososd",
                "/var/secrets/losos-backup.env",
            ):
                box.fail(f"test -e {f}")
            assert BUCKET_SECRET in decrypt(box, "backup-bucket")
            # lososd took the secret out of the bucket's JSON at start,
            # before it opened its port (the plugins load first, slowly
            # under TCG).
            box.wait_for_open_port(8082)
            box.succeed("test -s /var/secrets/losos-backup.json")
            box.fail(f"grep -qF '{BUCKET_SECRET}' /var/secrets/losos-backup.json")

        with subtest("the spare key it had still opens the API"):
            out = box.succeed(
                f"curl -sf -H 'Authorization: Bearer {TOKEN}' http://127.0.0.1:8082/api/credentials"
            )
            listed = json.loads(out)
            assert listed["key"] == "tpm2", listed
            for v in (TOKEN, JWT, SESSION, STORAGE, BUCKET_SECRET):
                assert v not in out, "a value is in the list"
            # lososd reads it from its own credential directory.
            assert box.succeed("cat /run/credentials/lososd.service/admin-token") == TOKEN
            box.fail("runuser -u nobody -- cat /run/credentials/lososd.service/admin-token")

    with subtest("claim the box and sign in"):
        box.wait_for_open_port(8082)
        claim = json.dumps({"name": "matus", "password": PASSWORD})
        out = curl(
            f"-w '\\n%{{http_code}}' -X POST https://{BOX}/api/setup/claim "
            f"-H 'Host: {BOX}' -H 'Content-Type: application/json' -d '{claim}'"
        )
        body, code = out.rsplit("\n", 1)
        assert code == "200", (code, body)
        box.wait_for_unit("authelia-losos.service")
        box.systemctl("start losos-identity-setup.service")
        box.wait_until_succeeds(f"grep -q \"domain: '{BOX}'\" /run/losos-identity/session.yml")
        box.wait_until_succeeds("systemctl is-active authelia-losos.service")
        box.wait_for_open_port(9091)
        cookie = sign_in("matus", PASSWORD)

    if AFTER:
        with subtest("LosOS Login reads the keys it had, and only it can"):
            d = "/run/credentials/authelia-losos.service"
            assert box.succeed(f"cat {d}/authelia-jwt") == JWT
            assert box.succeed(f"cat {d}/authelia-session") == SESSION
            assert box.succeed(f"cat {d}/authelia-storage") == STORAGE
            assert box.succeed(f"cat {d}/authelia-oidc-hmac") == HMAC
            assert box.succeed(f"cat {d}/authelia-oidc-jwks") == jwks
            assert box.succeed("cat /run/credentials/lososd.service/oidc-lososd") == decrypt(box, "oidc-lososd")
            box.fail(f"runuser -u nobody -- cat {d}/authelia-session")
            box.fail("test -e /var/secrets/losos-identity/jwt")

        with subtest("the list names every credential and holds none of them"):
            code, text = api("GET", "/api/credentials", cookie)
            assert code == 200, (code, text)
            view = json.loads(text)
            rows = {c["name"]: c for c in view["credentials"]}
            for n in BOX_MADE + ("backup-bucket",):
                assert rows[n]["set"], (n, rows[n])
            assert rows["admin-token"]["adopted"] and rows["admin-token"]["source"] == "box"
            assert rows["backup-bucket"]["source"] == "owner"
            assert rows["backup-bucket"]["pane"] == "/settings/backup"
            assert "authelia-losos" in rows["authelia-session"]["services"][0]
            for v in (TOKEN, JWT, SESSION, STORAGE, HMAC, JWKS_LINE, BUCKET_SECRET):
                assert v not in text, "a value is in the list"

        with subtest("the bucket's keys the owner types are sealed, not written"):
            NEW_SECRET = "bucket-" + secrets.token_hex(16)
            code, text = api(
                "POST",
                "/api/backup/target",
                cookie,
                {
                    "endpoint": "https://s3.example.org",
                    "bucket": "boxes",
                    "prefix": "mattbox",
                    "region": "",
                    "accessKeyId": "AKIDEXAMPLE",
                    "secretAccessKey": NEW_SECRET,
                },
            )
            assert code == 200, (code, text)
            assert NEW_SECRET not in text
            assert NEW_SECRET in decrypt(box, "backup-bucket")
            box.fail("test -e /var/secrets/losos-backup.env")
            for v in (TOKEN, JWT, SESSION, STORAGE, HMAC, JWKS_LINE, OLD_CLIENT, BUCKET_SECRET, NEW_SECRET):
                nowhere_on_disk(box, v, "/var /etc /root")
            code, text = api("GET", "/api/backup", cookie)
            assert code == 200 and NEW_SECRET not in text, (code, text)

        with subtest("a new value: made, sealed, and its reader restarted"):
            before_id = box.succeed("systemctl show -P InvocationID authelia-losos.service").strip()
            code, text = api("POST", "/api/credentials/authelia-session/rotate", cookie)
            assert code == 200, (code, text)
            fresh = decrypt(box, "authelia-session")
            assert re.fullmatch(r"[0-9a-f]{128}", fresh) and fresh != SESSION, fresh
            assert fresh not in text
            box.wait_until_succeeds(
                f"test \"$(systemctl show -P InvocationID authelia-losos.service)\" != '{before_id}'"
            )
            box.wait_until_succeeds("systemctl is-active authelia-losos.service")
            box.wait_for_open_port(9091)
            box.wait_until_succeeds(
                f"test \"$(cat /run/credentials/authelia-losos.service/authelia-session)\" = '{fresh}'"
            )
            # A new session key ends every session; signing in again works.
            cookie = sign_in("matus", PASSWORD)
            code, _ = api("GET", "/api/credentials", cookie)
            assert code == 200

        with subtest("the ones a new value would break are refused"):
            for n in ("admin-token", "authelia-storage", "authelia-oidc-jwks", "oidc-lososd", "backup-bucket"):
                code, text = api("POST", f"/api/credentials/{n}/rotate", cookie)
                assert code == 409, (n, code, text)
            assert decrypt(box, "admin-token") == TOKEN
            code, _ = api("POST", "/api/credentials/no-such-thing/rotate", cookie)
            assert code == 404

    with subtest("the pictures"):
        for plan in ["light", "dark"] + (["phone"] if AFTER else []):
            capture(plan)
        laptop.copy_from_machine("/tmp/screens", "")
        if AFTER:
            handbook_pictures(laptop, f"https://{BOX}", "matus", PASSWORD, [
                {"name": "settings-credentials", "path": "/settings/credentials",
                 "waitFor": {"selector": '[data-testid="credential"][data-name="backup-bucket"]'}},
            ])

    if AFTER:
        with subtest("the values survive a power cycle unchanged"):
            names = BOX_MADE + ("backup-bucket",)
            blobs = {n: sha(box, f"{STORE}/{n}") for n in names}
            power_cycle(box)
            box.wait_for_unit("lososd.service")
            box.wait_for_unit("authelia-losos.service")
            # The unit is active before lososd has loaded its plugins and
            # opened its port, a minute or more under TCG.
            box.wait_for_open_port(8082)
            for n, digest in blobs.items():
                assert sha(box, f"{STORE}/{n}") == digest, f"{n} was sealed again"
            assert decrypt(box, "admin-token") == TOKEN
            assert decrypt(box, "authelia-session") == fresh
            assert matches_digest()
            box.succeed(
                f"curl -sf -H 'Authorization: Bearer {TOKEN}' http://127.0.0.1:8082/api/credentials"
            )
  '';
}
