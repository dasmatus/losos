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
#               on a box that runs that cluster: sharing on and in a
#               mesh (modules/cluster.nix).
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
#   losos-traefik     Traefik, which holds losos.apps.ports: one
#                     entrypoint per port, LAN only. It reads its routes off
#                     the apps' containers, whose labels the shaping writes
#                     (backend/src/compose.rs `route`), through
#                     losos-podman-api, a socket-activated Podman API of
#                     `notshared` that only Traefik's group may open. A
#                     `shared` app, a Helm release with no container labels,
#                     gets a route file in traefikDir from the job instead.
#                     Nothing reloads: an app's port opens when its
#                     containers start and closes when they go. The box's
#                     own apps get a path on the front door (nginx) instead.
#                     Its API and metrics answer on 127.0.0.1 only
#                     (losos.apps.traefikPort), for the job and for lososd's
#                     statistics plugin.
#
# WHY THE SHAPING. The local cluster has no pod network, no kube-proxy and no
# DNS, and its node is NotReady for life (modules/cluster.nix). A chart as
# published would never start there. Shaping makes it run the way the box's
# own pods once did, and the non-root uid keeps an app on the host
# network out of lososd's API (the OUTPUT rule in modules/daemon.nix). The
# Podman side keeps the same rules for the same reasons.
#
# WHY A TRANSIENT UNIT. Like a backup (modules/backup.nix): lososd runs with
# ProtectHome=true, and the shaping makes the app's folders under
# /home/<user>/data. A unit started by PID 1 is outside that sandbox and
# outlives a lososd restart.
#
# WHY TRAEFIK HERE AND NGINX IN FRONT. nginx stays the front door: LosOS
# Login's auth_request, the CSP arms, the sockets only its group may open
# (LosOS Cloud, LosOS Git) and the Nextcloud addresses all live in its
# vhost, and none of that is an app's business. What Traefik takes is the
# forward range, which was one nginx file per app, written by root and
# applied by a reload that could fail and take every forward with it.
# Labels make the route part of the app: written once by the shaping,
# committed with its Compose file to the apps repo, gone with its
# containers.
#
# WHY TRAEFIK IS NOT NOTSHARED. Whoever opens the Podman API can run
# anything as `notshared`, so the socket is the whole grant and Traefik's
# own user adds no wall against that. It still keeps a Traefik bug from
# reading the owner's files directly, and keeps the `shared` apps' traffic
# out of the owner's uid.
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

  # The box's own cluster: modules/cluster.nix runs it on the shared side
  # (sharing on and in a mesh). A `shared` app needs it.
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
  # k3s writes it on start (root, 0600). /etc/rancher is persisted.
  kubeconfig = "/etc/rancher/k3s/k3s.yaml";

  # Ports an app may not listen on. Everything the box itself listens on or
  # may listen on, and the forward range, which Traefik holds. An app on one of
  # these would either fail to bind or, after a reboot that started it
  # first, take the port from the service that owns it.
  reserved = lib.unique (
    [
      80
      443
      config.losos.admin.apiPort
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
      cfg.traefikPort
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

  # Traefik (see the header). Its file provider reads traefikDir: the
  # middleware below, linked in by tmpfiles, and a route file per `shared`
  # app, which the job writes.
  traefikDir = "/var/lib/losos-apps/traefik";
  traefikApi = "http://127.0.0.1:${toString cfg.traefikPort}";
  podmanApiSocket = "/run/losos-podman-api/podman.sock";
  yaml = pkgs.formats.yaml { };

  # The same guard as lan-only.nix, in Traefik's terms: the RFC 1918 ranges
  # with the mesh's pod network (10.42.0.0/16) cut out, because
  # ipAllowList has no deny. Loopback matches none of them, so the tunnel
  # (which delivers from 127.0.0.1) is turned away as on the admin routes.
  # lan-only.nix's other line, a source equal to the address it arrived
  # on, guards routes nothing else on the box can reach; an app's port is
  # on 127.0.0.1 for anything on the box anyway, so it adds nothing here.
  lanRanges = [
    "10.0.0.0/11"
    "10.32.0.0/13"
    "10.40.0.0/15"
    "10.43.0.0/16"
    "10.44.0.0/14"
    "10.48.0.0/12"
    "10.64.0.0/10"
    "10.128.0.0/9"
    "172.16.0.0/12"
    "192.168.0.0/16"
  ];
  ipv6Refuse = "INPUT -p tcp --dport ${toString cfg.ports.from}:${toString cfg.ports.to} -j REJECT --reject-with tcp-reset";
  traefikMiddleware = yaml.generate "losos-traefik-middleware.yml" {
    http.middlewares.lan-only.ipAllowList.sourceRange = lanRanges;
  };

  # One entrypoint per port of the range, on every address. No read timeout:
  # an upload to an app is as long as it is, and Traefik's default cuts a
  # request at 60 s.
  traefikStatic = yaml.generate "losos-traefik.yml" {
    global = {
      checkNewVersion = false;
      sendAnonymousUsage = false;
    };
    log.level = "WARN";
    entryPoints = {
      # `api.insecure` serves the API on the entrypoint of this name.
      traefik.address = "127.0.0.1:${toString cfg.traefikPort}";
    }
    // lib.listToAttrs (
      map (
        port:
        lib.nameValuePair "port-${toString port}" {
          address = ":${toString port}";
          transport.respondingTimeouts = {
            readTimeout = "0s";
            writeTimeout = "0s";
            idleTimeout = "1h";
          };
        }
      ) (lib.range cfg.ports.from cfg.ports.to)
    );
    providers = {
      docker = {
        endpoint = "unix://${podmanApiSocket}";
        exposedByDefault = false;
        watch = true;
      };
      file = {
        directory = traefikDir;
        watch = true;
      };
    };
    api = {
      insecure = true;
      dashboard = false;
    };
    ping.entryPoint = "traefik";
    metrics.prometheus = {
      entryPoint = "traefik";
      addEntryPointsLabels = true;
      addRoutersLabels = true;
      addServicesLabels = true;
      buckets = [
        0.05
        0.1
        0.25
        0.5
        1.0
        2.5
        5.0
      ];
    };
  };

  appsJob = pkgs.writeShellApplication {
    name = "losos-apps";
    runtimeInputs = [
      helm
      pkgs.kubectl
      pkgs.jq
      pkgs.yq-go
      pkgs.curl
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
      route_file="${traefikDir}/$release.yml"

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

      # The app's LAN port is open once Traefik has its route: from the
      # containers' labels for a notshared app, from a route file the job
      # writes for a shared one. Traefik picks either up within a few
      # seconds; a minute without it is a failure the owner should see.
      wait_route() {
        local provider=$1 port=$2
        [[ "$port" =~ ^[0-9]+$ ]] || return 0
        for _ in $(seq 1 60); do
          if curl -fsS -o /dev/null "${traefikApi}/api/http/routers/app-$release@$provider"; then
            return 0
          fi
          sleep 1
        done
        set_phase failed "The app is installed, but the box could not open its port." "$port"
        exit 1
      }

      # A shared app's route, which no container label carries. Written
      # whole and renamed into place, so Traefik never reads half a file.
      write_route() {
        local port=$1 front
        front=$(jq -r '.frontPort' "$record")
        if [[ ! "$port" =~ ^[0-9]+$ || ! "$front" =~ ^[0-9]+$ ]]; then
          rm -f "$route_file"
          return 0
        fi
        jq -n --arg name "app-$release" --arg ep "port-$front" --arg url "http://127.0.0.1:$port" '{
          http: {
            routers: { ($name): { entryPoints: [$ep], rule: "PathPrefix(`/`)", middlewares: ["lan-only@file"], service: $name } },
            services: { ($name): { loadBalancer: { servers: [{ url: $url }] } } }
          }
        }' > "$route_file.tmp"
        chmod 644 "$route_file.tmp"
        mv "$route_file.tmp" "$route_file"
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
      front=$(jq -r '.frontPort // empty' "$record")
      shape_args=(--release="$release" --run-as="$run_as" --reserved="${reservedArg}"
        --taken="$taken" --report="$report" ''${front:+--front-port="$front"})

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
          wait_route docker "$port"
          if ! repo_store; then
            echo "losos-apps: $release runs, but its Compose file is not in the apps repo" >&2
          fi
          set_phase running "" "$port"
          echo "losos-apps: $release is running"
          ;;

        install:shared | install:kindashared)
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
          write_route "$port"
          wait_route file "$port"
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
          if [[ -d ${appsRepo}/$release ]]; then
            as_owner rm -rf "${appsRepo}/$release"
            repo_commit "Remove $release" || true
          fi
          # The record goes; the app's files stay in the user's data domain.
          rm -rf "$dir"
          echo "losos-apps: $release removed"
          ;;

        remove:shared | remove:kindashared)
          echo "losos-apps: removing $release"
          if ! helm uninstall "$release" --namespace "$ns" --wait=watcher --timeout 5m --ignore-not-found 2> "$dir/helm.err"; then
            cat "$dir/helm.err" >&2
            set_phase failed "$(last_words "$dir/helm.err")"
            exit 1
          fi
          kubectl delete namespace "$ns" --ignore-not-found --wait=false
          rm -f "$route_file"
          rm -rf "$dir"
          echo "losos-apps: $release removed"
          ;;

        # A shared (or kindashared) app's route file again, from its record:
        # at boot, for an app installed before Traefik held the range.
        route:shared | route:kindashared)
          write_route "$(jq -r '.appPort // empty' "$record")"
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

      systemd.tmpfiles.rules = [
        "d /var/lib/losos-apps 0755 root root -"
        "d ${traefikDir} 0755 root root -"
        "L+ ${traefikDir}/00-losos.yml - - - - ${traefikMiddleware}"
        # nginx's forwards, one file per app, before Traefik held the range.
        "R /var/lib/losos-apps/nginx - - - - -"
      ];

      users.users.losos-traefik = {
        isSystemUser = true;
        group = "losos-traefik";
        description = "Traefik, the LAN ports of the installed apps";
      };
      users.groups.losos-traefik = { };

      # notshared's Podman API, for Traefik alone: the socket is
      # notshared's and Traefik's group may open it. Socket-activated, so a
      # box with no installs runs no API until Traefik first asks.
      systemd.sockets.losos-podman-api = {
        description = "Podman API of notshared, for Traefik";
        wantedBy = [ "sockets.target" ];
        listenStreams = [ podmanApiSocket ];
        socketConfig = {
          SocketUser = "notshared";
          SocketGroup = "losos-traefik";
          SocketMode = "0660";
          DirectoryMode = "0755";
        };
      };
      systemd.services.losos-podman-api = {
        description = "Podman API of notshared, for Traefik";
        requires = [ "losos-podman-api.socket" ];
        wants = [ "user@${toString owner.uid}.service" ];
        after = [
          "losos-podman-api.socket"
          "user@${toString owner.uid}.service"
        ];
        environment = podmanEnv;
        path = [ "/run/wrappers" ];
        serviceConfig = {
          User = "notshared";
          Group = "notshared";
          # No address: Podman takes the socket systemd hands it.
          ExecStart = "${podmanPkg}/bin/podman system service --time=0";
          Restart = "on-failure";
          # Stopping the API must not take the apps' containers with it.
          KillMode = "process";
        };
      };

      systemd.services.losos-traefik = {
        description = "Traefik, the LAN ports of the installed apps";
        wantedBy = [ "multi-user.target" ];
        wants = [
          "network-online.target"
          "losos-podman-api.socket"
        ];
        after = [
          "network-online.target"
          "losos-podman-api.socket"
          "systemd-tmpfiles-setup.service"
        ];
        serviceConfig = {
          ExecStart = "${pkgs.traefik}/bin/traefik --configFile=${traefikStatic}";
          User = "losos-traefik";
          Group = "losos-traefik";
          Restart = "always";
          RestartSec = 2;
          NoNewPrivileges = true;
          CapabilityBoundingSet = "";
          ProtectSystem = "strict";
          ProtectHome = true;
          PrivateTmp = true;
          PrivateDevices = true;
          ProtectKernelTunables = true;
          ProtectKernelModules = true;
          ProtectKernelLogs = true;
          ProtectControlGroups = true;
          ProtectClock = true;
          ProtectHostname = true;
          RestrictNamespaces = true;
          RestrictRealtime = true;
          RestrictSUIDSGID = true;
          LockPersonality = true;
          MemoryDenyWriteExecute = true;
          RestrictAddressFamilies = [
            "AF_INET"
            "AF_INET6"
            "AF_UNIX"
          ];
          SystemCallArchitectures = "native";
          SystemCallFilter = [ "@system-service" ];
          LimitNOFILE = 65536;
        };
      };

      # A shared app installed before Traefik held the range has a record
      # and no route file: the job writes one from the record.
      systemd.services.losos-app-routes = lib.mkIf localCluster {
        description = "Routes of the installed shared apps";
        wantedBy = [ "multi-user.target" ];
        after = [ "systemd-tmpfiles-setup.service" ];
        path = [ pkgs.jq ];
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
        };
        script = ''
          for record in ${stateDir}/*/app.json; do
            [[ -e "$record" ]] || continue
            if jq -e '(.runAs == "shared" or .runAs == "kindashared") and .phase == "running"' "$record" > /dev/null; then
              release=$(jq -r .release "$record")
              [[ -e "${traefikDir}/$release.yml" ]] || ${lib.getExe appsJob} route "$release"
            fi
          done
        '';
      };

      networking.firewall = {
        allowedTCPPortRanges = [
          {
            inherit (cfg.ports) from to;
          }
        ];
        # IPv4 only, as nginx's forwards were. `lan-only` can name the LAN
        # only there: an IPv6 neighbour has a global address like anyone
        # else's. Go listens on both families for any wildcard, so the
        # firewall resets IPv6 connections to the range, and a laptop that
        # tries IPv6 first falls back to IPv4 instead of reading a 403.
        extraCommands = lib.mkIf config.networking.firewall.enable ''
          ip6tables -w -D ${ipv6Refuse} 2>/dev/null || true
          ip6tables -w -I ${ipv6Refuse}
        '';
        extraStopCommands = lib.mkIf config.networking.firewall.enable ''
          ip6tables -w -D ${ipv6Refuse} 2>/dev/null || true
        '';
      };

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
              compose="${podmanHome}/projects/$release/compose.json"
              labelled=0
              if [[ -e "$compose" ]] && jq -e '[.services[].labels["losos.app"]] | all' "$compose" > /dev/null; then
                labelled=1
              fi
              if [[ "$labelled" == 1 ]] || { [[ -e "$compose" ]] && ! ${lib.boolToString enabled}; }; then
                systemctl start --no-block "losos-podman-app@$release.service"
              elif ${lib.boolToString enabled}; then
                # Installed while notshared apps were Helm releases, or
                # shaped before its containers carried their route: the job
                # shapes it again, takes any old copy away and starts it, as
                # lososd would run it.
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
