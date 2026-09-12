# Master proxy + mesh control plane — edge side.
#
# The edge lives on a public VPS; a VPS flake imports it via the
# `nixosModules.edge` flake output next to its own hardware config. It runs
# two halves that share one binary and one tenant whitelist:
#
#   * the MASTER PROXY — Traefik (public TLS on :443), a rathole server
#     (appliance clients dial :2333), and losos-registrar (serve), the stub
#     Rust HTTP server that *constantly updates* Traefik's dynamic
#     file-provider config and rathole's server config from a tenant registry.
#   * the MESH CONTROL PLANE (`losos.edge.cluster.enable`) — an rke2 server
#     that appliances join as agents, carrying Longhorn and mesh compute.
#
# The two halves are deliberately separable: an edge with
# `losos.edge.cluster.enable = false` is exactly the proxy-only edge that
# shipped before the mesh existed, and every mesh unit here is gated so that
# stays true. Nothing in the proxy path may grow a hard dependency on the
# cluster path — an apiserver that is down must not take the tunnels with it.
#
# Traefik's file provider watches /etc/traefik/dynamic as a directory:
#   * register.yml  — the one static route (register.<domain> → the
#     registrar's loopback API). NixOS-managed (this module), symlinked in.
#   * losos.yml      — per-appliance routers, written at runtime by the
#     registrar. Traefik auto-reloads on change.
# The registrar is the sole writer of losos.yml and /etc/rathole/server.toml;
# it never touches register.yml. That separation keeps the registrar from
# being able to lock itself out of the control plane.
#
# See docs/superpowers/specs/2026-08-17-master-proxy-design.md and
# docs/superpowers/specs/2026-09-09-k3s-mesh-design.md. The appliance side is
# modules/proxy.nix (proxy) and modules/cluster.nix (mesh).
#
# NOTE(transport): rathole runs plain TCP for now. The tunnel carries
# post-TLS-termination HTTP, so enabling rathole's Noise transport (with a
# distributed static keypair) is a hardening step before production — see the
# spec's "Gotchas" and the rathole [transport] noise section. The registrar's
# config writer is where that flip lands.
{
  config,
  lib,
  pkgs,
  self,
  ...
}:

