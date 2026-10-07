# LosOS Git's Actions runner, on the box itself.
#
# Why a runner at all: the official-edge key ceremony (wiki/Master-Proxy.md,
# "Official edges") is meant to run *from the owner's box*, as Forgejo Actions
# workflows in a provisioning repository on LosOS Git
# (provisioning/edge-identity/). The root private key is then made on the box,
# lives only as an Actions secret there, and leaves it only as signed
# certificates deployed over SSH. Something has to execute those workflows,
# and the box has no shell for a person to do it by hand.
#
# Shape:
#   * host mode (label `losos:host`): jobs run as the unprivileged system user
#     `losos-git-runner` on the box, with a fixed PATH (bash, git, ssh, curl,
#     jq, tar, node, the registrar binary). There is no container runtime —
#     the box runs no Docker — so a `docker://` label would have nothing to
#     run in.
#   * registered with a shared secret, not a registration token:
#     `forgejo forgejo-cli actions register --secret-file` is idempotent and
#     talks to Forgejo's database directly, so every boot re-asserts the same
#     runner row (its UUID is derived from the secret) instead of collecting a
#     new runner per boot the way a one-shot token on a tmpfs root would. The
#     secret is 40 hex characters generated on first start into
#     losos.forgejo.runner.secretFile (0600 root, persisted under /var); the
#     runner reads it as a systemd credential, never from argv.
#   * works in both Forgejo modes. Native: services.forgejo on :8888. Container:
#     the pod on :3000 uses the host's Postgres over /run/postgresql and writes
#     its app.ini under /var/lib/forgejo/custom on the host filesystem (a
#     hostPath mount, flake/images.nix), so the host can run the same forgejo
#     binary (pkgs.forgejo, which is also what the image carries), as the same
#     `forgejo` user, against the same config — peer auth included. The
#     registration waits for that app.ini to appear (a first boot pulls the
#     image first) and the unit retries until it does.
#
# Trust: Actions is remote code execution by design. Whoever can push a
# workflow to a repository on LosOS Git can run code on the box as
# losos-git-runner — and since registration is closed
# (service.DISABLE_REGISTRATION in both modes), that is the accounts the owner
# created. The runner user owns nothing but its state directory; the admin
# token, both data homes and /var/secrets are out of its reach (ProtectHome,
# ProtectSystem=strict, the secret handed in as a 0400 credential). Turn it off
# with losos.forgejo.runner.enable = false, which also switches Actions off in
# the container pod (modules/workloads.nix follows the same option).
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.losos.forgejo.runner;
  forgejo = config.losos.forgejo;
  enabled = cfg.enable && forgejo.enable;
  native = forgejo.mode == "native";

  # Forgejo's loopback port per mode. 3000 is the same literal
  # modules/containers.nix and modules/workloads.nix carry — never an option,
  # containers.nix explains why.
  port = if native then config.services.forgejo.settings.server.HTTP_PORT else 3000;
  instance = "http://127.0.0.1:${toString port}";

  # Forgejo's work and custom directories, the same in both modes:
  # services.forgejo's defaults, and what the pod's entrypoint exports
  # (flake/images.nix). The `forgejo` user is modules/configuration.nix's
  # (uid 1003) in container mode and services.forgejo's own in native mode —
  # the same name either way, which is what Postgres' peer auth keys on.
  workDir = if native then config.services.forgejo.stateDir else "/var/lib/forgejo";
  customDir = if native then config.services.forgejo.customDir else "${workDir}/custom";
  forgejoUser = if native then config.services.forgejo.user else "forgejo";
  forgejoGroup = if native then config.services.forgejo.group else "forgejo";
  forgejoBin = lib.getExe' (
    if native then config.services.forgejo.package else pkgs.forgejo
  ) "forgejo";

  user = "losos-git-runner";
  stateDir = "/var/lib/${user}";
  registrar = config.losos.proxy.registrar.package;

  yaml = pkgs.formats.yaml { };
  # `@UUID@` is filled in at start from the secret. forgejo-cli derives the
  # runner's UUID from the secret's first 16 characters; the runner side has to
  # present the same one, which is exactly what the deprecated
  # `create-runner-file` used to compute — `server.connections` is its
  # replacement, and takes the token through a credential file.
  configTemplate = yaml.generate "losos-git-runner.yaml" {
    log = {
      level = "info";
      job_level = "info";
    };
    runner = {
      capacity = 1;
      timeout = "1h";
      shutdown_timeout = "5m";
      fetch_interval = "5s";
      labels = [ "losos:host" ];
      # Lets a workflow clone its own repository over loopback
      # ("$LOSOS_FORGEJO_URL/$GITHUB_REPOSITORY.git") instead of going out
      # through nginx and the box's mDNS name, which a job on the box itself
      # may not resolve.
      envs.LOSOS_FORGEJO_URL = instance;
    };
    cache.enabled = false;
    host.workdir_parent = "${stateDir}/work";
    server.connections.losos = {
      url = instance;
      uuid = "@UUID@";
      token_url = "file:$CREDENTIALS_DIRECTORY/secret";
    };
  };

  # Its own unit, ordered before the runner, because systemd sets up
  # LoadCredential= before it spawns ExecStartPre: a secret generated there
  # comes one step too late and the service dies at "Failed to set up
  # credentials" forever (seen in tests/git-runner.nix).
  makeSecret = pkgs.writeShellScript "losos-git-runner-secret" ''
    set -euo pipefail
    secret=${lib.escapeShellArg cfg.secretFile}
    if [ ! -s "$secret" ]; then
      umask 077
      install -d -m 0700 "$(dirname "$secret")"
      od -An -N20 -tx1 /dev/urandom | tr -d ' \n' > "$secret.tmp"
      mv "$secret.tmp" "$secret"
    fi
  '';

  # Runs as root (ExecStartPre's `+` prefix), before the daemon: asserts the
  # runner on Forgejo's side and writes the daemon's config with the matching
  # UUID into the state directory.
  register = pkgs.writeShellScript "losos-git-runner-register" ''
    set -euo pipefail
    secret=${lib.escapeShellArg cfg.secretFile}

    # Forgejo's own configuration must exist before anything can be registered
    # against its database. Native mode writes it in forgejo.service's preStart
    # (ordered before us); the pod writes it on its first start, which on a
    # first boot comes after the image is imported. Wait a little, then let
    # Restart= try again rather than holding the unit's start forever.
    ini=${lib.escapeShellArg "${customDir}/conf/app.ini"}
    for _ in $(seq 12); do
      [ -r "$ini" ] && break
      sleep 5
    done
    if [ ! -r "$ini" ]; then
      echo "losos-git-runner: $ini is not there yet (Forgejo has not started); retrying later" >&2
      exit 1
    fi

    # A 0400 copy the forgejo user can read, in a root-made tempdir, for the
    # one command that needs it. The secret itself stays root-only.
    tmp=$(mktemp -d /run/losos-git-runner-register.XXXXXX)
    trap 'rm -rf "$tmp"' EXIT
    install -m 0400 -o ${forgejoUser} -g ${forgejoGroup} "$secret" "$tmp/secret"
    chown ${forgejoUser}:${forgejoGroup} "$tmp"
    chmod 0700 "$tmp"
    ${lib.getExe' pkgs.util-linux "setpriv"} --reuid=${forgejoUser} --regid=${forgejoGroup} --init-groups \
      env FORGEJO_WORK_DIR=${lib.escapeShellArg workDir} FORGEJO_CUSTOM=${lib.escapeShellArg customDir} \
          HOME=${lib.escapeShellArg workDir} USER=${forgejoUser} \
      ${forgejoBin} forgejo-cli actions register \
        --secret-file "$tmp/secret" --name ${lib.escapeShellArg cfg.name} --labels losos

    # forgejo-cli takes the first 16 *characters* of the secret as the UUID's
    # bytes (the hex text itself, not its decoding): "0123…" becomes
    # 30313233-3435-…, which is what create-runner-file computed as well.
    h=$(printf '%s' "$(head -c 16 "$secret")" | od -An -tx1 | tr -d ' \n')
    uuid="''${h:0:8}-''${h:8:4}-''${h:12:4}-''${h:16:4}-''${h:20:12}"
    sed "s/@UUID@/$uuid/" ${configTemplate} > "$STATE_DIRECTORY/config.yaml.tmp"
    chown ${user}:${user} "$STATE_DIRECTORY/config.yaml.tmp"
    chmod 0644 "$STATE_DIRECTORY/config.yaml.tmp"
    mv "$STATE_DIRECTORY/config.yaml.tmp" "$STATE_DIRECTORY/config.yaml"
  '';
in
{
  config = lib.mkIf enabled {
    users.groups.${user} = { };
    users.users.${user} = {
      isSystemUser = true;
      group = user;
      home = stateDir;
      description = "LosOS Git Actions runner";
    };

    systemd.services.losos-git-runner-secret = {
      description = "LosOS Git Actions runner: shared registration secret";
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        ExecStart = makeSecret;
      };
    };

    systemd.services.losos-git-runner = {
      description = "LosOS Git Actions runner (host mode, on the box)";
      wantedBy = [ "multi-user.target" ];
      wants = [ "network-online.target" ];
      requires = [ "losos-git-runner-secret.service" ];
      after = [
        "network-online.target"
        "postgresql.service"
        "losos-git-runner-secret.service"
      ]
      ++ lib.optional native "forgejo.service"
      ++ lib.optional (!native) "k3s.service";
      # What a `run:` step finds on PATH. A `uses:` step needs node; the rest
      # is what provisioning/edge-identity/ needs, plus the registrar for the
      # key ceremony itself.
      path =
        with pkgs;
        [
          bash
          coreutils
          curl
          findutils
          gawk
          gitMinimal
          gnugrep
          gnused
          gnutar
          gzip
          jq
          nodejs
          openssh
          util-linux
        ]
        ++ lib.optional (registrar != null) registrar;
      environment.HOME = stateDir;
      unitConfig.StartLimitIntervalSec = 0;
      serviceConfig = {
        User = user;
        Group = user;
        StateDirectory = user;
        StateDirectoryMode = "0700";
        WorkingDirectory = stateDir;
        LoadCredential = "secret:${cfg.secretFile}";
        ExecStartPre = "+${register}";
        ExecStart = "${lib.getExe pkgs.forgejo-runner} daemon --config ${stateDir}/config.yaml";
        # Forgejo is restarted mid-rebuild like everything else, and on a
        # first boot its config is not there yet (see `register`).
        Restart = "on-failure";
        RestartSec = "30s";

        NoNewPrivileges = true;
        PrivateTmp = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectControlGroups = true;
        ProtectClock = true;
        ProtectHostname = true;
        RestrictSUIDSGID = true;
        RestrictRealtime = true;
        RestrictNamespaces = true;
        LockPersonality = true;
        UMask = "0077";
      };
    };
  };
}
