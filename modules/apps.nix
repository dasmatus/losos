# Apps on this box: the ones the owner installs (a chart from the catalogue
# search, or an app in the LosOS app format) and the box's own
# (losos.apps.builtin).
#
# The user an app runs as picks what runs it:
#
#   notshared   rootless Podman Compose, as `notshared` itself. No cluster:
#               a box that lends nothing to the mesh runs its apps without
#               one. A chart is rendered with `helm template`, shaped, and
#               turned into a Compose project (backend/src/compose.rs).
#   shared      a Helm release in this box's own k3s cluster, as before,
#               because that is the side of the box the mesh touches. Only
#               on a box that runs that cluster (Nextcloud in
#               container mode).
#
# lososd decides (backend/src/apps.rs); this module provides what it starts:
#
#   losos-apps        the job lososd runs as a `systemd-run` transient unit,
#                     one per app (`losos-app-<name>`). `install` shapes the
#                     app and starts it, `remove` takes it away. It reads the
#                     app, the user and the values from the app's record and
#                     writes the outcome back to it.
#   losos-shape       a Helm 4 post-renderer plugin, for a `shared` app. Helm
#                     hands it every rendered object, hooks included; `yq`
#                     turns them into JSON and `losos-app-shape` applies the
#                     rules in backend/src/apps.rs: host network, the chosen
#                     uid, no root, no capabilities, claims as folders in that
#                     user's data domain, no cluster access. A chart that
#                     needs more is refused with the reason.
#   losos-podman-app@ one unit per `notshared` app: `podman-compose up -d` as
#                     `notshared` on start, `down` on stop. The job restarts
#                     it; losos-podman-apps starts every one at boot.
#   the apps repo     a Git repository owned by `notshared`, apart from the
#                     box's configuration in /etc/nixos: one folder per
#                     installed `notshared` app with its Compose file (and
#                     the files it ships, or for a chart the Compose file
#                     the box made of it), one commit per install, update
#                     and removal. The job writes it as `notshared`.
#   the forwards      nginx includes one file per installed app from
#                     forwardDir, forwarding a port of losos.apps.ports to
#                     the app, LAN only. The job writes the file and reloads
#                     nginx. The box's own apps get a path on the front door
#                     instead.
#
# WHY THE SHAPING. The local cluster has no pod network, no kube-proxy and no
# DNS, and its node is NotReady for life (modules/cluster.nix). A chart as
# published would never start there. Shaping makes it run the way the box's
# own Nextcloud does, and the non-root uid keeps an app on the host
# network out of lososd's API (the OUTPUT rule in modules/daemon.nix). The
# Podman side keeps the same rules for the same reasons.
#
# WHY A TRANSIENT UNIT. Like a backup (modules/backup.nix): lososd runs with
# ProtectHome=true, and the shaping makes the app's folders under
# /home/<user>/data. A unit started by PID 1 is outside that sandbox and
# outlives a lososd restart.
#
# WHY LINGER. Rootless Podman wants a user manager: /run/user/1000, a user
# bus, and systemd scopes for the containers. Lingering gives `notshared`
# one from boot, without a login, and nothing else.
{
  pkgs,
  lib,
  config,
  ...
}:

let
  cfg = config.losos.apps;
  pkg = config.losos.backend.package;

  # The box's own cluster: modules/cluster.nix runs it while Nextcloud
  # is in container mode. A `shared` app needs it.
  localCluster = config.services.k3s.enable;
  # Installs from the Apps pane.
  enabled = cfg.enable && pkg != null;
  # Anything at all on Podman: installs, or the box's own apps.
  podmanOn = pkg != null && (cfg.enable || cfg.builtin != { });
  builtinNames = lib.attrNames cfg.builtin;

  owner = config.users.users.notshared;
  podmanRoot = "/var/lib/losos-podman";
  podmanHome = "${podmanRoot}/notshared";
  appsRepo = "${podmanHome}/apps";
  runtimeDir = "/run/user/${toString owner.uid}";
  podmanPkg = config.virtualisation.podman.package;

  storageConf = pkgs.writeText "losos-podman-storage.conf" ''
    [storage]
    driver = "overlay"
    graphroot = "${podmanHome}/storage"
    runroot = "${runtimeDir}/containers"
  '';

  # What every Podman command of an app runs with. The user manager's
  # runtime directory, and the image store under /var (persisted) rather
  # than in a home that is tmpfs above data/.
  podmanEnv = {
    XDG_RUNTIME_DIR = runtimeDir;
    DBUS_SESSION_BUS_ADDRESS = "unix:path=${runtimeDir}/bus";
    CONTAINERS_STORAGE_CONF = "${storageConf}";
  };

  podmanUp = pkgs.writeShellApplication {
    name = "losos-podman-up";
    runtimeInputs = [
      podmanPkg
      pkgs.podman-compose
      pkgs.coreutils
      pkgs.gnugrep
    ];
    text = ''
      release=''${1:?the name of the app}
      project="${podmanHome}/projects/$release"
      cd "$project"

      # Image archives from the Nix store: loaded once each, by name.
      mkdir -p "${podmanHome}/loaded"
      while read -r archive; do
        [[ -n "$archive" ]] || continue
        marker="${podmanHome}/loaded/$(basename "$archive")"
        if [[ ! -e "$marker" ]]; then
          podman load --quiet --input "$archive"
          touch "$marker"
        fi
      done < images.txt

      podman-compose --project-name "losos-$release" --file compose.json up --detach

      # `up` returns once the containers are created. One that stops at once
      # with an error is a failed start, not a running app: say which, and
      # its last words.
      sleep 5
      failed=0
      while read -r name state code; do
        if [[ "$state" == exited && "$code" != 0 ]]; then
          echo "losos-podman-up: $name stopped with $code" >&2
          podman logs --tail 20 "$name" >&2 || true
          failed=1
        fi
      done < <(podman ps --all --filter "label=io.podman.compose.project=losos-$release" \
        --format '{{.Names}} {{.State}} {{.ExitCode}}')
      exit "$failed"
    '';
  };

  podmanDown = pkgs.writeShellApplication {
    name = "losos-podman-down";
    runtimeInputs = [
      podmanPkg
      pkgs.podman-compose
      pkgs.findutils
    ];
    text = ''
      release=''${1:?the name of the app}
      project="${podmanHome}/projects/$release"
      if [[ -e "$project/compose.json" ]]; then
        cd "$project"
        podman-compose --project-name "losos-$release" --file compose.json down || true
      fi
      # `down` reads the Compose file as it is now. After an update that
      # dropped or renamed a service, or a start that failed halfway, some
      # of the project's containers are not in it: take them by label.
      podman ps --all --quiet --filter "label=io.podman.compose.project=losos-$release" \
        | xargs --no-run-if-empty podman rm --force --time 10
    '';
  };

  helm = pkgs.kubernetes-helm;
  stateDir = "/var/lib/losos/apps";
  helmHome = "/var/lib/losos/helm";
  forwardDir = "/var/lib/losos-apps/nginx";
  # k3s writes it on start (root, 0600). /etc/rancher is persisted.
  kubeconfig = "/etc/rancher/k3s/k3s.yaml";

  # Ports an app may not listen on. Everything the box itself listens on or
  # may listen on, and the forward range, which nginx holds. An app on one of
  # these would either fail to bind or, after a reboot that started it
  # first, take the port from the service that owns it.
  reserved = lib.unique (
    [
      80
      443
      config.losos.admin.apiPort
      config.losos.nextcloud.apachePort
      3000 # LosOS Git, modules/git.nix
      config.losos.lab.libvirt.port
      5432
      6379
      6443
      6444
      9345
      10010
      10248
      10249
      10250
      10256
      10257
      10258
      10259
      config.losos.cluster.kubeletPort
      10268
    ]
    ++ lib.range cfg.ports.from cfg.ports.to
  );
  reservedArg = lib.concatMapStringsSep "," toString reserved;

  # The box's own apps: the port each one's page listens on, and where its
  # Compose file and files are in the store.
  builtinPort = b: b.compose."x-losos".port or null;
  builtinTaken = lib.mapAttrsToList (n: b: "${toString (builtinPort b)}=${n}") (
    lib.filterAttrs (_: b: builtinPort b != null) cfg.builtin
  );
  builtinSource =
    name: b:
    pkgs.runCommand "losos-app-${name}" { } ''
      mkdir -p $out/files
      cp ${pkgs.writeText "compose.json" (builtins.toJSON b.compose)} $out/compose.json
      ${lib.optionalString (b.files != null) "cp -r ${b.files}/. $out/files/"}
    '';

  shapeScript = pkgs.writeShellScript "losos-shape" ''
    set -euo pipefail
    ${pkgs.yq-go}/bin/yq -o=json -I=0 '.' | ${pkg}/bin/losos-app-shape "$@"
  '';

  # A Helm 4 plugin directory. Helm 4 takes a post-renderer only as a plugin
  # (`--post-renderer <name>`), no longer as a bare executable.
  helmPlugins = pkgs.runCommand "losos-helm-plugins" { } ''
    mkdir -p $out/losos-shape
    cat > $out/losos-shape/plugin.yaml <<EOF
    apiVersion: v1
    name: losos-shape
    type: postrenderer/v1
    runtime: subprocess
    version: 1.0.0
    runtimeConfig:
      platformCommand:
        - command: ${shapeScript}
    EOF
  '';

  # The forward for one app. Same guard as `lanOnly` in
  # modules/containers.nix, for the same reasons: LAN only, never loopback
  # (the tunnel delivers from there), never the pod CIDR. IPv4 only, as the
  # front vhost listens: a `[::]` listen on a box without IPv6 would stop
  # nginx from starting at the next boot.
  forwardTemplate = pkgs.writeText "losos-app-forward.conf" ''
    server {
      listen @FRONT@;
      if ($remote_addr = $server_addr) {
        return 403;
      }
      deny 10.42.0.0/16;
      allow 10.0.0.0/8;
      allow 172.16.0.0/12;
      allow 192.168.0.0/16;
      deny all;
      client_max_body_size 0;
      location / {
        proxy_pass http://127.0.0.1:@APP@;
        proxy_http_version 1.1;
        proxy_set_header Host $http_host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection $connection_upgrade;
        proxy_buffering off;
        proxy_read_timeout 1h;
      }
    }
  '';

  appsJob = pkgs.writeShellApplication {
    name = "losos-apps";
    runtimeInputs = [
      helm
      pkgs.kubectl
      pkgs.jq
      pkgs.yq-go
      pkgs.coreutils
      pkgs.gnused
      pkgs.gnutar
      pkgs.gitMinimal
      pkgs.util-linux
      config.systemd.package
      pkg
    ];
    text = ''
      action=''${1:?install or remove}
      release=''${2:?the name of the app}
      dir="''${LOSOS_APPS_DIR:-${stateDir}}/$release"
      record="$dir/app.json"
      ns="losos-app-$release"
      forward="${forwardDir}/$release.conf"

      export KUBECONFIG=${kubeconfig}
      export HELM_CACHE_HOME=${helmHome}/cache
      export HELM_CONFIG_HOME=${helmHome}/config
      export HELM_DATA_HOME=${helmHome}/data
      export HELM_PLUGINS=${helmPlugins}

      # phase, message, app port (or "null")
      set_phase() {
        jq --arg phase "$1" --arg message "$2" --argjson port "''${3:-null}" '
          .phase = $phase
          | .message = (if $message == "" then null else $message end)
          | .updatedAt = (now | floor)
          | (if $port == null then . else .appPort = $port end)
        ' "$record" > "$record.tmp"
        chmod 600 "$record.tmp"
        mv "$record.tmp" "$record"
      }

      # The last thing a failed command said, without Helm's "Error: ".
      last_words() {
        sed -e '/^[[:space:]]*$/d' "$1" | tail -n 1 | sed -e 's/^Error: //' | cut -c 1-400
      }

      reload_nginx() {
        systemctl reload nginx.service
      }

      run_as=$(jq -r '.runAs' "$record")
      kind=$(jq -r '.kind // "chart"' "$record")
      project="${podmanRoot}/$run_as/projects/$release"
      unit="losos-podman-app@$release.service"

      # The apps repo, written as `notshared` so that root never follows a
      # path the user could have laid. Root reads the app's source (root's
      # alone) and hands it over as a tar stream.
      as_owner() {
        runuser -u notshared -- env HOME=${podmanHome} GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null "$@"
      }
      repo_git() {
        as_owner git -C ${appsRepo} -c user.name=LosOS -c user.email=apps@losos.local "$@"
      }
      # Commit what changed under the app's folder, if anything did. Called
      # where errexit is off, so every step says when it fails.
      repo_commit() {
        repo_git add --all -- "$release" || return 1
        if ! repo_git diff --cached --quiet; then
          repo_git commit --quiet --message "$1" || return 1
        fi
      }
      # The app's folder as it now runs: its Compose file and its files, or
      # for a chart the Compose file made of it.
      repo_store() {
        local verb=Install
        if [[ ! -d ${appsRepo}/.git ]]; then
          as_owner mkdir -p ${appsRepo} || return 1
          repo_git init --quiet --initial-branch=main || return 1
        fi
        if [[ -e ${appsRepo}/$release ]]; then
          verb=Update
        fi
        as_owner rm -rf "${appsRepo}/$release" || return 1
        as_owner mkdir "${appsRepo}/$release" || return 1
        if [[ "$kind" == compose ]]; then
          tar -C "$dir/source" -cf - compose.yaml files \
            | as_owner tar -C "${appsRepo}/$release" -xf - || return 1
        else
          as_owner cp "$project/compose.json" "${appsRepo}/$release/compose.json" || return 1
        fi
        repo_commit "$verb $release"
      }

      # The forward for the app's port, once it runs. A reload tests the
      # configuration first and keeps the old one if it fails; the file is
      # taken back out so the next boot's nginx does not stumble over it.
      open_port() {
        local port=$1 front
        front=$(jq -r '.frontPort' "$record")
        if [[ "$port" =~ ^[0-9]+$ && "$front" =~ ^[0-9]+$ ]]; then
          sed -e "s/@FRONT@/$front/" -e "s/@APP@/$port/" ${forwardTemplate} > "$forward.tmp"
          mv "$forward.tmp" "$forward"
        else
          rm -f "$forward"
        fi
        if ! reload_nginx; then
          rm -f "$forward"
          reload_nginx || true
          set_phase failed "The app is installed, but the box could not open its port." "$port"
          exit 1
        fi
      }

      # Why a shaping failed: the shaper's own sentence, else the last thing
      # the command before it said.
      refusal() {
        if [[ -e "$report" ]] && jq -e '.ok == false' "$report" > /dev/null; then
          jq -r '.refusal' "$report"
        else
          last_words "$dir/helm.err"
        fi
      }

      # Ports other apps listen on, so two cannot both claim one: installed
      # apps from their records, the box's own from here.
      taken=$(
        {
          for other in "''${LOSOS_APPS_DIR:-${stateDir}}"/*/app.json; do
            [[ -e "$other" ]] || continue
            jq -r --arg me "$release" \
              'select(.release != $me and .appPort != null) | "\(.appPort)=\(.release)"' "$other"
          done
          ${lib.concatMapStrings (p: "echo ${lib.escapeShellArg p}\n") builtinTaken}
        } | paste -sd, -
      )
      report="$dir/render.json"
      rm -f "$report"
      shape_args=(--release="$release" --run-as="$run_as" --reserved="${reservedArg}"
        --taken="$taken" --report="$report")

      # Before notshared apps ran on Podman they were Helm releases in the
      # box's own cluster, under the same name. Installing or removing one
      # takes the old release away first, so two copies never share a port.
      leave_cluster() {
        ${
          if localCluster then
            ''
              if [[ -r ${kubeconfig} ]] && helm status "$release" --namespace "$ns" > /dev/null 2>&1; then
                echo "losos-apps: taking the cluster's copy of $release away"
                helm uninstall "$release" --namespace "$ns" --wait=watcher --timeout 5m 2>> "$dir/helm.err" || return 1
                kubectl delete namespace "$ns" --ignore-not-found --wait=false || true
              fi
            ''
          else
            ":"
        }
      }

      chart_ref() {
        repo=$(jq -r '.chart.repo' "$record")
        name=$(jq -r '.chart.name' "$record")
        version=$(jq -r '.chart.version' "$record")
        chart_args=(--version "$version")
        if [[ "''${repo,,}" == oci://* ]]; then
          ref="''${repo%/}/$name"
        else
          ref="$name"
          chart_args+=(--repo "$repo")
        fi
        jq '.values // {}' "$record" > "$dir/values.json"
        if jq -e '.valuesYaml | type == "string"' "$record" > /dev/null; then
          jq -r '.valuesYaml' "$record" > "$dir/values.yaml"
          chart_args+=(-f "$dir/values.yaml")
        else
          rm -f "$dir/values.yaml"
        fi
        chart_args+=(-f "$dir/values.json")
      }

      case "$action:$run_as" in
        install:notshared)
          : > "$dir/helm.err"
          if [[ "$kind" == compose ]]; then
            echo "losos-apps: making $release from its compose file"
            if ! yq -o=json -I=0 '.' "$dir/source/compose.yaml" > "$dir/compose.in.json" 2> "$dir/helm.err"; then
              cat "$dir/helm.err" >&2
              set_phase failed "The compose file could not be read: $(last_words "$dir/helm.err")"
              exit 1
            fi
            mkdir -p "$dir/source/files"
            ok=1
            losos-app-shape "''${shape_args[@]}" --podman --compose --source "$dir/source/files" \
              < "$dir/compose.in.json" || ok=0
          else
            chart_ref
            echo "losos-apps: rendering $name $version as $release ($run_as, Podman)"
            ok=1
            helm template "$release" "$ref" "''${chart_args[@]}" --namespace "$ns" 2> "$dir/helm.err" \
              | yq -o=json -I=0 '.' 2>> "$dir/helm.err" \
              | losos-app-shape "''${shape_args[@]}" --podman || ok=0
          fi
          if [[ "$ok" != 1 ]]; then
            cat "$dir/helm.err" >&2
            message=$(refusal)
            set_phase failed "''${message:-The app could not be made ready.}"
            exit 1
          fi
          if ! leave_cluster; then
            set_phase failed "The cluster's copy of the app could not be taken away: $(last_words "$dir/helm.err")"
            exit 1
          fi
          echo "losos-apps: starting $release"
          if ! systemctl restart "$unit"; then
            journalctl --unit "$unit" --lines 20 --output cat >&2 || true
            set_phase failed "The app did not start. Its last words are in the job log."
            exit 1
          fi
          port=$(jq '.port // null' "$report")
          open_port "$port"
          if ! repo_store; then
            echo "losos-apps: $release runs, but its Compose file is not in the apps repo" >&2
          fi
          set_phase running "" "$port"
          echo "losos-apps: $release is running"
          ;;

        install:shared)
          chart_ref
          args=(upgrade --install "''${chart_args[@]}"
            --namespace "$ns" --create-namespace --skip-crds
            --wait=watcher --timeout 15m)
          args+=(--post-renderer losos-shape)
          for a in "''${shape_args[@]}"; do
            args+=(--post-renderer-args="$a")
          done

          echo "losos-apps: installing $name $version as $release ($run_as)"
          if ! helm "''${args[@]}" -- "$release" "$ref" 2> "$dir/helm.err"; then
            cat "$dir/helm.err" >&2
            message=$(refusal)
            set_phase failed "''${message:-The install did not finish.}"
            exit 1
          fi
          port=$(jq '.port // null' "$report")
          open_port "$port"
          set_phase running "" "$port"
          echo "losos-apps: $release is running"
          ;;

        remove:notshared)
          echo "losos-apps: removing $release"
          : > "$dir/helm.err"
          if ! systemctl stop "$unit" || ! leave_cluster; then
            set_phase failed "The app did not stop. Try again."
            exit 1
          fi
          rm -rf "$project"
          rm -f "$forward"
          reload_nginx || true
          if [[ -d ${appsRepo}/$release ]]; then
            as_owner rm -rf "${appsRepo}/$release"
            repo_commit "Remove $release" || true
          fi
          # The record goes; the app's files stay in the user's data domain.
          rm -rf "$dir"
          echo "losos-apps: $release removed"
          ;;

        remove:shared)
          echo "losos-apps: removing $release"
          if ! helm uninstall "$release" --namespace "$ns" --wait=watcher --timeout 5m --ignore-not-found 2> "$dir/helm.err"; then
            cat "$dir/helm.err" >&2
            set_phase failed "$(last_words "$dir/helm.err")"
            exit 1
          fi
          kubectl delete namespace "$ns" --ignore-not-found --wait=false
          rm -f "$forward"
          reload_nginx || true
          rm -rf "$dir"
          echo "losos-apps: $release removed"
          ;;

        *)
          echo "losos-apps: unknown action $action" >&2
          exit 2
          ;;
      esac
    '';
  };
in
{
  config = lib.mkMerge [
    (lib.mkIf enabled {
      systemd.services.lososd = {
        environment = {
          LOSOS_APPS_JOB = lib.getExe appsJob;
          LOSOS_APPS_DIR = stateDir;
          LOSOS_APPS_PORTS = "${toString cfg.ports.from}-${toString cfg.ports.to}";
          LOSOS_APPS_BUILTIN = lib.concatStringsSep "," builtinNames;
          LOSOS_HELM_HOME = helmHome;
        }
        # A `shared` app needs the box's own cluster (backend/src/apps.rs).
        // lib.optionalAttrs localCluster { LOSOS_APPS_KUBE = "1"; };
        # `GET /api/apps/chart` pulls the chart and reads its values
        # (backend/src/io_backend.rs). `path` replaces PATH, so both have to
        # be named here or the dialog fails with ENOENT.
        path = [
          helm
          pkgs.yq-go
        ];
      };

      services.nginx.appendHttpConfig = ''
        include ${forwardDir}/*.conf;
      '';

      systemd.tmpfiles.rules = [
        "d /var/lib/losos-apps 0755 root root -"
        "d ${forwardDir} 0755 root root -"
      ];

      networking.firewall.allowedTCPPortRanges = [
        {
          inherit (cfg.ports) from to;
        }
      ];

      assertions = [
        {
          assertion = cfg.ports.from <= cfg.ports.to && cfg.ports.from >= 1024;
          message = "losos.apps.ports must be a range from 1024 up, with `from` at most `to`.";
        }
      ];
    })

    (lib.mkIf podmanOn {
      virtualisation.podman.enable = true;

      # Rootless Podman as `notshared`: a range of sub-ids for the user
      # namespace, and a user manager from boot (see WHY LINGER above).
      users.users.notshared = {
        linger = true;
        subUidRanges = [
          {
            startUid = 200000;
            count = 65536;
          }
        ];
        subGidRanges = [
          {
            startGid = 200000;
            count = 65536;
          }
        ];
      };

      systemd.tmpfiles.rules = [
        "d ${podmanRoot} 0755 root root -"
        "d ${podmanHome} 0700 notshared notshared -"
        "d ${podmanHome}/storage 0700 notshared notshared -"
        "d ${podmanHome}/projects 0700 notshared notshared -"
      ];

      # One per `notshared` app. Started by the install job and at boot;
      # `up` leaves the containers in scopes of the user manager, and `down`
      # on stop takes them away again.
      systemd.services."losos-podman-app@" = {
        description = "LosOS app %i";
        wants = [
          "network-online.target"
          "user@${toString owner.uid}.service"
        ];
        after = [
          "network-online.target"
          "user@${toString owner.uid}.service"
        ];
        environment = podmanEnv;
        # newuidmap and newgidmap are setuid wrappers.
        path = [ "/run/wrappers" ];
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
          User = "notshared";
          Group = "notshared";
          ExecStart = "${lib.getExe podmanUp} %i";
          # Post, not ExecStop: it runs after a failed start too, which can
          # leave half a project running.
          ExecStopPost = "${lib.getExe podmanDown} %i";
          # The first start pulls the images.
          TimeoutStartSec = "30min";
          TimeoutStopSec = "5min";
        };
      };

      # Every installed `notshared` app that was running, again after a boot.
      systemd.services.losos-podman-apps = {
        description = "Start the installed LosOS apps";
        wantedBy = [ "multi-user.target" ];
        wants = [ "network-online.target" ];
        after = [
          "network-online.target"
          "user@${toString owner.uid}.service"
        ];
        path = [
          pkgs.jq
          config.systemd.package
        ];
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
        };
        script = ''
          for record in ${stateDir}/*/app.json; do
            [[ -e "$record" ]] || continue
            if jq -e '.runAs == "notshared" and .phase == "running"' "$record" > /dev/null; then
              release=$(jq -r .release "$record")
              if [[ -e "${podmanHome}/projects/$release/compose.json" ]]; then
                systemctl start --no-block "losos-podman-app@$release.service"
              elif ${lib.boolToString enabled}; then
                # Installed while notshared apps were Helm releases: the
                # job makes it a Podman project and takes the old one away,
                # as lososd would run it.
                systemd-run --no-block --unit "losos-app-$release" \
                  --description "losos app install $release" \
                  --property CollectMode=inactive-or-failed \
                  --property "StandardOutput=append:${stateDir}/$release/job.log" \
                  --property "StandardError=append:${stateDir}/$release/job.log" \
                  --setenv LOSOS_APPS_DIR=${stateDir} \
                  ${lib.getExe appsJob} install "$release"
              fi
            fi
          done
        '';
      };
    })

    (lib.mkIf (podmanOn && cfg.builtin != { }) {
      # The box's own apps: shaped like an installed one at every boot, from
      # the store, then started.
      systemd.services = lib.mapAttrs' (
        name: b:
        lib.nameValuePair "losos-builtin-${name}" {
          description = "LosOS app ${name}, one of the box's own";
          wantedBy = [ "multi-user.target" ];
          wants = [ "network-online.target" ];
          after = [
            "network-online.target"
            "user@${toString owner.uid}.service"
          ];
          path = [
            pkgs.yq-go
            config.systemd.package
          ];
          serviceConfig = {
            Type = "oneshot";
            RemainAfterExit = true;
            ExecStop = "${config.systemd.package}/bin/systemctl stop losos-podman-app@${name}.service";
          };
          script = ''
            report=$(mktemp)
            ${pkg}/bin/losos-app-shape --podman --compose \
              --release=${name} --run-as=notshared \
              --reserved="${reservedArg}" \
              --taken=${lib.escapeShellArg (lib.concatStringsSep "," builtinTaken)} \
              --report="$report" --source ${builtinSource name b}/files \
              < ${builtinSource name b}/compose.json
            systemctl restart losos-podman-app@${name}.service
          '';
        }
      ) cfg.builtin;

      services.nginx.virtualHosts."losos-front".locations = lib.mapAttrs' (
        _: b:
        lib.nameValuePair b.path {
          proxyPass = "http://127.0.0.1:${toString (builtinPort b)}/";
          proxyWebsockets = true;
          extraConfig = lib.optionalString b.lanOnly (import ./lan-only.nix) + b.locationConfig;
        }
      ) (lib.filterAttrs (_: b: b.path != null) cfg.builtin);

      assertions = lib.flatten (
        lib.mapAttrsToList (name: b: [
          {
            assertion = builtinPort b == null || !(lib.elem (builtinPort b) reserved);
            message = "losos.apps.builtin.${name}: its port is one the box keeps for itself.";
          }
          {
            assertion = b.path == null || builtinPort b != null;
            message = "losos.apps.builtin.${name}: a path needs x-losos.port in the Compose file.";
          }
          {
            assertion = b.path == null || (lib.hasPrefix "/" b.path && lib.hasSuffix "/" b.path);
            message = "losos.apps.builtin.${name}: the path starts and ends with a slash.";
          }
          {
            assertion = builtins.match "[a-z][a-z0-9-]{0,39}" name != null && !(lib.hasSuffix "-" name);
            message = "losos.apps.builtin.${name}: the name is lower-case letters, digits and dashes.";
          }
        ]) cfg.builtin
      );
    })
  ];
}
