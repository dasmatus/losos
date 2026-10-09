# Apps the owner installs from the catalogue: Helm charts, run in this box's
# own k3s cluster as `notshared` or `shared`.
#
# lososd decides (backend/src/apps.rs); this module provides what it starts.
# Three pieces:
#
#   losos-apps        the job lososd runs as a `systemd-run` transient unit,
#                     one per app (`losos-app-<name>`). `install` runs
#                     `helm upgrade --install` with the shaping plugin below
#                     and waits for the pods; `remove` uninstalls. It reads
#                     the chart, the user and the values from the app's
#                     record and writes the outcome back to it.
#   losos-shape       a Helm 4 post-renderer plugin. Helm hands it every
#                     rendered object, hooks included; `yq` turns them into
#                     JSON and `losos-app-shape` (backend/src/bin) applies the
#                     rules in backend/src/apps.rs: host network, the chosen
#                     uid, no root, no capabilities, claims as folders in that
#                     user's data domain, no cluster access. A chart that
#                     needs more is refused with the reason.
#   the forwards      nginx includes one file per app from forwardDir,
#                     forwarding a port of losos.apps.ports to the app, LAN
#                     only. The job writes the file and reloads nginx.
#
# WHY THE SHAPING. The local cluster has no pod network, no kube-proxy and no
# DNS, and its node is NotReady for life (modules/cluster.nix). A chart as
# published would never start there. Shaping makes it run the way the box's
# own Nextcloud and Forgejo do, and the non-root uid keeps a pod on the host
# network out of lososd's API (the OUTPUT rule in modules/daemon.nix).
#
# WHY A TRANSIENT UNIT. Like a backup (modules/backup.nix): lososd runs with
# ProtectHome=true, and the shaping makes the app's folders under
# /home/<user>/data. A unit started by PID 1 is outside that sandbox and
# outlives a lososd restart.
#
# Only on a box that runs its own cluster: with Nextcloud and Forgejo both
# native, there is no k3s to install into and lososd says so.
{
  pkgs,
  lib,
  config,
  ...
}:

let
  cfg = config.losos.apps;
  pkg = config.losos.backend.package;

  # modules/cluster.nix's `localEnabled`, a `let` binding there.
  localCluster =
    config.losos.nextcloud.mode == "container"
    || (config.losos.forgejo.mode == "container" && config.losos.forgejo.enable);
  enabled = cfg.enable && localCluster && pkg != null;

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
      3000 # Forgejo, modules/containers.nix `forgejoPort`
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
      pkgs.coreutils
      pkgs.gnused
      config.systemd.package
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

      case "$action" in
        install)
          repo=$(jq -r '.chart.repo' "$record")
          name=$(jq -r '.chart.name' "$record")
          version=$(jq -r '.chart.version' "$record")
          run_as=$(jq -r '.runAs' "$record")
          front=$(jq -r '.frontPort' "$record")

          jq '.values // {}' "$record" > "$dir/values.json"
          args=(upgrade --install --version "$version"
            --namespace "$ns" --create-namespace --skip-crds
            --wait=watcher --timeout 15m)
          if [[ "''${repo,,}" == oci://* ]]; then
            ref="''${repo%/}/$name"
          else
            ref="$name"
            args+=(--repo "$repo")
          fi
          if jq -e '.valuesYaml | type == "string"' "$record" > /dev/null; then
            jq -r '.valuesYaml' "$record" > "$dir/values.yaml"
            args+=(-f "$dir/values.yaml")
          else
            rm -f "$dir/values.yaml"
          fi
          args+=(-f "$dir/values.json")

          # Ports other apps listen on, so two cannot both claim one.
          taken=$(
            for other in "''${LOSOS_APPS_DIR:-${stateDir}}"/*/app.json; do
              [[ -e "$other" ]] || continue
              jq -r --arg me "$release" \
                'select(.release != $me and .appPort != null) | "\(.appPort)=\(.release)"' "$other"
            done | paste -sd, -
          )
          report="$dir/render.json"
          rm -f "$report"
          args+=(--post-renderer losos-shape
            --post-renderer-args="--release=$release"
            --post-renderer-args="--run-as=$run_as"
            --post-renderer-args="--reserved=${reservedArg}"
            --post-renderer-args="--taken=$taken"
            --post-renderer-args="--report=$report")

          echo "losos-apps: installing $name $version as $release ($run_as)"
          if ! helm "''${args[@]}" -- "$release" "$ref" 2> "$dir/helm.err"; then
            cat "$dir/helm.err" >&2
            if [[ -e "$report" ]] && jq -e '.ok == false' "$report" > /dev/null; then
              message=$(jq -r '.refusal' "$report")
            else
              message=$(last_words "$dir/helm.err")
            fi
            set_phase failed "''${message:-The install did not finish.}"
            exit 1
          fi

          port=$(jq '.port // null' "$report")
          if [[ "$port" =~ ^[0-9]+$ && "$front" =~ ^[0-9]+$ ]]; then
            sed -e "s/@FRONT@/$front/" -e "s/@APP@/$port/" ${forwardTemplate} > "$forward.tmp"
            mv "$forward.tmp" "$forward"
          else
            rm -f "$forward"
          fi
          # A reload tests the configuration first and keeps the old one if
          # it fails; the file is taken back out so the next boot's nginx
          # does not stumble over it either.
          if ! reload_nginx; then
            rm -f "$forward"
            reload_nginx || true
            set_phase failed "The app is installed, but the box could not open its port." "$port"
            exit 1
          fi
          set_phase running "" "$port"
          echo "losos-apps: $release is running"
          ;;

        remove)
          echo "losos-apps: removing $release"
          if ! helm uninstall "$release" --namespace "$ns" --wait=watcher --timeout 5m --ignore-not-found 2> "$dir/helm.err"; then
            cat "$dir/helm.err" >&2
            set_phase failed "$(last_words "$dir/helm.err")"
            exit 1
          fi
          kubectl delete namespace "$ns" --ignore-not-found --wait=false
          rm -f "$forward"
          reload_nginx || true
          # The record goes; the app's files stay in the user's data domain.
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
  config = lib.mkIf enabled {
    systemd.services.lososd = {
      environment = {
        LOSOS_APPS_JOB = lib.getExe appsJob;
        LOSOS_APPS_DIR = stateDir;
        LOSOS_APPS_PORTS = "${toString cfg.ports.from}-${toString cfg.ports.to}";
        LOSOS_HELM_HOME = helmHome;
      };
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
  };
}