let
  cfg = config.losos.edge;
  enabled = cfg.enable;
  rathole = cfg.rathole.package;
  registrar = cfg.registrar.package;

  registerDomain = "register.${cfg.publicDomain}";

  # ── Mesh control plane constants ────────────────────────────────────────
  meshEnabled = cfg.cluster.enable;

  # rke2 writes the admin kubeconfig here on a SERVER only; an agent never
  # gets one. Both mesh units below run on the edge, so this path is real.
  meshKubeconfig = "/etc/rancher/rke2/rke2.yaml";

  # Credentials the registrar's join route presents to the mesh apiserver
  # when it deletes a stale node object. Runtime paths, never store paths:
  # losos-mesh-rbac.service extracts both out of the cluster after rke2 has
  # applied the RBAC manifest below.
  meshKubeTokenFile = "/var/secrets/losos-mesh-kube-token";
  meshKubeCaFile = "/var/secrets/losos-mesh-kube-ca.crt";

  # Written by the registrar's reconciler (one entry per enrolled tenant),
  # read by losos-mesh-taint.service. Lives in the registrar's StateDirectory
  # so it survives a restart without a second persistence story.
  computeWindowsFile = "/var/lib/losos-registrar/compute-windows.json";

  # The ServiceAccount the join route authenticates as, and the
  # non-expiring Secret that carries its token. A projected token would be
  # the modern spelling, but it expires and is only mountable into a pod —
  # the registrar is a plain systemd service reading a file, so it needs the
  # legacy `kubernetes.io/service-account-token` Secret.
  meshSaName = "losos-registrar";
  meshSaSecretName = "losos-registrar-token";

  # Applied to a mesh node object while its owner's compute window is shut.
  # NoSchedule rather than NoExecute: a window closing must stop NEW mesh
  # work landing on someone's box, not rip a half-finished pod out from
  # under it at 07:00.
  computeWindowTaint = "losos.dev/compute-window";

  # Agents dial the SUPERVISOR port, not the apiserver's 6443. Handing back
  # the apiserver URL here would produce a node that retries forever with a
  # TLS error nobody on the appliance can read.
  meshServerAddr = "https://${cfg.cluster.advertiseAddr}:${toString cfg.cluster.supervisorPort}";

  # Format a `host:port` socket string for the registrar's `--listen`,
  # bracketing IPv6 literals: `::` → `[::]:8443`, `0.0.0.0` → `0.0.0.0:8443`
  # (parsed as a `SocketAddr`, which rejects an unbracketed IPv6 literal).
  # Only used for the API listen address — the rathole server.toml, seed
  # included, is rendered by the registrar itself (`losos-registrar seed`),
  # so its formatting has a single implementation in config.rs.
  fmtBind =
    addr: port:
    if lib.hasInfix ":" addr then "[${addr}]:${toString port}" else "${addr}:${toString port}";

  # Static Traefik config. We bypass services.traefik.staticConfigOptions
  # because the NixOS module force-merges `providers.file.filename` (a single
  # file); we need `providers.file.directory` so Traefik watches the dir the
  # registrar drops losos.yml into alongside the static register.yml.
  tomlFmt = pkgs.formats.toml { };
  staticCfg = tomlFmt.generate "losos-traefik-static.toml" {
    entryPoints.web.address = ":80";
    entryPoints.web.http.redirections.entryPoint.to = "websecure";
    entryPoints.web.http.redirections.entryPoint.scheme = "https";
    entryPoints.websecure.address = ":443";
    providers.file.directory = "/etc/traefik/dynamic";
    certificatesResolvers.le.acme.email =
      if cfg.acmeEmail == null then "unconfigured@losos.cfd" else cfg.acmeEmail;
    certificatesResolvers.le.acme.storage = "/var/lib/traefik/acme.json";
    certificatesResolvers.le.acme.tlsChallenge = { };
  };

  # The one static route: register.<domain> → the registrar's loopback API.
  # Has a cert before any appliance registers (avoids the register/route
  # chicken-and-egg). The registrar never rewrites this file.
  #
  # Note that this router has no path rule, so it also fronts the mesh join
  # route (POST /cluster/join). That route is therefore on the public
  # internet and authenticates every caller against the same per-appliance
  # token the proxy uses — see backend-registrar/src/server.rs.
  registerYml = pkgs.writeText "losos-register-route.yml" ''
    http:
      routers:
        register:
          rule: "Host(`${registerDomain}`)"
          service: register
          entryPoints:
            - websecure
          tls:
            certResolver: le
            domains:
              - main: "${registerDomain}"
      services:
        register:
          loadBalancer:
            servers:
              - url: "http://127.0.0.1:${toString cfg.registrarApiPort}"
  '';

  # tenants.json: id → {hostname, cluster, token_file}. token_file is a PATH
  # (not the secret), so this file can live in the world-readable store — the
  # secret is the file's *contents*, read by the registrar at reconcile time.
  #
  # This writer hardcodes its attribute set, so a per-tenant option that is
  # not named here is silently dropped and the registrar sees the serde
  # default instead. That is exactly how `cluster` would fail: absent from
  # the JSON it deserializes as `false` and every join answers 403, with no
  # error anywhere to explain why.
  #
  # The Nix attribute name and the JSON key are not guaranteed to match —
  # `tokenFile` is rendered as `token_file` because the Rust struct
  # `TenantEntry` (backend-registrar/src/server.rs) spells its fields in
  # snake_case and takes no serde rename. `hostname` and `cluster` happen to
  # be identical in both languages. Whenever a field is added on either side,
  # the key written here must be the one the struct actually deserializes.
  tenantsJson = pkgs.writeText "losos-registrar-tenants.json" (
    builtins.toJSON (
      lib.mapAttrs (_: v: {
        inherit (v) hostname cluster;
        token_file = toString v.tokenFile;
      }) cfg.tenants
    )
  );

  # RBAC for the registrar's stale-node cleanup, shipped as an rke2
  # auto-deploy manifest so the cluster brings its own credentials up rather
  # than needing an operator with kubectl on first boot.
  #
  # Least privilege, and it is narrower than "get/delete on nodes and
  # secrets" sounds: nodes are cluster-scoped so they need a ClusterRole, but
  # the only Secret the registrar ever touches is
  # <node>.node-password.rke2 in kube-system. A ClusterRole cannot say "in
  # kube-system" — it would hand out every Secret in the cluster, including
  # the mesh agent token itself — so the Secret half is a namespaced Role.
  meshRbacManifest = [
    {
      apiVersion = "v1";
      kind = "ServiceAccount";
      metadata = {
        name = meshSaName;
        namespace = "kube-system";
      };
    }
    {
      apiVersion = "rbac.authorization.k8s.io/v1";
      kind = "ClusterRole";
      metadata.name = meshSaName;
      rules = [
        {
          apiGroups = [ "" ];
          resources = [ "nodes" ];
          verbs = [
            "get"
            "delete"
          ];
        }
      ];
    }
    {
      apiVersion = "rbac.authorization.k8s.io/v1";
      kind = "ClusterRoleBinding";
      metadata.name = meshSaName;
      roleRef = {
        apiGroup = "rbac.authorization.k8s.io";
        kind = "ClusterRole";
        name = meshSaName;
      };
      subjects = [
        {
          kind = "ServiceAccount";
          name = meshSaName;
          namespace = "kube-system";
        }
      ];
    }
    {
      apiVersion = "rbac.authorization.k8s.io/v1";
      kind = "Role";
      metadata = {
        name = meshSaName;
        namespace = "kube-system";
      };
      rules = [
        {
          apiGroups = [ "" ];
          resources = [ "secrets" ];
          verbs = [
            "get"
            "delete"
          ];
        }
      ];
    }
    {
      apiVersion = "rbac.authorization.k8s.io/v1";
      kind = "RoleBinding";
      metadata = {
        name = meshSaName;
        namespace = "kube-system";
      };
      roleRef = {
        apiGroup = "rbac.authorization.k8s.io";
        kind = "Role";
        name = meshSaName;
      };
      subjects = [
        {
          kind = "ServiceAccount";
          name = meshSaName;
          namespace = "kube-system";
        }
      ];
    }
    {
      apiVersion = "v1";
      kind = "Secret";
      type = "kubernetes.io/service-account-token";
      metadata = {
        name = meshSaSecretName;
        namespace = "kube-system";
        annotations."kubernetes.io/service-account.name" = meshSaName;
      };
    }
  ];

  # Extracts the ServiceAccount token and the cluster CA into the two
  # runtime files the registrar reads. Deliberately a poller (see the unit's
  # Restart=on-failure): rke2-server.service reports started well before the
  # apiserver answers, and the token controller populates the Secret's data
  # some time after the manifest is applied, so the first few runs are
  # *expected* to fail. Every failure path here is a non-zero exit rather
  # than a half-written file — a truncated token would make the join route
  # 503 with a Kubernetes 401 in the log and no obvious cause.
  meshRbacScript = pkgs.writeShellScript "losos-mesh-rbac" ''
    set -euo pipefail
    export PATH=${
      lib.makeBinPath [
        pkgs.kubectl
        pkgs.coreutils
      ]
    }
    export KUBECONFIG=${meshKubeconfig}

    # Fails while the apiserver is still coming up, which is the point.
    kubectl -n kube-system get serviceaccount ${meshSaName} >/dev/null

    [ -d /var/secrets ] || install -d -m 0700 /var/secrets
    umask 077

    tok=${meshKubeTokenFile}.tmp
    ca=${meshKubeCaFile}.tmp

    kubectl -n kube-system get secret ${meshSaSecretName} \
      -o jsonpath='{.data.token}' | base64 -d > "$tok"
    [ -s "$tok" ]
    chmod 0600 "$tok"
    mv -f "$tok" ${meshKubeTokenFile}

    kubectl -n kube-system get secret ${meshSaSecretName} \
      -o jsonpath='{.data.ca\.crt}' | base64 -d > "$ca"
    [ -s "$ca" ]
    chmod 0644 "$ca"
    mv -f "$ca" ${meshKubeCaFile}
  '';

  # Applies or removes the compute-window taint on every enrolled mesh node.
  #
  # This runs on the EDGE and not on the appliance because it cannot run on
  # the appliance: the NodeRestriction admission plugin forbids a kubelet
  # from editing its own node's taints, and rke2's --node-taint is fixed for
  # the life of the process. So the box that knows what time it is locally is
  # not the box allowed to act on it — see the timezone note below.
  #
  # Failing open is not an option: an unreadable or malformed window is
  # treated as "closed", so a corrupt state file costs the mesh some capacity
  # instead of quietly scheduling strangers' workloads onto someone's box in
  # the middle of their working day.
  meshTaintScript = pkgs.writeShellScript "losos-mesh-taint" ''
    set -euo pipefail
    export PATH=${
      lib.makeBinPath [
        pkgs.kubectl
        pkgs.jq
        pkgs.coreutils
      ]
    }
    export KUBECONFIG=${meshKubeconfig}

    windows=${computeWindowsFile}
    if [ ! -e "$windows" ]; then
      # The registrar writes this on its first reconcile tick. Before that
      # there is nothing to enforce, and exiting 0 keeps a fresh edge from
      # logging a failed unit every five minutes.
      exit 0
    fi

    # NOTE(timezone): the taint is written HERE, because the edge is the only
    # node the NodeRestriction plugin lets write it — so the comparison happens
    # on a machine that is not the owner's. This edge is a VPS and runs UTC; the
    # appliance ships Europe/Berlin. So the window is evaluated in the zone the
    # APPLIANCE sent with it, not in ours.
    #
    # This used to read the edge's clock and a note here claimed the two "agree
    # by default (both UTC)". That was false for the appliance half, and the
    # cost was not academic: a 23:00-07:00 window entered by an owner in Berlin
    # was enforced 00:00-08:00 in winter and 01:00-09:00 in summer, sliding an
    # hour at each DST change — precisely the "strangers' workloads in the
    # middle of their working day" this script's fail-closed rule exists to
    # prevent. Do not hoist `now` back out of the loop: it is per-node now
    # because the zone is.
    in_window() {
      local s e now tz
      tz="$3"
      case "$1" in [0-9][0-9]:[0-9][0-9]) ;; *) return 1 ;; esac
      case "$2" in [0-9][0-9]:[0-9][0-9]) ;; *) return 1 ;; esac
      # An empty or unresolvable zone makes `date` fall back to UTC silently.
      # The registrar validates the name (window.rs valid_tz) and defaults it to
      # UTC, so an empty value here means a row written before the field
      # existed — which meant UTC anyway.
      [ -n "$tz" ] || tz=UTC
      now=$(( 10#$(TZ="$tz" date +%H) * 60 + 10#$(TZ="$tz" date +%M) ))
      s=$(( 10#''${1%%:*} * 60 + 10#''${1##*:} ))
      e=$(( 10#''${2%%:*} * 60 + 10#''${2##*:} ))
      if [ "$s" -le "$e" ]; then
        [ "$now" -ge "$s" ] && [ "$now" -lt "$e" ]
      else
        # end < start means the window wraps midnight, which is the normal
        # case for "while I sleep".
        [ "$now" -ge "$s" ] || [ "$now" -lt "$e" ]
      fi
    }

    # Wire shape, and the only coupling between this unit and the registrar.
    # It is defined by backend-registrar/src/window.rs, which owns the writer:
    #   { "nodes": [ { "node_name": "mattbox-01", "share_compute": true,
    #                  "window_start": "23:00", "window_end": "07:00",
    #                  "tz": "Europe/Berlin", "idle": true } ] }
    # A list under a named key rather than a bare array or a map: the named key
    # leaves room for a sibling field later, and the list keeps the writer's
    # output byte-stable so its reconciler can skip a no-op rewrite. node_name
    # is the appliance id, which the join route forces them to be equal (gate 3
    # in server.rs). Change the shape on one side only and the taint silently
    # stops tracking the window — nothing here can tell an empty file from a
    # renamed key.
    jq -r '.nodes[]
           | [ .node_name,
               (.share_compute | tostring),
               (.window_start // ""),
               (.window_end // ""),
               (.tz // "UTC"),
               ((.idle // false) | tostring) ]
           | @tsv' "$windows" |
    while IFS=$'\t' read -r node share start end tz idle; do
      # A tenant that has been issued a token but has not finished joining
      # has no node object yet. Skip it rather than failing the whole run.
      if ! kubectl get node "$node" >/dev/null 2>&1; then
        continue
      fi

      # Three conditions, and they are not interchangeable.
      #
      #   share   the owner turned compute sharing on at all
      #   window  the hours they said the box may be lent out
      #   idle    whether they are actually using it right now
      #
      # The window is permission and idle is reality, and the mesh gets the
      # machine only when both hold. Idle can withdraw availability inside a
      # window; it can never grant it outside one, because an owner who set a
      # window meant it and a box being quiet at 14:00 is not consent.
      #
      # `.idle // false` in the jq above is the fail-closed half: a registrar
      # that has heard nothing from this box since it restarted, or whose last
      # report is older than the heartbeat TTL, omits the field or writes
      # false, and the taint stays on. "I could not tell" and "the owner is
      # away" must never be the same answer.
      if [ "$share" = "true" ] && [ "$idle" = "true" ] && in_window "$start" "$end" "$tz"; then
        # `kubectl taint <node> key-` exits non-zero when the taint is not
        # there, which is the steady state for an open window.
        kubectl taint node "$node" ${computeWindowTaint}- >/dev/null 2>&1 || true
      else
        kubectl taint node "$node" \
          ${computeWindowTaint}=closed:NoSchedule --overwrite >/dev/null
      fi
    done
  '';

  # Seed the rathole server config at first boot so rathole can start
  # independently of the registrar. `losos-registrar seed` renders the
  # zero-tenant config through the same desired_config the reconciler uses,
  # so the seed and the registrar's steady-state rewrite are identical by
  # construction (zero churn, no hand-maintained byte-identity). Idempotent:
  # exits untouched when the file already exists — after first boot the
  # running registrar is the writer.
  seedArgs = lib.concatStringsSep " " [
    "${registrar}/bin/losos-registrar"
    "seed"
    "--rathole-config"
    "/etc/rathole/server.toml"
    "--rathole-bind-addr"
    cfg.ratholeBindAddr
    "--rathole-bind-port"
    (toString cfg.ratholeBindPort)
    "--bootstrap-token-file"
    (toString cfg.bootstrapTokenFile)
  ];

  # Mesh half of `serve`. Every one of these is optional in the registrar's
  # parser, and without --mesh-agent-token-file the join route answers 503 —
  # which is what makes a proxy-only edge (losos.edge.cluster.enable = false)
  # keep working with no other change.
  meshServeArgs = lib.optionals meshEnabled [
    "--mesh-agent-token-file"
    (toString cfg.cluster.agentTokenFile)
    "--mesh-server-addr"
    meshServerAddr
    "--kube-api"
    "https://127.0.0.1:${toString cfg.cluster.apiPort}"
    "--kube-token-file"
    meshKubeTokenFile
    "--kube-ca-file"
    meshKubeCaFile
    "--compute-windows-file"
    computeWindowsFile
  ];

  serveArgs = lib.concatStringsSep " " (
    [
      "${registrar}/bin/losos-registrar"
      "serve"
      "--listen"
      "${fmtBind cfg.registrarApiBind cfg.registrarApiPort}"
      "--registry"
      "/var/lib/losos-registrar/registry.json"
      "--traefik-dir"
      "/etc/traefik/dynamic"
      "--rathole-config"
      "/etc/rathole/server.toml"
      "--rathole-bind-addr"
      cfg.ratholeBindAddr
      "--rathole-bind-port"
      (toString cfg.ratholeBindPort)
      "--port-range"
      cfg.ratholePortRange
      "--bootstrap-token-file"
      (toString cfg.bootstrapTokenFile)
      "--tenants-file"
      (toString tenantsJson)
      "--reconcile-interval"
      cfg.reconcileInterval
      "--heartbeat-ttl"
      cfg.heartbeatTtl
    ]
    ++ meshServeArgs
  );
in
{
  config = lib.mkIf enabled {
    # The edge module is only imported by the edge system, so wiring the
    # registrar package here (gated on `enabled`, which is always true for the
    # edge system) keeps edge.nix self-contained — mirrors defaults.nix wiring
    # losos.backend.package on the appliance.
    losos.edge.registrar.package = self.packages.x86_64-linux.losos-registrar;

    assertions = [
      {
        assertion = cfg.acmeEmail != null;
        message = "losos.edge.enable requires losos.edge.acmeEmail (Let's Encrypt account email).";
      }
      {
        assertion = meshEnabled -> cfg.cluster.advertiseAddr != "";
        message = ''
          losos.edge.cluster.enable requires losos.edge.cluster.advertiseAddr.
          It is what appliances are told to dial and what goes into the
          server's TLS SAN list; a control plane behind NAT that advertises a
          private address is unjoinable, and it fails at the agent, on a box
          with no shell to read the error from.
        '';
      }
      # rke2's own server config reference documents neither --https-listen-port
      # nor --supervisor-port (https://docs.rke2.io/reference/server_config):
      # the apiserver is pinned to 6443 and the supervisor to 9345. So these two
      # options select what the FIREWALL opens and what the join route
      # advertises, and nothing else. Left free they would open a port nothing
      # listens on while agents kept dialling 9345 — a cluster that never forms,
      # with a firewall that looks correct.
      {
        assertion = meshEnabled -> cfg.cluster.apiPort == 6443;
        message = ''
          losos.edge.cluster.apiPort must stay 6443: rke2 has no flag to move
          the apiserver, so any other value only mis-programs the firewall and
          the registrar's --kube-api.
        '';
      }
      {
        assertion = meshEnabled -> cfg.cluster.supervisorPort == 9345;
        message = ''
          losos.edge.cluster.supervisorPort must stay 9345: rke2 has no flag to
          move the supervisor, so any other value only mis-programs the
          firewall and the serverAddr handed to joining appliances.
        '';
      }
    ];

    # ── Traefik (master proxy) ───────────────────────────────────────────
    services.traefik = {
      enable = true;
      staticConfigFile = staticCfg;
      group = "traefik";
    };

    # /etc/traefik/dynamic: real dir (0755, root) holding the static
    # register.yml (symlinked from the store) + the registrar's runtime
    # losos.yml. Traefik's file provider reads both.
    systemd.tmpfiles.rules = [
      "d /etc/traefik/dynamic 0755 root root - -"
      "L+ /etc/traefik/dynamic/register.yml - - - - ${toString registerYml}"
    ];

    # ── losos-registrar (registration API + reconciler) ─────────────────
    systemd.services.losos-registrar = {
      description = "losos edge registrar — registration API + Traefik/rathole config reconciler";
      wantedBy = [ "multi-user.target" ];
      after = [
        "network-online.target"
        "losos-rathole-seed.service"
      ]
      ++ lib.optional meshEnabled "losos-mesh-rbac.service";
      # Wants, not Requires, on the RBAC extractor: the master-proxy half must
      # keep serving /register and rewriting Traefik on an edge whose mesh
      # apiserver is down or not yet up. A failed extraction costs the join
      # route (503) and nothing else.
      wants = [ "network-online.target" ] ++ lib.optional meshEnabled "losos-mesh-rbac.service";
      serviceConfig = {
        ExecStart = serveArgs;
        StateDirectory = "losos-registrar";
        StateDirectoryMode = "0700";
        Restart = "always";
        RestartSec = 5;
        # Writes /etc/traefik/dynamic + /etc/rathole; reads 0600 token files.
        # Runs as root; deliberately no ProtectSystem (would make /etc RO).
        PrivateTmp = true;
        NoNewPrivileges = true;
      };
    };

    # ── rathole server config seed ───────────────────────────────────────
    # Writes the zero-tenant server.toml so rathole can start before the
    # registrar. Idempotent (only when server.toml is absent); the registrar
    # rewrites the file afterward, so this is a first-boot seed only.
    systemd.services.losos-rathole-seed = {
      description = "losos rathole server config seed (zero-tenant server.toml)";
      wantedBy = [ "multi-user.target" ];
      before = [
        "losos-rathole.service"
        "losos-registrar.service"
      ];
      serviceConfig = {
        ExecStart = seedArgs;
        Type = "oneshot";
        RemainAfterExit = true;
        PrivateTmp = true;
      };
    };

    # ── rathole server (tunnel endpoint) ─────────────────────────────────
    # Starts from the seeded [server] base (losos-rathole-seed), independent
    # of the registrar. The registrar rewrites server.toml to add
    # [server.services.*]; rathole's `notify` file-watcher hot-reloads the
    # config the instant it's rewritten (atomic temp+rename) — no signal
    # needed. SIGHUP would kill rathole 0.5 (no handler) and Restart=always
    # would resurrect it ~5s later, so the registrar sends nothing.
    systemd.services.losos-rathole = {
      description = "losos rathole server — master-proxy tunnel endpoint";
      wantedBy = [ "multi-user.target" ];
      after = [
        "network-online.target"
        "losos-rathole-seed.service"
      ];
      requires = [ "losos-rathole-seed.service" ];
      wants = [ "network-online.target" ];
      serviceConfig = {
        ExecStart = "${rathole}/bin/rathole /etc/rathole/server.toml";
        Restart = "always";
        RestartSec = 5;
        PrivateTmp = true;
      };
    };

    # ── Mesh control plane (rke2 server) ─────────────────────────────────
    # The appliance runs the SAME nixpkgs module with role = "agent"
    # (modules/cluster.nix) — one name-parameterized generator, two
    # instances. Everything set here is server-only by rke2's own role
    # documentation: an agent given `cni`, `disable` or `agentTokenFile`
    # warns or hard-errors, so none of it may be copied across.
    #
    # agentTokenFile is provisioned out of band. If it is missing, rke2-server
    # refuses to start and the whole mesh half stays down — loudly, in the
    # journal — rather than generating a token nobody can hand to an appliance.
    services.rke2 = lib.mkIf meshEnabled {
      enable = true;
      role = "server";
      package = cfg.cluster.package;
      agentTokenFile = cfg.cluster.agentTokenFile;
      cni = cfg.cluster.cni;
      # Appliances reach this address from the public internet; without it the
      # server advertises whatever the VPS calls its primary interface, which
      # on most providers is a private address behind a 1:1 NAT.
      nodeExternalIP = cfg.cluster.advertiseAddr;
      extraFlags = [ "--tls-san=${cfg.cluster.advertiseAddr}" ];

      # Longhorn is not packaged in nixpkgs, so it is deployed as a pinned
      # Helm chart. `autoDeployCharts` fetches the .tgz at build time into a
      # fixed-output derivation, so the deployment stays reproducible instead
      # of resolving a chart repo at 03:00. Null (the default) means the mesh
      # runs without distributed storage — a valid state, not a broken one.
      autoDeployCharts = lib.optionalAttrs (cfg.cluster.longhornChart != null) {
        longhorn = cfg.cluster.longhornChart;
      };

      manifests.losos-registrar-rbac.content = meshRbacManifest;
    };

    # Longhorn prerequisites for THIS node. Every node that stores a replica
    # needs an iSCSI initiator and NFS client tooling — the manager attaches
    # volumes over iSCSI and the default backup target speaks NFS. The
    # appliance carries its own copy of this in modules/cluster.nix; it is
    # not shared, because the two nodes reach it through different modules
    # and the appliance also needs kernel modules the edge does not.
    #
    # services.openiscsi already pulls in the iscsi_tcp kernel module and puts
    # iscsiadm on the system PATH, which is what Longhorn's environment check
    # looks for when it nsenters into PID 1's mount namespace.
    services.openiscsi = lib.mkIf (meshEnabled && cfg.cluster.longhornChart != null) {
      enable = true;
      # Required (no default). Must be unique per node: two initiators sharing
      # an IQN corrupt each other's sessions.
      name = "iqn.2026-09.dev.losos:edge-${cfg.publicDomain}";
    };

    # kubectl is not in the rke2 derivation — it ships bin/rke2 and
    # bin/rke2-killall.sh and nothing else — so the mesh units below reference
    # it by store path. Listing it here as well is for the operator: an edge
    # whose cluster cannot be inspected from a shell is one nobody can debug.
    environment.systemPackages = lib.optionals meshEnabled (
      [ pkgs.kubectl ] ++ lib.optional (cfg.cluster.longhornChart != null) pkgs.nfs-utils
    );

    # ── Mesh: registrar credentials for the apiserver ────────────────────
    systemd.services.losos-mesh-rbac = lib.mkIf meshEnabled {
      description = "losos mesh RBAC — extract the registrar's ServiceAccount token and cluster CA";
      wantedBy = [ "multi-user.target" ];
      after = [ "rke2-server.service" ];
      requires = [ "rke2-server.service" ];
      before = [
        "losos-registrar.service"
        "losos-mesh-taint.service"
      ];
      # The default start-rate limit (5 starts in 10s) would give up roughly
      # 40 seconds into a poll that routinely needs minutes — the apiserver
      # has to come up AND the token controller has to fill in the Secret.
      # Disabling the limit is what makes Restart=on-failure a poller instead
      # of a unit that dies quietly during the first boot it was written for.
      unitConfig.StartLimitIntervalSec = 0;
      serviceConfig = {
        ExecStart = meshRbacScript;
        Type = "oneshot";
        RemainAfterExit = true;
        Restart = "on-failure";
        RestartSec = 10;
        UMask = "0077";
        PrivateTmp = true;
        NoNewPrivileges = true;
      };
    };

    # ── Mesh: compute-window enforcement ─────────────────────────────────
    # Five minutes is the granularity the window is enforced at, which is why
    # the SPA offers HH:MM and not seconds. Persistent = false on purpose: a
    # missed run while the edge was down must not fire a catch-up that
    # evaluates a window against the wrong wall clock.
    systemd.services.losos-mesh-taint = lib.mkIf meshEnabled {
      description = "losos mesh compute-window taint reconciler";
      after = [ "losos-mesh-rbac.service" ];
      serviceConfig = {
        ExecStart = meshTaintScript;
        Type = "oneshot";
        PrivateTmp = true;
        NoNewPrivileges = true;
      };
    };

    systemd.timers.losos-mesh-taint = lib.mkIf meshEnabled {
      description = "losos mesh compute-window taint reconciler timer";
      wantedBy = [ "timers.target" ];
      timerConfig = {
        OnCalendar = "*:0/5";
        Persistent = false;
        AccuracySec = "1min";
        Unit = "losos-mesh-taint.service";
      };
    };

    # ── Firewall: public web + the rathole client-dial port ──────────────
    # The registrar API is loopback-only in production (Traefik fronts it at
    # register.<publicDomain>); only open it in the firewall when it is bound
    # off-loopback — which the option docs say is a test-only direct-dial
    # posture, never a deployment one.
    #
    # The mesh adds exactly two: the apiserver and rke2's supervisor. Agents
    # dial the supervisor to register and then talk to the apiserver, so
    # opening only 6443 produces a cluster that never forms. Nothing else the
    # mesh runs is reachable from off-box.
    networking.firewall.allowedTCPPorts = [
      80
      443
      cfg.ratholeBindPort
    ]
    ++ lib.optional (cfg.registrarApiBind != "127.0.0.1") cfg.registrarApiPort
    ++ lib.optionals meshEnabled [
      cfg.cluster.apiPort
      cfg.cluster.supervisorPort
    ];
  };
}
