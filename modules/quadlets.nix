# The box's own services as Podman quadlets.
#
# Two services move into containers here, run by systemd through quadlet
# (a `.container` file in /etc/containers/systemd that the quadlet generator
# turns into a unit of its own):
#
#   authelia-<name>   every enabled Authelia instance — on the appliance,
#                     LosOS Login (modules/identity.nix), the box's own
#                     Authelia. Same binary, same rendered configuration,
#                     same socket, same secrets — the process runs in a
#                     Podman container on the host network instead of
#                     straight on the host, so nginx (auth_request), lososd
#                     (LOSOS_LOGIN_ADDR) and the file watcher notice no
#                     difference.
#   losos-webui       the admin pages' SPA (losos.admin.ui). nginx keeps the
#                     front door and its guards (the LAN rule, the
#                     sub_filter for the no-JavaScript sign-in, the
#                     server-level headers); the document root and the SPA
#                     fallback move into a small nginx container on
#                     loopback, which the front vhost proxies to.
#
# ROOTLESS, AS ONE OF THE BOX'S OWN ACCOUNTS. The containers run under the
# side account (lososInternal.quadlets.user): `notshared`, the owner's
# account, by default — `shared` or `kindashared` on a box that lends
# itself to a mesh or to its local gateways, the same three accounts
# lososInternal.sideUser picks between. Each account has its own sub-id
# range and its own Podman store, so the variants never share an image or
# a container; keep-id maps the account's own uid into the namespace, and
# every unit runs as the account (its runtime directory from linger's user
# manager, the wrappers on PATH). What the container must read, the
# modules that own it make readable by that account: the identity module's
# people file and session redis follow the account (their group is the
# account's, in quadlet mode), the secrets ride the unit's own
# LoadCredential lines, and the state directories are the account's
# (losos-quadlet-prepare makes them so).
#
# DAEMONLESS, as Podman is: no `podman system service` and no session bus
# — every unit runs `podman run` itself, conmon is one process per
# container, and the unit is the cgroup.
#
# WHY QUADLETS. A quadlet unit is a unit of its own: `systemd-analyze
# critical-chain`, the journal, `systemctl show` and a restart speak of one
# container each, where a `podman-compose up` or a big service hides several
# behind one unit with timing of the whole. Startup timing per service is
# then a matter of reading InactiveExit and ActiveEnter off the unit (the
# nix/tests/quadlets.nix test does exactly that), and updating one of them
# is loading a new image and restarting its unit, without touching the
# others or the NixOS generation beneath.
#
# WHAT IS DERIVED, NOT ASSUMED. Every generated unit keeps the name the
# NixOS Authelia module gives the native one (the option's `name`, which
# defaults to the instance's attribute), the user and group are the
# module's own, and the port, the socket, the people file, the storage, the
# notifier and the portal assets all come out of the instance's `settings`,
# as do the secret files and the `..._FILE` variables — nothing here
# repeats a value the configuration already states. The unit ordering
# identity.nix gives the native unit arrives through the `autheliaUnit`
# option below, so it is written in one place.
#
# What stays on the host: nginx itself (the front door holds sockets and
# guards no container should own), redis (Authelia's session store, a
# socket only its group may open, mounted into the container), and the
# credential machinery of modules/credentials.nix: the generated unit keeps
# the native unit's name, so the store's LoadCredential lines and every
# `systemctl try-restart <unit>` elsewhere in this tree keep working
# unchanged.
#
# The conversion is opt-in (lososInternal.quadlets.enable): the appliance
# defaults to the native services until the VM test has held a boot of this.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.lososInternal.quadlets;
  yaml = pkgs.formats.yaml { };

  # The credential directory systemd opens for a generated unit's
  # LoadCredential lines. The native unit's `%d` expands to the same
  # place, so the `..._FILE=%d/...` variables work verbatim once `%d` is
  # written out.
  credDirOf = unit: "/run/credentials/${unit}.service";

  # One enabled Authelia instance, as the conversion runs it: the unit
  # name is the module's own rule (`authelia`, `-<name>` when the instance
  # has a name, which defaults to the instance's attribute), the user and
  # group the module made, the rendered configuration and the paths out of
  # its settings. An instance that configures nothing the container needs
  # to see simply contributes fewer mounts.
  instancesOf =
    { key, instance }:
    let
      # The module names the unit after `name`, which defaults to the
      # instance's attribute; the empty name is the bare `authelia`.
      name = instance.name;
      unit = "authelia" + lib.optionalString (name != "") "-${name}";
      credDir = credDirOf unit;
      configFile = yaml.generate "${unit}-config.yml" instance.settings;
      configFiles = [ configFile ] ++ instance.settingsFiles;

      # The Authelia module's secret options, with the `..._FILE` variable
      # each one becomes: what the native unit's environment and
      # LoadCredential lines are, the generated unit's are too.
      secretOpts = {
        jwtSecretFile = "AUTHELIA_IDENTITY_VALIDATION_RESET_PASSWORD_JWT_SECRET_FILE";
        storageEncryptionKeyFile = "AUTHELIA_STORAGE_ENCRYPTION_KEY_FILE";
        sessionSecretFile = "AUTHELIA_SESSION_SECRET_FILE";
        oidcHmacSecretFile = "AUTHELIA_IDENTITY_PROVIDERS_OIDC_HMAC_SECRET_FILE";
      };
      setSecretOpts = lib.filter (opt: instance.secrets.${opt} != null) (lib.attrNames secretOpts);

      # The `..._FILE` variables name `%d/...` on the native unit; the
      # container knows the directory by its full name. The module's own
      # secret options (`secrets.*_FILE`) become variables the same way it
      # makes them variables of the native unit, by option name — the name
      # the file carries in the credential directory.
      env = lib.mapAttrs (_: v: lib.replaceStrings [ "%d" ] [ credDir ] v) (
        instance.environmentVariables
        // (lib.listToAttrs (
          map (opt: lib.nameValuePair secretOpts.${opt} "%d/${opt}") setSecretOpts
        ))
      );

      # The same secrets as the LoadCredential lines the module puts on
      # the native unit — plus whatever the module that configured the
      # instance (the identity module) hands over through the option
      # below.
      loadCredentials =
        cfg.autheliaUnit.loadCredentials
        ++ map (opt: "${opt}:${instance.secrets.${opt}}") setSecretOpts;

      # Paths from the settings the container must see at the same path.
      # Directories of files it reads go in read-only, its own storage and
      # the notification file read-write, the redis socket and the
      # credential directory as they are. Nothing here mounts a path that
      # does not exist at the time the unit starts: the unit orders itself
      # after what makes them (the `autheliaUnit` option's ordering
      # below).
      dirOfPath = p: lib.optionalString (p != null && lib.hasPrefix "/" p) (lib.dirOf p);
      maybeVolume = p: mode: lib.optionalString (p != null) "${p}:${p}${mode}";
      volumes =
        let
          s = instance.settings;
          redis = s.session.redis.host or null;
        in
        lib.unique (
          lib.filter (v: v != "") [
            (maybeVolume (s.server.asset_path or null) ":ro")
            (maybeVolume (dirOfPath (s.authentication_backend.file.path or null)) ":ro")
            (maybeVolume (dirOfPath (s.storage.local.path or null)) "")
            (maybeVolume (dirOfPath (s.notifier.filesystem.filename or null)) "")
            (lib.optionalString (redis != null && lib.hasPrefix "/" redis)
              "${dirOfPath redis}:${dirOfPath redis}")
            (maybeVolume credDir ":ro")
            # The rendered configuration: the generated YAML, then every
            # settingsFile beside it (the session domains, the OIDC
            # signing key's template).
            (lib.concatStringsSep "\n" (map (f: "${f}:${f}:ro") configFiles))
          ]
        );

      imageRef = "localhost/${unit}:local";
      # The instance's image: the package as the native unit ran it, theme
      # and all, as a container image the loader puts in Podman's store.
      image = pkgs.dockerTools.buildLayeredImage {
        name = "localhost/${unit}";
        tag = "local";
        contents = [ instance.package ];
        config.Entrypoint = [ "/bin/authelia" ];
      };
      # What the container writes (its storage, its notifier file): the
      # directories the prepare unit makes the side account's own, since
      # the container runs as that account and the native unit's
      # StateDirectory (Authelia's own user) does not exist here.
      rwDirs = lib.unique (
        lib.filter (p: p != "" && p != null) [
          (dirOfPath (instance.settings.storage.local.path or null))
          (dirOfPath (instance.settings.notifier.filesystem.filename or null))
        ]
      );
      stateDirs = lib.concatStringsSep " " rwDirs;
    in
    {
      inherit instance unit credDir env loadCredentials volumes configFile configFiles imageRef image stateDirs;
      container = pkgs.writeText "${unit}.container" ''
        # Generated by modules/quadlets.nix: ${unit}, in a container — the
        # native unit's process, the same configuration, on the host
        # network. Rootless Podman as ${cfg.user}: keep-id maps the
        # account's own uid into the namespace, so the process runs as
        # ${cfg.user} (uid ${toString sideUid}) on the host and in the
        # container alike, and the unit runs as the account.
        [Container]
        Image=${imageRef}
        ContainerName=${unit}
        Network=host
        UserNS=keep-id
        User=${toString sideUid}
        NoNewPrivileges=true
        ${lib.concatMapStringsSep "\n" (v: "Volume=${v}") volumes}
        ${lib.concatMapStringsSep "\n" (k: "Environment=${k}=${env.${k}}") (lib.attrNames env)}
        Environment=CREDENTIALS_DIRECTORY=${credDir}
        Exec=--config=${lib.concatStringsSep "," configFiles}

        [Unit]
        Description=Authelia (${unit}), in a Podman container
        After=losos-quadlet-prepare.service user@${toString sideUid}.service ${lib.concatStringsSep " " cfg.autheliaUnit.after}
        Requires=losos-quadlet-prepare.service ${lib.concatStringsSep " " cfg.autheliaUnit.requires}
        ${lib.optionalString (cfg.autheliaUnit.wants != [ ]) "Wants=${lib.concatStringsSep " " cfg.autheliaUnit.wants}"}
        ${lib.optionalString (cfg.autheliaUnit.conditionPathExists != null) "ConditionPathExists=${cfg.autheliaUnit.conditionPathExists}"}

        [Service]
        # The account the container runs as, rootless: the store, the
        # runtime directory and the wrappers on PATH.
        User=${cfg.user}
        Group=${cfg.user}
        ${serviceEnv}
        # The same keep-alive the native unit had.
        Restart=always
        RestartSec=5s
        # The secrets, through the same credential machinery as before: the
        # generated unit keeps the native name, so the credential store's
        # entries for it apply as they always did.
        ${lib.concatMapStringsSep "\n" (c: "LoadCredential=${c}") loadCredentials}
        ${lib.concatMapStringsSep "\n" (k: "Environment=${k}=${env.${k}}") (lib.attrNames env)}
        Environment=CREDENTIALS_DIRECTORY=${credDir}
        # Read the configuration on the host before the container runs: a
        # broken configuration fails the start, as the native unit's
        # ExecStartPre did.
        ExecStartPre=${instance.package}/bin/authelia --config=${lib.concatStringsSep "," configFiles} validate-config

        [Install]
        WantedBy=multi-user.target
      '';
    };

  autheliaInstances = map instancesOf (
    lib.filter (pair: pair.instance.enable) (
      lib.mapAttrsToList (key: instance: { inherit key instance; }) config.services.authelia.instances
    )
  );

  # The web UI's image: nginx (for the SPA fallback, which darkhttpd has
  # no idea of) and the bundle copied into the image, so the container
  # serves what the front vhost's document root served, deep links
  # included.
  webuiRoot = pkgs.runCommand "losos-webui-root" { } ''
    mkdir -p $out/srv
    cp -r ${config.losos.admin.ui}/. $out/srv/
  '';
  webuiConf = pkgs.writeText "losos-webui-nginx.conf" ''
    pid /run/nginx.pid;
    error_log /dev/stderr warn;
    events { }
    http {
      access_log /dev/stdout;
      include ${pkgs.nginx}/conf/mime.types;
      server {
        listen 127.0.0.1:${toString cfg.uiPort};
        root ${webuiRoot}/srv;
        index index.html;
        location / {
          try_files $uri $uri/ /index.html;
        }
      }
    }
  '';
  webuiImageRef = "localhost/losos-webui:local";
  webuiImage = pkgs.dockerTools.buildLayeredImage {
    name = "localhost/losos-webui";
    tag = "local";
    contents = [ pkgs.nginx webuiRoot ];
    # daemon off: the container's main process is nginx itself, and the
    # unit's restart is systemd's. The configuration arrives as a mount —
    # the same way Authelia's does — so the file is not a layer of its
    # own.
    config.Cmd = [
      "/bin/nginx"
      "-c"
      "${webuiConf}"
      "-g"
      "daemon off;"
    ];
  };
  webuiContainer = pkgs.writeText "losos-webui.container" ''
    # Generated by modules/quadlets.nix. The admin pages' SPA, in a
    # container; nginx proxies the front vhost's document root to it.
    # Rootless Podman as ${cfg.user}, keep-id: the process runs as the
    # account's own uid.
    [Container]
    Image=${webuiImageRef}
    ContainerName=losos-webui
    Network=host
    UserNS=keep-id
    User=${toString sideUid}
    NoNewPrivileges=true
    Volume=${webuiConf}:${webuiConf}:ro

    [Unit]
    Description=The admin pages' web UI, in a Podman container
    After=losos-quadlet-prepare.service user@${toString sideUid}.service
    Requires=losos-quadlet-prepare.service

    [Service]
    # Rootless, as the account: its runtime directory and store, the
    # wrappers and the podman wrapper on PATH.
    User=${cfg.user}
    Group=${cfg.user}
    ${serviceEnv}
    Restart=always
    RestartSec=5s

    [Install]
    WantedBy=multi-user.target
  '';

  uiConverted = cfg.enable && config.losos.admin.enable && config.losos.admin.ui != null;

  # The account the containers run as (lososInternal.quadlets.user): the
  # same three accounts the box's own side model has, each with its own
  # sub-id range (never overlapping another's), its own Podman store under
  # /var and its own user manager — linger from boot, because rootless
  # Podman wants one and the box has no login to start it. `notshared`'s
  # range is the apps module's own (modules/apps.nix), so a converted
  # service and an app of the same account share one range, as one store.
  podmanPkg = config.virtualisation.podman.package;
  sideUser = config.users.users.${cfg.user};
  sideUid = sideUser.uid;
  runtimeDir = "/run/user/${toString sideUid}";
  podmanRoot = "/var/lib/losos-podman";
  podmanHome = "${podmanRoot}/${cfg.user}";
  subIdBase = {
    notshared = 200000;
    shared = 300000;
    kindashared = 400000;
  };
  storageConf = pkgs.writeText "losos-quadlets-${cfg.user}-storage.conf" ''
    [storage]
    driver = "overlay"
    graphroot = "${podmanHome}/storage"
    runroot = "${runtimeDir}/containers"
  '';

  # The environment of a unit that runs Podman rootless as the side user,
  # as .container [Service] lines: the account's runtime directory
  # (linger's user manager makes it), the account's own store, and the
  # setuid wrappers (newuidmap, newgidmap) plus the podman wrapper on
  # PATH.
  #
  # Daemonless by design, as Podman is: no `podman system service` and no
  # session bus — each unit runs `podman run` itself, conmon is one
  # process per container, and the unit is the cgroup. (The tree's one
  # long-running API, losos-podman-api for Traefik, is socket-activated
  # and predates this module.)
  serviceEnvMap = {
    XDG_RUNTIME_DIR = runtimeDir;
    CONTAINERS_STORAGE_CONF = "${storageConf}";
    PATH = "/run/wrappers/bin:${podmanPkg}/bin";
  };
  serviceEnv = lib.concatStringsSep "\n" (
    map (k: "Environment=\"${k}=${serviceEnvMap.${k}}\"") (lib.attrNames serviceEnvMap)
  );

  # Images load into the account's own store once, by marker under /var
  # (which /persist keeps): a boot after an update loads only what
  # changed.
  images =
    map (i: {
      inherit (i) imageRef;
      archive = i.image;
    }) autheliaInstances
    ++ lib.optionals uiConverted [
      {
        imageRef = webuiImageRef;
        archive = webuiImage;
      }
    ];
