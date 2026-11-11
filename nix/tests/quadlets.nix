# nixos-test-vms config for the Podman quadlets (modules/quadlets.nix).
#
# LosOS Login (Authelia) and the admin web UI run as containers systemd
# starts through quadlet, each a unit of its own — the point of the
# conversion is that a boot times each service apart, so this test reads
# the timing off the units as well as proving the containers work:
#
#   1. the .container files sit in /etc/containers/systemd and the native
#      authelia unit is gone (the generated one took its name),
#   2. the images loaded into Podman's own store,
#   3. the web UI container serves the SPA on its loopback port, deep
#      links included (the SPA fallback),
#   4. Authelia stays down while no people file exists (the same gated
#      start modules/identity.nix gives the native unit), and answers its
#      health endpoint once the file is there — the people file read from
#      inside the container, the secrets through systemd's credential
#      machinery,
#   5. `podman ps` sees both containers, by name,
#   6. and the timing: for each unit, the microseconds from
#      InactiveExitTimestampMonotonic to ActiveEnterTimestampMonotonic,
#      plus `systemd-analyze critical-chain` for both — printed here for
#      every run, and asserted under a generous cap (the container has to
#      come up, not to win a race).
#
# Minimal on purpose: no nginx, no lososd, no identity module — the
# quadlet machinery and the two containers are what is under test. What
# the test states, it states once: the port is losos.identity.port's, the
# paths the settings' own.
{ pkgs }:

let
  # The test's one statement of its numbers and paths: the node's options
  # take them, and the script reads the same.
  autheliaPort = 9091;
  uiPort = 8291;
  autheliaPortStr = toString autheliaPort;
  uiPortStr = toString uiPort;
  peopleFile = "/var/lib/losos-identity/users.yml";
  secretsDir = "/var/secrets/losos-identity";
  secretFile = name: "${secretsDir}/${name}";
  stateDir = "/var/lib/authelia-losos";
in

