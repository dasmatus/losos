# The OCI images the LOCAL k3s cluster runs.
#
# Two of them: a sandbox (pause) image and Forgejo. They are
# preloaded through the cluster's `images` option, which symlinks each tarball
# into /var/lib/rancher/k3s/agent/images for the agent to import at start, so
# the pods run with `imagePullPolicy: Never` and nothing is ever fetched from a
# container registry. The image names below are under `losos.local/` for the
# same reason: if somebody ever flips that policy, the pull fails on a name
# that cannot resolve instead of quietly reaching Docker Hub.
#
# What this does relax, and it must be said plainly rather than left implied:
# the images are NOT built on the appliance. An image is hundreds of MiB and
# system.autoUpgrade rebuilds at 03:00 on a repurposed mini-PC with a tmpfs
# root, three hours before an unconditional reboot. They come from this
# project's own Nix binary cache instead. Nothing is pulled from a *container
# registry*; the box does substitute from a Nix cache it trusts, and a cache
# that is unreachable at 03:00 means the rebuild fails and the previous
# generation keeps running.
#
# None of these images know anything that varies per box. Forgejo's ROOT_URL
# and the owner's site settings are rendered by modules/workloads.nix in
# module context and hostPath-mounted at /etc/losos/forgejo/losos.ini.
# Changing one changes the store path, changes the manifest, and the kubelet
# restarts the pod — no image rebuild ever.
#
# The pods run with hostNetwork (the local cluster has no CNI), which is why
# every listener these images configure is bound explicitly to loopback by the
# mounted config: nginx stays the only process on the box holding a public
# port.
#
# The image tags are left to dockerTools, which derives them from the output
# hash. That is deliberate: a hand-written tag like "34.0.2" would stay put
# across a content change, the manifest string would not change, and the
# kubelet — told never to pull — would happily keep running the previous image
# forever. A hash tag makes every content change a new manifest and therefore a
# restart.
{ pkgs }:

let
  inherit (pkgs) lib;

  # Every image gets these. `/bin/sh` comes from bash and is what Forgejo's
  # generated git hooks expect to exist; coreutils is there
  # for the entrypoints and for `crictl exec` on a box with no shell of its
  # own, which is the only way anyone will ever debug one of these pods.
  baseTools = [
    pkgs.bashInteractive
    pkgs.coreutils
  ];

  # dockerTools does not create /tmp or /usr/bin, and both are load-bearing:
  # programs put temp files in /tmp, and Forgejo writes git
  # hooks that start with `#!/usr/bin/env bash`. A missing /usr/bin/env is a
  # push that fails at the hook, long after anyone is still watching the
  # deployment.
  commonLayout = ''
    mkdir -p tmp usr/bin
    chmod 1777 tmp
    ln -s ${pkgs.coreutils}/bin/env usr/bin/env
  '';

  # ── Forgejo ───────────────────────────────────────────────────────────────

  forgejoStateDir = "/var/lib/forgejo";
  forgejoCustomDir = "${forgejoStateDir}/custom";

  # The LosOS look (admin-ui/themes/). The same import modules/services.nix
  # makes for native mode, so both modes ship one set of files.
  themes = import ../admin-ui/themes { inherit pkgs; };

  # Everything about Forgejo that does not vary per box. The mounted
  # losos.ini — HTTP_ADDR, HTTP_PORT, ROOT_URL, DISABLE_REGISTRATION,
  # INSTALL_LOCK, actions.ENABLED — is concatenated *after* this at start, and
  # go-ini merges repeated section headers with the last value for a key
  # winning, so the per-box file overrides anything here.
  #
  # The database mirrors what modules/services.nix provisions natively: one
  # Postgres, peer auth over the socket, hence the pinned forgejo uid in
  # modules/configuration.nix.
  forgejoBaseIni = pkgs.writeText "forgejo-base.ini" ''
    [DEFAULT]
    RUN_MODE = prod
    RUN_USER = forgejo
    WORK_PATH = ${forgejoStateDir}

    [database]
    DB_TYPE = postgres
    HOST = /run/postgresql
    NAME = forgejo
    USER = forgejo
    SSL_MODE = disable

    [repository]
    ROOT = ${forgejoStateDir}/repositories

    [server]
    APP_DATA_PATH = ${forgejoStateDir}/data
    LFS_START_SERVER = true
    ; This appliance has no SSH at all — not for humans, not for git. The
    ; published route is HTTPS through the front vhost and nothing else.
    START_SSH_SERVER = false
    DISABLE_SSH = true

    [lfs]
    PATH = ${forgejoStateDir}/data/lfs

    [session]
    COOKIE_NAME = session

    [security]
    INSTALL_LOCK = true

    [log]
    MODE = console
    LEVEL = Info

  '';

  # LosOS Git's name, theme, <meta> tags and footer (admin-ui/themes/), in
  # the shape modules/services.nix merges into services.forgejo.settings
  # natively. Concatenated between the base and the per-box file.
  forgejoBrandIni = pkgs.writeText "forgejo-brand.ini" (
    lib.generators.toINI { } themes.forgejo.settings
  );

  forgejoEntrypoint = pkgs.writeShellApplication {
    name = "losos-forgejo";
    runtimeInputs = [
      pkgs.coreutils
      pkgs.forgejo
      pkgs.git
      pkgs.gnupg
    ];
    text = ''
      # PID 1 of the Forgejo pod. The shape follows nixpkgs'
      # services.forgejo preStart, because the problem is the same one: an
      # app.ini that is regenerated from the store on every start, plus a
      # handful of secrets that must survive that regeneration or every
      # session, LFS token and OAuth2 grant on the box is invalidated on
      # reboot.

      umask 0027

      conf=/etc/losos/forgejo/losos.ini

      fail() {
        echo "losos-forgejo: $*" >&2
        exit 1
      }

      [ -r "$conf" ] || fail "$conf is missing or unreadable. It is rendered and hostPath-mounted by modules/workloads.nix."
      [ -d ${forgejoStateDir} ] || fail "${forgejoStateDir} does not exist. It is a hostPath mount; the host creates it via systemd-tmpfiles."
      [ -w ${forgejoStateDir} ] || fail "${forgejoStateDir} is not writable by uid $(id -u). The host must create it owned by the forgejo uid — see the tmpfiles rules in modules/workloads.nix."

      export FORGEJO_WORK_DIR=${forgejoStateDir}
      export FORGEJO_CUSTOM=${forgejoCustomDir}
      export HOME=${forgejoStateDir}
      export USER=forgejo

      mkdir -p ${forgejoCustomDir}/conf ${forgejoCustomDir}/public/assets ${forgejoStateDir}/data ${forgejoStateDir}/repositories

      # Replace each LosOS-managed tree instead of merging into the persistent
      # custom directory, so removed or renamed assets cannot survive upgrades.
      # Keep conf/ intact because its secrets persist across restarts. Copy
      # rather than link because the store is not mounted in the pod.
      for tree in public/assets/css public/assets/img templates; do
        rm -rf "${forgejoCustomDir}/$tree"
        cp -rT --no-preserve=mode,ownership "${themes.forgejo.customDir}/$tree" "${forgejoCustomDir}/$tree"
      done

      # Generated once and kept. `forgejo generate secret` is the same
      # generator the NixOS module uses, and the __FILE environment variables
      # below are the documented way to get a secret into app.ini without it
      # ever being written into the store.
      for secret in SECRET_KEY INTERNAL_TOKEN JWT_SECRET LFS_JWT_SECRET; do
        file=${forgejoCustomDir}/conf/$secret
        if [ ! -s "$file" ]; then
          (
            umask 0077
            forgejo generate secret "$secret" > "$file"
          )
        fi
      done

      config=${forgejoCustomDir}/conf/app.ini
      cat ${forgejoBaseIni} ${forgejoBrandIni} "$conf" > "$config"

      FORGEJO__security__SECRET_KEY__FILE=${forgejoCustomDir}/conf/SECRET_KEY
      FORGEJO__security__INTERNAL_TOKEN__FILE=${forgejoCustomDir}/conf/INTERNAL_TOKEN
      FORGEJO__oauth2__JWT_SECRET__FILE=${forgejoCustomDir}/conf/JWT_SECRET
      FORGEJO__server__LFS_JWT_SECRET__FILE=${forgejoCustomDir}/conf/LFS_JWT_SECRET
      export FORGEJO__security__SECRET_KEY__FILE
      export FORGEJO__security__INTERNAL_TOKEN__FILE
      export FORGEJO__oauth2__JWT_SECRET__FILE
      export FORGEJO__server__LFS_JWT_SECRET__FILE

      environment-to-ini --config "$config"
      chmod 0600 "$config"

      # Both are idempotent and both are needed after an upgrade: `migrate`
      # brings the schema forward, `regenerate hooks` rewrites the store paths
      # baked into every repository's git hooks, which change on every rebuild.
      # Forgejo 16 no longer has the `hooks` subcommand (`admin regenerate`
      # lists only `keys`; it prints its usage and exits 0 when asked for
      # hooks, which is what the pod log showed), so ask before
      # calling: on a version that has it the hooks are rewritten, on one that
      # does not the hooks are Forgejo's own business.
      forgejo migrate
      if forgejo admin regenerate --help 2>/dev/null | grep -qE '^ +hooks '; then
        forgejo admin regenerate hooks
      fi

      ${import ./forgejo-bootstrap.nix {
        tokenFile = "${forgejoStateDir}/.losos-token";
      }}

      exec forgejo web
    '';
  };

  # ── Pause ─────────────────────────────────────────────────────────────────

  # The sandbox image every pod on the local cluster is built around: PID 1 of
  # the pod's namespaces, whose whole job is to hold them open and reap
  # whatever gets reparented onto it. tini + `sleep inf` is what nixpkgs' own
  # nixos/tests/rancher/single-node.nix passes to --pause-image, and this is
  # the same use.
  pauseEnv = pkgs.buildEnv {
    name = "losos-image-pause-root";
    paths = [
      pkgs.tini
      pkgs.coreutils
    ];
  };
in
{
  losos-image-pause = pkgs.dockerTools.buildLayeredImage {
    name = "losos.local/pause";
    compressor = "zstd";
    contents = pauseEnv;
    config.Entrypoint = [
      "/bin/tini"
      "-g"
      "--"
      "/bin/sleep"
      "inf"
    ];
  };

  losos-image-forgejo = pkgs.dockerTools.buildLayeredImage {
    name = "losos.local/forgejo";
    compressor = "zstd";
    contents = pkgs.buildEnv {
      name = "losos-image-forgejo-root";
      paths = baseTools ++ [
        pkgs.tini
        forgejoEntrypoint
        pkgs.forgejo
        pkgs.git
      ];
    };
    extraCommands = commonLayout;
    config = {
      Entrypoint = [
        "/bin/tini"
        "-g"
        "--"
        "/bin/losos-forgejo"
      ];
      Env = [
        "PATH=/bin"
        "LANG=C.UTF-8"
      ];
      WorkingDir = forgejoStateDir;
    };
  };
}