in
{
  config = lib.mkIf cfg.enable {
    # The quadlet generator rides in the podman package (systemd.packages,
    # the NixOS Podman module's): every daemon-reload turns the .container
    # files into units.
    virtualisation.podman.enable = true;

    # The account: linger (a user manager from boot, without a login) and
    # a range of sub-ids of its own for the user namespace.
    users.users.${cfg.user} = {
      linger = true;
      subUidRanges = [
        {
          startUid = subIdBase.${cfg.user};
          count = 65536;
        }
      ];
      subGidRanges = [
        {
          startGid = subIdBase.${cfg.user};
          count = 65536;
        }
      ];
    };

    # The account's store and state directories, owned by it.
    systemd.tmpfiles.rules = [
      "d ${podmanHome} 0700 ${cfg.user} ${cfg.user} -"
      "d ${podmanHome}/storage 0700 ${cfg.user} ${cfg.user} -"
      "d /var/lib/losos-quadlets 0755 root root -"
      "d /var/lib/losos-quadlets/loaded 0755 root root -"
    ];

    lososInternal.quadlets.units = lib.mkMerge (
      (map (i: { ${i.unit} = "LosOS Login"; }) autheliaInstances)
      ++ (lib.optionals uiConverted [
        {
          losos-webui = "Admin pages";
        }
      ])
    );

    systemd.services = lib.mkMerge [
      {
        # What the account must own before anything starts: the images in
        # its store (as the account itself — the store is rootless's), and
        # the state directories of every converted service (the container
        # writes what its settings name, so the files it leaves are the
        # account's, not root's).
        losos-quadlet-prepare = {
          description = "Prepare the quadlets' Podman store and state";
          wantedBy = [ "multi-user.target" ];
          before =
            map (i: "${i.unit}.service") autheliaInstances
            ++ lib.optionals uiConverted [ "losos-webui.service" ];
          wants = [ "user@${toString sideUid}.service" ];
          after = [ "user@${toString sideUid}.service" ];
          serviceConfig = {
            Type = "oneshot";
            RemainAfterExit = true;
          };
          path = [
            pkgs.coreutils
            pkgs.util-linux
            podmanPkg
          ];
          script = ''
            export XDG_RUNTIME_DIR=${runtimeDir} CONTAINERS_STORAGE_CONF=${storageConf}
            ${
              lib.concatMapStrings (i: ''
                for d in ${i.stateDirs}; do
                  install -d -o ${cfg.user} -g ${cfg.user} "$d"
                done
              '') autheliaInstances
            }
            ${lib.concatMapStrings (img: ''
              if ! runuser -u ${cfg.user} -- ${podmanPkg}/bin/podman image exists "${img.imageRef}" 2> /dev/null; then
                runuser -u ${cfg.user} -- ${podmanPkg}/bin/podman load --quiet --input "${img.archive}"
              fi
            '') images}
          '';
        };
      }

      # The native unit steps aside for the container one. Everything the
      # identity and Authelia modules hang off the unit's name (the
      # restarts of modules/identity.nix, the path unit that starts LosOS
      # Login once the first person exists) keeps working: the name stays.
      (lib.listToAttrs (
        map (i: lib.nameValuePair i.unit { enable = lib.mkForce false; }) autheliaInstances
      ))
    ];

    environment.etc = lib.mkMerge (
      (map (i: {
        "containers/systemd/${i.unit}.container".source = i.container;
      }) autheliaInstances)
      ++ (lib.optionals uiConverted [
        {
          "containers/systemd/losos-webui.container".source = webuiContainer;
        }
      ])
    );

    # The generated units into multi-user.target: the .container file's
    # [Install] alone does nothing until something enables the unit, and
    # the box has no hand to run `systemctl enable`.
    systemd.targets.multi-user.wants =
      map (i: "${i.unit}.service") autheliaInstances
      ++ lib.optionals uiConverted [ "losos-webui.service" ];

    # The front vhost hands the SPA's document root to the container and
    # keeps everything else: the LAN guard and the no-JavaScript
    # sub_filter (which run on the proxied response too), the server-level
    # headers, and the root for the locations that keep serving from the
    # store (/assets/, the handbook, the Git pages — all faster from the
    # host's own nginx than through the proxy).
    services.nginx.virtualHosts."losos-front".locations."/" = lib.mkIf uiConverted {
      proxyPass = "http://127.0.0.1:${toString cfg.uiPort}";
      tryFiles = lib.mkForce null;
    };
  };
}