pkgs.testers.nixosTest {
  name = "losos-quadlets";

  nodes.machine =
    { pkgs, ... }:
    {
      imports = [
        ../../modules/options.nix
        ../../modules/quadlets.nix
      ];

      # The account the containers run as (the appliance declares it in
      # modules/configuration.nix with its own static uid; the test does
      # the same, uid and all, and the conversion reads it from there).
      users.users.notshared = {
        uid = 1000;
        isNormalUser = true;
        group = "notshared";
      };
      users.groups.notshared = {
        gid = 1000;
      };

      lososInternal.quadlets = {
        enable = true;
        uiPort = uiPort;
        # No identity module here: no ordering beyond the image loader, and
        # the same people-file gate the appliance's own conversion carries.
        autheliaUnit.conditionPathExists = peopleFile;
      };

      # A stand-in for the built SPA (nix/packages.nix's losos-admin-ui):
      # the quadlet only has to serve a document root.
      losos.admin = {
        enable = true;
        ui = pkgs.writeTextDir "index.html" "<html><body>quadlet webui</body></html>";
      };

      # Authelia, configured the way modules/identity.nix configures it:
      # secrets as credentials, a file backend it watches, its storage and
      # notifier under /var/lib.
      services.authelia.instances.losos = {
        enable = true;
        settings = {
          server.address = "tcp://127.0.0.1:${toString autheliaPort}";
          log.level = "info";
          ntp.disable_startup_check = true;
          authentication_backend.file = {
            path = peopleFile;
            watch = true;
          };
          access_control.default_policy = "one_factor";
          session.name = "losos_session";
          storage.local.path = "${stateDir}/db.sqlite3";
          notifier.filesystem.filename = "${stateDir}/notifications.txt";
        };
        secrets = {
          manual = true;
          jwtSecretFile = secretFile "jwt";
          sessionSecretFile = secretFile "session";
          storageEncryptionKeyFile = secretFile "storage";
        };
        environmentVariables = {
          AUTHELIA_IDENTITY_VALIDATION_RESET_PASSWORD_JWT_SECRET_FILE = "%d/jwtSecretFile";
          AUTHELIA_SESSION_SECRET_FILE = "%d/sessionSecretFile";
          AUTHELIA_STORAGE_ENCRYPTION_KEY_FILE = "%d/storageEncryptionKeyFile";
        };
      };

      # The secrets, before anything reads them.
      systemd.services.losos-quadlet-secrets = {
        description = "Test secrets for the Authelia quadlet";
        before = [ "authelia-losos.service" ];
        wantedBy = [ "multi-user.target" ];
        serviceConfig.Type = "oneshot";
        script = ''
          mkdir -p ${secretsDir}
          for s in jwt session storage; do
            if [[ ! -s ${secretsDir}/$s ]]; then
              head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n' > ${secretsDir}/$s
            fi
          done
        '';
      };

      virtualisation = {
        memorySize = 2048;
        cores = 4;
      };
    };

  testScript =
    let
      authelia = pkgs.authelia;
    in
    ''
      import json

      machine.start()

      # Podman rootless, as the account the quadlets run under: its store
      # is the one the units get (CONTAINERS_STORAGE_CONF off the unit's
      # own environment).
      conf = machine.succeed(
        "systemctl cat losos-webui.service | grep -o 'CONTAINERS_STORAGE_CONF=[^\"]*'"
      ).strip().split("=", 1)[1]
      podman = f"runuser -u notshared -- env XDG_RUNTIME_DIR=/run/user/1000 CONTAINERS_STORAGE_CONF={conf} podman"

      # 1. The .container files are where the generator reads them, and the
      #    native unit is not in the system at all.
      machine.succeed("test -e /etc/containers/systemd/authelia-losos.container")
      machine.succeed("test -e /etc/containers/systemd/losos-webui.container")
      machine.fail("test -e /etc/systemd/system/authelia-losos.service")

      # 2. The prepare unit ran, and the account's store has both images.
      machine.wait_for_unit("losos-quadlet-prepare.service")
      machine.wait_until_succeeds(f"{podman} image exists localhost/losos-authelia:local")
      machine.wait_until_succeeds(f"{podman} image exists localhost/losos-webui:local")

      # 3. The web UI container: serving the SPA, fallback included.
      machine.wait_for_unit("losos-webui.service")
      machine.wait_until_succeeds("curl -fsS http://127.0.0.1:${uiPortStr}/ | grep -q 'quadlet webui'")
      # A deep link, which only the SPA fallback can answer:
      machine.succeed("curl -fsS http://127.0.0.1:${uiPortStr}/settings/network | grep -q 'quadlet webui'")

      # 4. Authelia waits for its people file: the unit is down (the
      #    condition holds it) and the port answers nothing.
      machine.succeed("systemctl is-inactive authelia-losos.service")
      machine.fail("curl -fsS --max-time 2 http://127.0.0.1:${autheliaPortStr}/api/health")

      # The first person, with a password hash Authelia itself made.
      digest = machine.succeed(
        "${authelia}/bin/authelia crypto hash generate pbkdf2 --variant sha512 --password=the-first-password | sed -n 's/^Digest: //p'"
      ).strip()
      assert digest.startswith("$argon2id$"), f"no argon2id digest: {digest!r}"
      machine.succeed(
        "mkdir -p /var/lib/losos-identity && printf 'users:\\n  ada:\\n    displayname: Ada\\n    password: %s\\n    groups:\\n      - losos-admins\\n' '%s' > /var/lib/losos-identity/users.yml"
        % (digest, digest)
      )
      machine.succeed("systemctl start authelia-losos.service")
      machine.wait_for_unit("authelia-losos.service")
      machine.wait_until_succeeds("curl -fsS http://127.0.0.1:${autheliaPortStr}/api/health")

      # 5. Podman runs both, under the names the quadlets set — in the
      #    account's own store.
      names = machine.succeed(f"{podman} ps --format '{{{{.Names}}}}'")
      assert "losos-authelia" in names, f"authelia container missing: {names!r}"
      assert "losos-webui" in names, f"webui container missing: {names!r}"

      # 6. The timing, the thing the conversion is for: each unit's start
      #    read apart, from the unit's own timestamps.
      def startup_ms(unit):
          out = machine.succeed(
            f"systemctl show {unit} -p InactiveExitTimestampMonotonic --value -p ActiveEnterTimestampMonotonic --value"
          )
          t = [int(x) for x in out.split()]
          return (t[1] - t[0]) / 1000.0

      timings = {}
      for unit in ("authelia-losos.service", "losos-webui.service"):
          timings[unit] = startup_ms(unit)
          print(f"STARTUP {unit}: {timings[unit]:.0f} ms")
      machine.log(machine.succeed("systemd-analyze critical-chain authelia-losos.service losos-webui.service"))

      # A generous cap: the container has to come up, not win a race.
      assert timings["authelia-losos.service"] < 120_000, timings
      assert timings["losos-webui.service"] < 60_000, timings

      # A restart is a start of its own, timed apart again: the web UI
      # restarts and is back inside its unit, measured as before.
      machine.succeed("systemctl restart losos-webui.service")
      machine.wait_for_unit("losos-webui.service")
      machine.wait_until_succeeds("curl -fsS http://127.0.0.1:${uiPortStr}/ | grep -q 'quadlet webui'")
      again = startup_ms("losos-webui.service")
      print(f"STARTUP losos-webui.service (restart): {again:.0f} ms")

      json.dump(timings, open("quadlet-startup-ms.json", "w"), indent=2)
    '';
}
