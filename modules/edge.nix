# Master proxy + mesh control plane — edge side.
#
# The edge lives on a public VPS; a VPS flake imports it via the
# `nixosModules.edge` flake output next to its own hardware config. It runs
# two halves that share one binary and one tenant whitelist:
#
#   * the MASTER PROXY — Traefik (public TLS on :443 over TCP, and HTTP/3
#     over QUIC on UDP :443), the tunnel server (boxes dial :2333, QUIC on
#     UDP or TLS on TCP), and losos-registrar (serve), the Rust HTTP server
#     Traefik discovers the boxes from (its HTTP provider polls the
#     registrar's loopback provider listener) and that writes the tunnel's
#     file from a tenant registry.
#   * the MESH CONTROL PLANE (`losos.edge.cluster.enable`) — an rke2 server
#     that appliances join as agents, carrying Longhorn and mesh compute.
#
# The two halves are deliberately separable: an edge with
# `losos.edge.cluster.enable = false` is exactly the proxy-only edge that
# shipped before the mesh existed, and every mesh unit here is gated so that
# stays true. Nothing in the proxy path may grow a hard dependency on the
# cluster path — an apiserver that is down must not take the tunnels with it.
#
# Traefik has two providers:
#   * file, watching /etc/traefik/dynamic: register.yml, the one static route
#     (register.<domain> → the registrar's loopback API), and its siblings.
#     NixOS-managed (this module and edge-inference.nix), symlinked in.
#   * http, polling http://127.0.0.1:<traefikProviderPort>/traefik: one
#     router per live box name and published machine, decided by the
#     registrar's reconciler and held in its memory. Traefik keeps the routes
#     it has whenever the registrar does not answer 200.
# The registrar never touches the file provider's directory. That separation
# keeps it from being able to lock itself out of the control plane. Edges
# from before the HTTP provider had the registrar write losos.yml and
# losos-vms.yml into that directory; tmpfiles removes both on the first boot
# of this generation, or Traefik would route every box twice.
#
# See docs/superpowers/specs/2026-08-17-master-proxy-design.md and
# docs/superpowers/specs/2026-09-09-k3s-mesh-design.md. The appliance side is
# modules/proxy.nix (proxy) and modules/cluster.nix (mesh).
#
# Transport: the tunnel (edge/registrar/src/tunnel.rs, `losos-registrar
# tunnel-server`) is QUIC, TLS 1.3 under a certificate the edge makes on its
# first start in /var/lib/losos-tunnel and keeps. Boxes pin its SHA-256,
# served by the registrar at /tunnel-pin, and prove their tenant token per
# service. It carries one QUIC stream per connection: the box's own TLS for
# a box that ends TLS itself (the default, edge/registrar/src/passthrough.rs,
# which this edge cannot read), plain HTTP after Traefik's TLS for an older
# box. Where UDP is blocked a box reaches the same port over TCP
# (TLS, then HTTP/2 streams).
{
  config,
  lib,
  pkgs,
  self,
  utils,
  ...
}:

let
  cfg = config.losos.edge;
  enabled = cfg.enable;
  registrar = cfg.registrar.package;

  registerDomain = "register.${cfg.publicDomain}";

  # The boxes on Tor and I2P (modules/edge-hidden.nix).
  hidden = import ./hidden-lib.nix { inherit lib pkgs; };
  hiddenOn = cfg.hidden.tor.enable || cfg.hidden.i2p.enable;
  hiddenArgs = lib.optionals hiddenOn [
    "--hidden-dir"
    "/var/lib/losos-registrar/hidden"
    "--hidden-addresses"
    hidden.addressesFile
    "--hidden-http-port"
    (toString hidden.edgePlainPort)
    "--hidden-https-port"
    (toString hidden.edgeTlsPort)
  ];

  # The edge as a way out (modules/edge-exit.nix). The paths are that
  # module's; the registrar only writes the peers and reads the two files.
  exitCfg = cfg.exit;
  exitArgs = lib.optionals exitCfg.enable (
    [
      "--exit-dir"
      "/var/lib/losos-exit"
      "--exit-subnet"
      exitCfg.subnet
      "--exit-endpoint"
      exitCfg.endpoint
      "--exit-public-key-file"
      "/var/lib/losos-exit/public.key"
    ]
    ++ lib.optionals exitCfg.onion.enable [
      "--exit-onion-file"
      "/var/lib/tor/onion/losos-exit/hostname"
      "--exit-onion-port"
      (toString exitCfg.onion.port)
    ]
  );

  # ── Mesh control plane constants ────────────────────────────────────────
  meshEnabled = cfg.cluster.enable;
  # The registrar API answers off-box when it is bound to a non-loopback
  # address, or when the edge advertises itself on the LAN (which rebinds it).
  registrarOffBox = cfg.registrarApiBind != "127.0.0.1" || cfg.lan.advertise;

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

  # The backup door on bought storage (edge/registrar/src/server/storage.rs):
  # restic's REST server, started by the registrar on a buyer's volume on
  # this node. Its image rides in rke2's airgap folder, so nothing is pulled.
  backupsOn = cfg.market.enable && meshEnabled && cfg.market.backups.enable;
  restServerImage = (import ../nix/images.nix { inherit pkgs; }).losos-image-rest-server;
  restServerRef = "${restServerImage.imageName}:${restServerImage.imageTag}";
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
  # Used for the API and tunnel listen addresses; the per-box binds in the
  # tunnel's file are rendered by the registrar itself, so their formatting
  # has a single implementation in config.rs.
  fmtBind =
    addr: port:
    if lib.hasInfix ":" addr then "[${addr}]:${toString port}" else "${addr}:${toString port}";

  # Static Traefik config. We bypass services.traefik.staticConfigOptions
  # because the NixOS module force-merges `providers.file.filename` (a single
  # file); we need `providers.file.directory` so Traefik watches the dir the
  # static register*.yml files are linked into.
  #
  # websecure answers HTTP/3 on UDP 443 beside TCP, and Traefik adds an
  # Alt-Svc header to every response so a browser moves to QUIC on its next
  # request; a network that blocks UDP keeps HTTP/2 on TCP.
  #
  # Traefik's Prometheus counters are on a loopback entrypoint of their own,
  # read by the registrar for the manage page's Statistics (crate::stats). The
  # buckets are finer than Traefik's four so a 95th percentile means
  # something; the metrics entrypoint's own requests are not counted.
  tomlFmt = pkgs.formats.toml { };
  staticCfg = tomlFmt.generate "losos-traefik-static.toml" (
    lib.recursiveUpdate
      {
        entryPoints.web.address = ":80";
        entryPoints.web.http.redirections.entryPoint.to = "websecure";
        entryPoints.web.http.redirections.entryPoint.scheme = "https";
        # The redirect is a router Traefik makes on `web`, by default at the
        # highest priority, which would take a box's Let's Encrypt HTTP-01
        # requests too. A box that ends TLS itself gets its own certificate
        # that way (edge/registrar/src/passthrough.rs), so the registrar puts
        # a router for that one path on `web`, and the redirect takes
        # everything else at the lowest priority.
        entryPoints.web.http.redirections.entryPoint.priority = 1;
        entryPoints.websecure.address = ":443";
        entryPoints.websecure.http3 = { };
        entryPoints.metrics.address = "127.0.0.1:${toString cfg.traefikMetricsPort}";
        metrics.prometheus = {
          entryPoint = "metrics";
          addEntryPointsLabels = true;
          addRoutersLabels = false;
          addServicesLabels = true;
          buckets = [
            0.005
            0.01
            0.025
            0.05
            0.1
            0.25
            0.5
            1.0
            2.5
            5.0
            10.0
          ];
        };
        providers.file.directory = "/etc/traefik/dynamic";
        providers.http = {
          endpoint = "http://127.0.0.1:${toString cfg.traefikProviderPort}/traefik";
          pollInterval = "2s";
          pollTimeout = "2s";
        };
        certificatesResolvers.le.acme.email =
          if cfg.acmeEmail == null then "unconfigured@losos.cfd" else cfg.acmeEmail;
        certificatesResolvers.le.acme.storage = "/var/lib/traefik/acme.json";
        certificatesResolvers.le.acme.tlsChallenge = { };
      }
      # A disk image upload is gigabytes, and Traefik v3 cuts any request
      # whose body takes longer than the entry point's readTimeout (60 s by
      # default). Raised only on an edge that takes uploads; the registrar's
      # own idle timeout (server/vm.rs) still drops a client that stops
      # sending.
      (
        lib.recursiveUpdate
          (lib.optionalAttrs cfg.vms.enable {
            entryPoints.websecure.transport.respondingTimeouts.readTimeout = "6h";
          })
          # Where Tor and I2P deliver the boxes' addresses
          # (modules/edge-hidden.nix): loopback only, no redirect to https,
          # and only the registrar's routers for those names.
          (
            lib.optionalAttrs hiddenOn {
              entryPoints.losos-hidden.address = "127.0.0.1:${toString hidden.edgePlainPort}";
              entryPoints.losos-hidden-tls.address = "127.0.0.1:${toString hidden.edgeTlsPort}";
            }
          )
      )
  );

  # The one static route: register.<domain> → the registrar's loopback API.
  # Has a cert before any appliance registers (avoids the register/route
  # chicken-and-egg). The registrar never rewrites this file.
  #
  # Note that this router has no path rule, so it also fronts the mesh join
  # route (POST /cluster/join). That route is therefore on the public
  # internet and authenticates every caller against the same per-appliance
  # token the proxy uses — see edge/registrar/src/server.rs.
  # The two middlewares are what keeps one anonymous client from taking the
  # registrar's whole in-flight budget. Every route there shares one pool of
  # 64 permits, taken before the body is read, so 64 connections that send
  # headers and then trickle (or withhold) a body used to hold all of them
  # and answer every tenant's /heartbeat with 503. `buffering` makes Traefik
  # read the whole body before it forwards anything, so a slow body costs a
  # Traefik goroutine and not a registrar permit; its cap matches the
  # registrar's largest (MAX_WEBHOOK_BYTES, the Stripe webhook). `inFlightReq`
  # then holds any one source address to a handful of requests at once,
  # keyed by the remote address Traefik itself saw. The registrar also stops
  # pruning while it is shedding (reconcile_once in server.rs), so what a
  # many-address flood still manages is a 503, not every tenant's routes gone.
  registerYml = pkgs.writeText "losos-register-route.yml" ''
    http:
      middlewares:
        register-buffer:
          buffering:
            maxRequestBodyBytes: 262144
        register-per-client:
          inFlightReq:
            amount: 8
            sourceCriterion:
              ipStrategy:
                depth: 0
      routers:
        register:
          rule: "Host(`${registerDomain}`)"
          service: register
          middlewares:
            - register-per-client
            - register-buffer
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

  # The two machine image transfers (edge/registrar/src/server/vm.rs)
  # cannot go through `register-buffer`: a disk image is gigabytes, and
  # buffering it would hold all of it in Traefik first. This router takes
  # the two path prefixes ahead of `register` (the longer rule wins) and
  # keeps the per-client cap from register.yml (one file provider, one
  # namespace); the registrar holds them to four transfers at once, its
  # upload size limit and an idle timeout of its own. A box's backup on the
  # storage it bought (server/storage.rs) sends 16 MiB packs, over the
  # buffer's 256 KiB, so it takes this router too, with the registrar's own
  # cap of eight requests and 64 MiB a body.
  vmTransferYml = (pkgs.formats.yaml { }).generate "losos-register-vm-transfer.yml" {
    http.routers.register-vm-transfer = {
      rule = "Host(`${registerDomain}`) && (PathPrefix(`/market/vm-images/upload/`) || PathPrefix(`/market/vm-images/fetch/`) || PathPrefix(`/market/storage/`))";
      service = "register";
      middlewares = [ "register-per-client" ];
      entryPoints = [ "websecure" ];
      tls = {
        certResolver = "le";
        domains = [ { main = registerDomain; } ];
      };
    };
  };

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
  # `TenantEntry` (edge/registrar/src/server.rs) spells its fields in
  # snake_case and takes no serde rename. `hostname` and `cluster` happen to
  # be identical in both languages. Whenever a field is added on either side,
  # the key written here must be the one the struct actually deserializes.
  tenantsJson = pkgs.writeText "losos-registrar-tenants.json" (
    builtins.toJSON (
      lib.mapAttrs (_: v: {
        inherit (v)
          hostname
          cluster
          market
          exit
          ;
        token_file = toString v.tokenFile;
        relay_zone = v.relayZone;
      }) cfg.tenants
    )
  );

  # ── Federation (handbook/docs/in-depth/edge-federation.md) ───────────────
  # Where TOFU-enrolled boxes are kept (losos.edge.lan.openEnrolment): a
  # second whitelist-shaped file plus one token file per box, under the
  # registrar's StateDirectory so it survives a restart like the registry.
  enrolDir = "/var/lib/losos-registrar/enrolled";
  enrolArgs = lib.optionals cfg.lan.openEnrolment [
    "--enrol-dir"
    enrolDir
  ];

  # The uplink target the registrar reads (`relay::UplinkFile`): paths, never
  # secrets, so the rendered copy may live in the store. A `configFile`
  # points at a runtime path instead; the registrar re-reads either on every
  # pass and treats a missing file as "no uplink yet".
  uplinkRendered = pkgs.writeText "losos-uplink.json" (
    builtins.toJSON {
      registrar_url = cfg.uplink.registrarUrl;
      tunnel_endpoint = cfg.uplink.tunnelEndpoint;
      inherit (cfg.uplink) id;
      token_file = toString cfg.uplink.tokenFile;
      pin_file = toString cfg.uplink.pinFile;
    }
  );
  uplinkFile =
    if cfg.uplink.configFile != null then cfg.uplink.configFile else toString uplinkRendered;
  uplinkTunnelConfig = "/etc/losos-tunnel/uplink.json";
  uplinkArgs = lib.optionals cfg.uplink.enable [
    "--uplink-file"
    uplinkFile
    "--uplink-tunnel-config"
    uplinkTunnelConfig
    "--uplink-interval"
    cfg.uplink.interval
  ];

  # Clusters and bans (handbook/docs/in-depth/edge-federation.md, "Three
  # tiers"). The cluster file is runtime state (the manage page writes it), the
  # registrar re-reads it every pass; the official cluster and its peers are
  # configuration; the ban file is re-read on every uplink pass and a missing
  # one bans nothing.
  secondaryArgs = lib.optionals cfg.secondary.enable [
    "--secondary-file"
    cfg.secondary.file
    "--secondary-self-url"
    cfg.lan.url
    "--secondary-self-tunnel"
    cfg.lan.tunnelEndpoint
    "--secondary-interval"
    cfg.secondary.interval
  ];
  officialArgs =
    lib.optionals (cfg.official.cluster != null) [
      "--official-cluster"
      cfg.official.cluster
    ]
    ++ lib.concatMap (p: [
      "--official-peer"
      "${p.cluster},${p.url},${p.tunnel}"
    ]) cfg.official.peers
    ++ [
      "--bans-file"
      cfg.bansFile
    ];

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
      ]
      # Market fulfilment (edge/registrar/src/market.rs): a paid storage
      # order becomes a PersistentVolumeClaim in a `market-<buyer>` namespace.
      # Neither can be scoped to those namespaces — a namespace does not exist
      # before it is created, and a ClusterRole cannot say "namespaces named
      # market-*" — so these are create/get only, never update or delete, and
      # present only when the market is on.
      ++ lib.optionals cfg.market.enable [
        {
          apiGroups = [ "" ];
          resources = [
            "namespaces"
            "persistentvolumeclaims"
          ];
          verbs = [
            "get"
            "create"
          ];
        }
      ]
      # The backup door (edge/registrar/src/server/storage.rs): one
      # rest-server Deployment, a Service and the Secret with its one
      # password per bought volume that a box backs up to, in the buyer's
      # namespace. Create and get only, and never a read of a Secret.
      ++ lib.optionals backupsOn [
        {
          apiGroups = [ "apps" ];
          resources = [ "deployments" ];
          verbs = [
            "get"
            "create"
          ];
        }
        {
          apiGroups = [ "" ];
          resources = [ "services" ];
          verbs = [
            "get"
            "create"
          ];
        }
        {
          apiGroups = [ "" ];
          resources = [ "secrets" ];
          verbs = [ "create" ];
        }
      ]
      # Machine fulfilment (edge/registrar/src/server/vm.rs): one
      # VirtualMachine per paid replica and a Service in front, in the same
      # `market-<buyer>` namespaces. `patch` is the halt of a lapsed order
      # (runStrategy only) and `list` the buyer's status page; still nothing
      # that deletes.
      ++ lib.optionals cfg.vms.enable [
        {
          apiGroups = [ "kubevirt.io" ];
          resources = [ "virtualmachines" ];
          verbs = [
            "get"
            "list"
            "create"
            "patch"
          ];
        }
        {
          apiGroups = [ "" ];
          resources = [ "services" ];
          verbs = [
            "get"
            "create"
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
    # It is defined by edge/registrar/src/window.rs, which owns the writer:
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

  # The tunnel's state: its certificate, key and pin (the registrar serves
  # the pin at /tunnel-pin), and the file the registrar writes and the
  # tunnel server reads.
  tunnelStateDir = "/var/lib/losos-tunnel";
  tunnelConfig = "/etc/losos-tunnel/server.json";

  # ExecStart lines are built with escapeSystemdExecArgs, not by joining with
  # spaces. Several arguments are operator strings (the bind address, the port
  # range, the return URL): systemd would expand a `%` in one as a specifier
  # (a return URL with `%2F` in it reached the registrar changed), and split
  # one with a space into two arguments.
  tunnelArgs = utils.escapeSystemdExecArgs [
    "${registrar}/bin/losos-registrar"
    "tunnel-server"
    "--listen"
    (fmtBind cfg.tunnelBindAddr cfg.tunnelBindPort)
    "--identity-dir"
    tunnelStateDir
    "--config"
    tunnelConfig
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

  # The Stripe key lives in a unit of its own, `losos-stripe-gate`
  # (`losos-registrar stripe-gate`), and nowhere else. The registrar — the
  # internet-facing process, which also carries the master proxy — never holds
  # it: it asks the gate over a Unix socket for the few things it may need
  # (create an account carrying the box UUID, tag, check or link one, start a
  # checkout, verify a webhook) and the gate refuses anything outside the
  # operator's limits (destination account shape, currency, fee ceiling,
  # session lifetime). The blobs are sealed with systemd-creds and handed to
  # the gate by LoadCredentialEncrypted=, so they are plaintext only in that
  # unit's private credential tmpfs. A missing blob skips the gate
  # (ConditionPathExists) and the registrar answers 503 for /market/*; since
  # the gate is its own unit, a blob that will not decrypt cannot touch the
  # proxy, which is why the older ExecStartPre dance is gone.
  gateSocket = "/run/losos-stripe-gate/gate.sock";
  gateSealed = [
    (toString cfg.market.stripeSecretKeySealed)
    (toString cfg.market.webhookSecretSealed)
  ];

  # The hardware the edge sells, read by both the registrar and the gate.
  # The Stripe gate runs only for an edge that takes cards; a coins-only
  # edge (losos.edge.market.cards.enable = false) has no Stripe account.
  gateOn = cfg.market.enable && cfg.market.cards.enable;
  hardwareOn = cfg.market.enable && cfg.market.hardware.enable;
  hardwareCatalogue = pkgs.writeText "losos-hardware.json" (
    builtins.toJSON {
      inherit (cfg.market) currency;
      inherit (cfg.market.hardware) countries;
      items = lib.mapAttrsToList (sku: i: {
        inherit sku;
        inherit (i) name detail;
        unit_amount = i.unitAmount;
      }) cfg.market.hardware.items;
    }
  );
  hardwareArgs =
    flag:
    lib.optionals hardwareOn [
      flag
      "${hardwareCatalogue}"
    ];

  # Market half of `serve`. Without --market-gate-socket every /market/*
  # route answers 503, so an edge that leaves the market off parses exactly the
  # arguments it always did.
  marketServeArgs =
    lib.optionals gateOn [
      "--market-gate-socket"
      gateSocket
    ]
    ++ lib.optionals (cfg.market.enable && !cfg.market.cards.enable) [ "--market-no-cards" ]
    ++ lib.optionals (cfg.market.enable && cfg.market.directPay) [ "--market-direct-pay" ]
    ++ lib.optionals cfg.market.enable [
      "--market-state-file"
      "/var/lib/losos-registrar/market.json"
      "--market-fee-bps"
      (toString cfg.market.feeBps)
      "--market-coin-fee-limit"
      (toString cfg.market.coinFeeLimit)
      "--market-offline-refund-after"
      (toString cfg.market.offlineRefundAfter)
      "--market-currency"
      cfg.market.currency
      "--market-storage-class"
      cfg.market.storageClass
    ]
    ++ lib.optionals backupsOn [
      "--market-storage-image"
      restServerRef
    ]
    ++ lib.optionals (cfg.market.enable && cfg.market.returnUrl != "") [
      "--market-return-url"
      cfg.market.returnUrl
    ]
    ++ hardwareArgs "--market-hardware-catalogue"
    ++ vmServeArgs
    ++ usdcServeArgs
    ++ btcServeArgs
    ++ xmrServeArgs
    ++ zecServeArgs;

  # Bitcoin (losos.edge.market.bitcoin): the edge's own node, on loopback,
  # signed in with the cookie it writes into its chain's directory. The
  # registrar runs as root and reads the cookie on every call, because the
  # node writes a new one each time it starts.
  btcOn = usdcOn && cfg.market.bitcoin.enable;
  btcChain = cfg.market.bitcoin.chain;
  btcRpcPort = 8332;
  btcDataDir = "/var/lib/bitcoind-losos";
  btcCookie =
    if btcChain == "main" then "${btcDataDir}/.cookie" else "${btcDataDir}/${btcChain}/.cookie";
  btcServeArgs = lib.optionals btcOn [
    "--market-btc-rpc"
    "http://127.0.0.1:${toString btcRpcPort}"
    "--market-btc-cookie-file"
    btcCookie
  ];

  # Monero (losos.edge.market.monero): the edge's own pruned node and a
  # wallet service beside it, both on loopback. The wallet service holds the
  # sellers' view-only wallets and signs in with a `user:password` it writes
  # on its first start; the registrar runs as root and reads the same file.
  xmrOn = usdcOn && cfg.market.monero.enable;
  xmrNet = cfg.market.monero.network;
  xmrDaemonPort = 18081;
  xmrWalletPort = 18083;
  xmrWalletDir = "/var/lib/losos-monero-wallet";
  xmrLogin = "${xmrWalletDir}/rpc-login";
  xmrServeArgs = lib.optionals xmrOn [
    "--market-xmr-wallet-rpc"
    "http://127.0.0.1:${toString xmrWalletPort}"
    "--market-xmr-login-file"
    xmrLogin
    "--market-xmr-daemon-rpc"
    "http://127.0.0.1:${toString xmrDaemonPort}"
  ];

  # Zcash (losos.edge.market.zcash): no node on the edge. The registrar asks
  # the public lightwalletd servers for compact blocks and decrypts them
  # itself, so the sellers' viewing keys stay here.
  zecOn = usdcOn && cfg.market.zcash.enable;
  zecNet =
    {
      mainnet = "main";
      testnet = "test";
      regtest = "regtest";
    }
    .${cfg.market.zcash.network};
  zecServeArgs = lib.optionals zecOn (
    lib.concatMap (url: [
      "--market-zec-server"
      url
    ]) cfg.market.zcash.servers
    ++ [
      "--market-zec-network"
      zecNet
    ]
  );

  # USDC (losos.edge.market.usdc). The RPC URL is a systemd credential of
  # the registrar, not a flag: a flag would put the provider key in the
  # unit's command line.
  usdcOn = cfg.market.enable && cfg.market.usdc.enable;
  # The registrar's own table (edge/registrar/src/stablecoin.rs); it
  # refuses anything else at start, this says so at eval.
  usdcKnown = [
    "base"
    "base-sepolia"
    "arbitrum"
    "arbitrum-sepolia"
    "optimism"
    "optimism-sepolia"
    "solana"
    "solana-devnet"
  ];
  usdcNetworks = cfg.market.usdc.networks;
  credDir = "/run/credentials/losos-registrar.service";
  usdcSpec =
    name: n:
    lib.concatStringsSep "," (
      [
        "network=${name}"
        "rpc=${credDir}/usdc-rpc-${name}"
        "treasury=${n.treasury}"
      ]
      ++ lib.optional (n.pay != null) "pay=${n.pay}"
      ++ lib.optional (n.genesisHash != null) "genesis=${n.genesisHash}"
    );
  ensOn = usdcOn && cfg.market.usdc.ensRpcUrlSealed != null;
  # Payments wait in an escrow (LososEscrow on EVM, losos-escrow on Solana)
  # until the registrar releases them with this key. A Solana network with no
  # `pay` is a direct transfer and needs none.
  releaserOn = usdcOn && lib.any (n: n.pay != null) (lib.attrValues usdcNetworks);
  usdcServeArgs =
    lib.optionals usdcOn (
      lib.concatLists (
        lib.mapAttrsToList (name: n: [
          "--market-usdc"
          (usdcSpec name n)
        ]) usdcNetworks
      )
    )
    ++ lib.optionals releaserOn [
      "--market-usdc-releaser-file"
      "${credDir}/usdc-releaser-key"
    ]
    ++ lib.optionals ensOn [
      "--market-ens-rpc-file"
      "${credDir}/ens-rpc"
    ]
    ++ lib.optionals binanceOn [
      "--onramp-binance-api"
      binance.apiUrl
      "--onramp-binance-client-id"
      binance.clientId
      "--onramp-binance-webhook-key-file"
      (toString binance.webhookPublicKeyFile)
      "--onramp-binance-key-file"
      "${credDir}/binance-connect-key"
      "--onramp-binance-token-file"
      "${credDir}/binance-connect-token"
    ];
  # Buying USDC with euros at Binance (losos.edge.market.usdc.binance,
  # edge/registrar/src/onramp.rs). The partner key and token are credentials
  # like the RPC URLs; the client id, API origin and Binance's webhook key
  # are public.
  binance = cfg.market.usdc.binance;
  binanceOn = usdcOn && binance.enable;

  # Machines (losos.edge.vms, modules/edge-vms.nix): the shape of a replica,
  # where its disk is claimed from, and the origin CDI's importer fetches an
  # uploaded image back from, which is this registrar's public name.
  vmServeArgs = lib.optionals cfg.vms.enable (
    [
      "--vm-enable"
      "--vm-cpu"
      (toString cfg.vms.cpu)
      "--vm-memory-mib"
      (toString cfg.vms.memoryMiB)
      "--vm-disk-gib"
      (toString cfg.vms.diskGiB)
      "--vm-storage-class"
      "losos-vm-local"
      "--vm-upload-dir"
      "/var/lib/losos-registrar/vm-images"
      "--vm-upload-max-gib"
      (toString cfg.vms.uploadMaxGiB)
      "--vm-fetch-base"
      "https://${registerDomain}"
    ]
    ++ lib.optionals (cfg.vms.domain != null) [
      "--vm-domain"
      cfg.vms.domain
    ]
  );

  # The DNS half of `serve` (losos.edge.dns.*). Without --dns-zone every
  # /domains/* route answers 503 and no zone file is written.
  dnsCfg = cfg.dns;
  dnsZoneFile = "/var/lib/losos-dns/${dnsCfg.zone}.zone";
  dnsServeArgs = lib.optionals dnsCfg.enable (
    [
      "--dns-zone"
      dnsCfg.zone
      "--dns-zone-file"
      dnsZoneFile
      "--dns-nameservers"
      (lib.concatStringsSep "," dnsCfg.nameservers)
      "--dns-hostmaster"
      dnsCfg.hostmaster
      "--dns-check-url"
      dnsCfg.checkUrl
      "--public-domain"
      cfg.publicDomain
      "--domains-state-file"
      "/var/lib/losos-registrar/domains.json"
    ]
    ++ lib.optionals (dnsCfg.ipv4 != [ ]) [
      "--dns-ipv4"
      (lib.concatStringsSep "," dnsCfg.ipv4)
    ]
    ++ lib.optionals (dnsCfg.ipv6 != [ ]) [
      "--dns-ipv6"
      (lib.concatStringsSep "," dnsCfg.ipv6)
    ]
    ++ lib.optionals echOn echArgs
    ++ lib.optionals relayRoutes [
      "--relay-routes"
      "--relay-routes-file"
      "/var/lib/losos-registrar/relay-routes.json"
      "--relay-pass-key-file"
      "/var/lib/losos-registrar/relay-pass.key"
    ]
  );
  # Custom domains for boxes behind a local edge (edge/registrar's
  # routes.rs): the route table and the key relay passes are signed with,
  # both in the registrar's StateDirectory, the key made on its first start.
  relayRoutes = dnsCfg.enable && dnsCfg.relayRoutes.enable;

  # Encrypted Client Hello (losos.edge.dns.ech, edge/registrar/src/ech.rs):
  # one key, made on the first start by losos-ech-key below, published in
  # the zone's HTTPS records by the registrar and decrypted with by Traefik.
  # The directory is setgid traefik, so the file the registrar (root) writes
  # is Traefik's to read and nobody else's. Only with the zone: without it
  # nothing tells a browser the key.
  echOn = dnsCfg.enable && dnsCfg.ech.enable;
  echKeyFile = "/var/lib/losos-ech/ech.pem";
  echPublicName = if dnsCfg.ech.publicName == null then registerDomain else dnsCfg.ech.publicName;
  echArgs = [
    "--ech-key-file"
    echKeyFile
    "--ech-public-name"
    echPublicName
  ];
  # The `default` TLS option is the one a handshake to the public name
  # meets, so it carries the key; Traefik reads the file when it loads this.
  echYml = (pkgs.formats.yaml { }).generate "losos-ech.yml" {
    tls.options.default.echKeys = [ echKeyFile ];
  };

  # Credentials the registrar decrypts. The list is empty on an edge that
  # does not run USDC, and the unit then has no LoadCredentialEncrypted at
  # all.
  registrarCreds =
    lib.optionals usdcOn (
      lib.mapAttrsToList (name: n: "usdc-rpc-${name}:${toString n.rpcUrlSealed}") usdcNetworks
    )
    ++ lib.optionals releaserOn [
      "usdc-releaser-key:${toString cfg.market.usdc.releaserKeySealed}"
    ]
    ++ lib.optionals ensOn [
      "ens-rpc:${toString cfg.market.usdc.ensRpcUrlSealed}"
    ]
    ++ lib.optionals binanceOn [
      "binance-connect-key:${toString binance.keySealed}"
      "binance-connect-token:${toString binance.tokenSealed}"
    ];

  serveArgs = utils.escapeSystemdExecArgs (
    [
      "${registrar}/bin/losos-registrar"
      "serve"
      "--listen"
      "${fmtBind cfg.registrarApiBind cfg.registrarApiPort}"
      "--registry"
      "/var/lib/losos-registrar/registry.json"
      "--traefik-provider-listen"
      "127.0.0.1:${toString cfg.traefikProviderPort}"
      "--tunnel-config"
      tunnelConfig
      "--tunnel-pin-file"
      "${tunnelStateDir}/pin"
      "--port-range"
      cfg.tunnelPortRange
      "--tenants-file"
      (toString tenantsJson)
      "--reconcile-interval"
      cfg.reconcileInterval
      "--heartbeat-ttl"
      cfg.heartbeatTtl
    ]
    ++ meshServeArgs
    ++ marketServeArgs
    ++ passportArgs
    ++ dnsServeArgs
    ++ enrolArgs
    ++ uplinkArgs
    ++ secondaryArgs
    ++ officialArgs
    ++ accessArgs
    ++ manageArgs
    ++ callsRelayArgs
    ++ inferenceArgs
    ++ ciArgs
    ++ hiddenArgs
    ++ exitArgs
    ++ tangArgs
  );

  # Call media for this edge's boxes (losos.edge.calls.relay,
  # edge/registrar/src/calls_relay.rs). The QUIC port is dual-stack like
  # the tunnel's; media ports bind per address.
  relayCfg = cfg.calls.relay;
  callsRelayArgs = lib.optionals relayCfg.enable (
    [
      "--calls-relay-listen"
      "[::]:${toString relayCfg.port}"
      "--calls-relay-media-ports"
      relayCfg.mediaPorts
    ]
    ++ lib.concatMap (a: [
      "--calls-relay-address"
      a
    ]) relayCfg.addresses
  );
  mediaRange =
    let
      parts = lib.splitString "-" relayCfg.mediaPorts;
    in
    {
      from = lib.toInt (lib.elemAt parts 0);
      to = lib.toInt (lib.elemAt parts 1);
    };
  # Mesh CI's job queue (edge/registrar/src/ci.rs): the jobs boxes hand
  # over, kept in the registrar's StateDirectory across restarts.
  ciArgs = lib.optionals cfg.ci.enable [
    "--ci-dir"
    "/var/lib/losos-registrar/ci"
  ];

  # The boxes' Tang keys (losos.edge.tang, edge/registrar/src/tang.rs): a
  # key pair per box in the registrar's StateDirectory, answered by Tang's
  # own tangd, one run per request.
  tangArgs = lib.optionals cfg.tang.enable [
    "--tang-dir"
    "/var/lib/losos-registrar/tang"
    "--tangd"
    "${pkgs.tang}/libexec/tangd"
    "--tangd-keygen"
    "${pkgs.tang}/libexec/tangd-keygen"
  ];

  # Models split across boxes (losos.edge.inference,
  # modules/edge-inference.nix): the catalogue, the loopback ports the
  # lending boxes' RPC tunnels are bound on, and where the registrar writes
  # each head's environment and finds its socket.
  inferenceArgs = lib.optionals cfg.inference.enable [
    "--inference-models"
    "/etc/losos/inference-models.json"
    "--llm-port-range"
    "${toString cfg.inference.portRange.from}-${toString cfg.inference.portRange.to}"
    "--llm-env-dir"
    "/run/losos-llm-env"
    "--llm-socket-dir"
    "/run/losos-llm-heads"
  ];

  # The page boxes are managed from (losos.edge.manage, server/manage.rs in
  # the registrar). The key is made by the registrar on its first start.
  manageArgs = lib.optionals cfg.manage.enable (
    [
      "--manage-key-file"
      "/var/lib/losos-registrar/manage-key"
    ]
    ++ lib.optionals (cfg.manage.ui != null) [
      "--manage-ui"
      "${cfg.manage.ui}"
    ]
    ++ [
      "--manage-traefik-metrics"
      "http://127.0.0.1:${toString cfg.traefikMetricsPort}/metrics"
    ]
  );

  # The terminal (docs/edge-terminal.md): on once a key is seeded. The key
  # and the tokens live in the registrar's StateDirectory; the seal script
  # runs through systemd-run, outside this unit's sandbox and cgroup.
  accessArgs = lib.optionals (cfg.access.gpgKeys != [ ]) [
    "--access-dir"
    "/var/lib/losos-registrar/access"
    "--access-keys-file"
    "/etc/losos/edge-access-keys.asc"
    "--seal-credential"
    "${cfg.credentials.sealPackage}/bin/losos-seal-credential"
    "--systemd-run"
    "${config.systemd.package}/bin/systemd-run"
  ];

  # The open gateway's passport (losos.edge.passport.enable,
  # modules/edge-passport.nix): the edge key the registrar makes on its
  # first start, the owner's settings it accepts and the copy measured at
  # boot, and the TPM objects losos-gateway-tpm made.
  passportArgs = lib.optionals cfg.passport.enable [
    "--gateway-key-file"
    "/var/lib/losos-registrar/edge.key"
    "--gateway-settings-file"
    "/var/lib/losos-gateway/settings.json"
    "--gateway-measured-file"
    "/run/losos-gateway/settings.json"
    "--gateway-tpm-dir"
    "/run/losos-gateway/tpm"
    "--gateway-tcti"
    "device:/dev/tpm0"
  ];
in
{
  # Where losos.mdns below lands (lan.advertise).
  imports = [
    ./mdns.nix
    ./edge-passport.nix
  ];

  config = lib.mkIf enabled {
    # The edge module is only imported by the edge system, so wiring the
    # registrar package here (gated on `enabled`, which is always true for the
    # edge system) keeps edge.nix self-contained — mirrors defaults.nix wiring
    # losos.backend.package on the appliance.
    losos.edge.registrar.package = self.packages.x86_64-linux.losos-registrar;
    losos.edge.manage.ui = lib.mkIf cfg.manage.enable (
      lib.mkDefault self.packages.x86_64-linux.losos-admin-ui
    );

    assertions = [
      {
        # A lending box's second tunnel service is `<id>:llm`
        # (edge/registrar/src/config.rs); a colon in an id could take it.
        assertion = lib.all (id: !lib.hasInfix ":" id) (lib.attrNames cfg.tenants);
        message = "losos.edge.tenants: an appliance id may not contain a colon.";
      }
      {
        # The offline page's middleware and service (edge/registrar's
        # config.rs, OFFLINE_MIDDLEWARE) sit beside the tenants' own.
        assertion = !(cfg.tenants ? losos_offline);
        message = "losos.edge.tenants: `losos_offline` is the name of the edge's offline page.";
      }
      {
        assertion = dnsCfg.enable -> (dnsCfg.ipv4 != [ ] || dnsCfg.ipv6 != [ ]);
        message = ''
          losos.edge.dns.enable requires losos.edge.dns.ipv4 and/or
          losos.edge.dns.ipv6: the edge's public addresses, which every name
          in the zone resolves to.
        '';
      }
      {
        # An edge that serves its own zone is an official one (the names
        # answer only on a claimed gateway), and an official edge's names
        # are signed and hide which box a browser asks for.
        assertion = dnsCfg.enable -> (dnsCfg.dnssec.enable && dnsCfg.ech.enable);
        message = ''
          losos.edge.dns.enable requires losos.edge.dns.dnssec.enable and
          losos.edge.dns.ech.enable: an official edge's zone is signed and
          carries its Encrypted Client Hello key.
        '';
      }
      {
        assertion = dnsCfg.relayRoutes.enable -> dnsCfg.enable;
        message = ''
          losos.edge.dns.relayRoutes.enable requires losos.edge.dns.enable:
          only an edge that routes custom domains keeps the route table.
        '';
      }
      {
        assertion = dnsCfg.enable -> cfg.passport.enable;
        message = ''
          losos.edge.dns.enable requires losos.edge.passport.enable: only an
          open gateway that boxes can check hands out names, and the domain
          routes answer 503 until its owner has claimed it.
        '';
      }
      {
        assertion = gateOn -> cfg.market.returnUrl != "";
        message = ''
          losos.edge.market.cards.enable requires losos.edge.market.returnUrl:
          the page Stripe sends buyers and sellers back to after Checkout and
          onboarding.
        '';
      }
      {
        assertion = (cfg.market.enable && !cfg.market.cards.enable) -> cfg.market.usdc.enable;
        message = ''
          losos.edge.market.cards.enable = false needs losos.edge.market.usdc.enable:
          without cards the market takes coins, and USDC is the checkout they ride on.
        '';
      }
      {
        assertion = hardwareOn -> cfg.market.cards.enable;
        message = "losos.edge.market.hardware.enable needs losos.edge.market.cards.enable: hardware is paid by card.";
      }
      {
        assertion = cfg.market.usdc.enable -> cfg.market.enable;
        message = "losos.edge.market.usdc.enable requires losos.edge.market.enable.";
      }
      {
        assertion = cfg.market.usdc.enable -> cfg.market.currency == "usd";
        message = ''
          losos.edge.market.usdc.enable requires losos.edge.market.currency = "usd":
          a price in cents is that many USDC cents.
        '';
      }
      {
        assertion = cfg.market.usdc.enable -> usdcNetworks != { };
        message = "losos.edge.market.usdc.enable needs at least one network in losos.edge.market.usdc.networks.";
      }
      {
        assertion = lib.all (name: lib.elem name usdcKnown) (lib.attrNames usdcNetworks);
        message = ''
          losos.edge.market.usdc.networks takes ${lib.concatStringsSep ", " usdcKnown}; got
          ${lib.concatStringsSep ", " (lib.attrNames usdcNetworks)}.
        '';
      }
      {
        assertion =
          let
            modes = lib.unique (
              map (name: lib.hasInfix "sepolia" name || lib.hasSuffix "devnet" name) (lib.attrNames usdcNetworks)
            );
          in
          lib.length modes <= 1;
        message = ''
          losos.edge.market.usdc.networks mixes test and live networks; one edge
          settles in the mode of its Stripe key.
        '';
      }
      {
        assertion = lib.all (n: n) (
          lib.mapAttrsToList (
            name: n:
            if lib.hasPrefix "solana" name then
              builtins.match "[1-9A-HJ-NP-Za-km-z]{32,44}" n.treasury != null
              && (
                n.pay == null || (builtins.match "[1-9A-HJ-NP-Za-km-z]{32,44}" n.pay != null && n.pay != n.treasury)
              )
            else
              n.pay != null
              && builtins.match "0x[0-9a-fA-F]{40}" n.treasury != null
              && lib.toLower n.pay != lib.toLower n.treasury
              && n.genesisHash == null
          ) usdcNetworks
        );
        message = ''
          losos.edge.market.usdc.networks: an EVM network needs `pay` and an
          `0x` treasury that differ; a Solana network needs a base58 treasury
          and, for its escrow, a base58 `pay` that differs from it. Only
          `solana-devnet` takes `genesisHash`.
        '';
      }
      {
        assertion = binance.enable -> usdcOn && usdcNetworks ? base;
        message = ''
          losos.edge.market.usdc.binance.enable needs losos.edge.market.usdc
          with the live `base` network: Binance sends the USDC on Base.
        '';
      }
      {
        assertion =
          binance.enable
          -> binance.apiUrl != null && binance.clientId != null && binance.webhookPublicKeyFile != null;
        message = ''
          losos.edge.market.usdc.binance.enable needs apiUrl, clientId and
          webhookPublicKeyFile, the details Binance gives with the partner
          account.
        '';
      }
      {
        assertion = cfg.market.zcash.enable -> cfg.market.usdc.enable;
        message = ''
          losos.edge.market.zcash.enable requires losos.edge.market.usdc.enable:
          Zcash is offered beside USDC, priced in the same dollars.
        '';
      }
      {
        assertion = cfg.market.zcash.enable -> builtins.length cfg.market.zcash.servers >= 2;
        message = ''
          losos.edge.market.zcash.servers needs at least two servers, of
          different operators: a payment counts only when two agree.
        '';
      }
      {
        assertion = cfg.market.monero.enable -> cfg.market.usdc.enable;
        message = ''
          losos.edge.market.monero.enable requires losos.edge.market.usdc.enable:
          Monero is offered beside USDC, priced in the same dollars.
        '';
      }
      {
        assertion = cfg.market.bitcoin.enable -> cfg.market.usdc.enable;
        message = ''
          losos.edge.market.bitcoin.enable requires losos.edge.market.usdc.enable:
          Bitcoin is offered beside USDC, priced in the same dollars.
        '';
      }
      {
        assertion = cfg.market.enable -> meshEnabled;
        message = ''
          losos.edge.market.enable requires losos.edge.cluster.enable so paid
          storage orders can be fulfilled by the mesh.
        '';
      }
      {
        assertion =
          (cfg.market.enable && cfg.market.storageClass == "longhorn") -> cfg.cluster.longhornChart != null;
        message = ''
          losos.edge.market.storageClass is "longhorn" but
          losos.edge.cluster.longhornChart is null: no Longhorn means no such
          storage class, so every paid storage order would wait for a volume
          that never comes and be refunded. Keep the default chart, or name a
          storage class this cluster has.
        '';
      }
      {
        assertion = cfg.acmeEmail != null;
        message = "losos.edge.enable requires losos.edge.acmeEmail (Let's Encrypt account email).";
      }
      {
        assertion = cfg.lan.openEnrolment -> cfg.lan.advertise;
        message = ''
          losos.edge.lan.openEnrolment requires losos.edge.lan.advertise: open
          enrolment accepts any box that can reach the registrar with the
          token it brought, which is a LAN posture. On an edge the internet
          reaches it would let anyone publish a hostname through it.
        '';
      }
      {
        assertion = cfg.secondary.enable -> cfg.lan.advertise;
        message = ''
          losos.edge.secondary.enable requires losos.edge.lan.advertise: boxes
          find the gateways of a secondary cluster by their adverts.
        '';
      }
      {
        assertion = lib.length cfg.official.peers <= 64;
        message = "losos.edge.official.peers lists more than 64 edges.";
      }
      {
        assertion =
          cfg.uplink.enable
          -> (cfg.uplink.configFile != null || (cfg.uplink.registrarUrl != "" && cfg.uplink.id != ""));
        message = ''
          losos.edge.uplink.enable needs the hub: either
          losos.edge.uplink.configFile (a runtime JSON the owner fills in) or
          both losos.edge.uplink.registrarUrl and losos.edge.uplink.id.
        '';
      }
      {
        assertion =
          cfg.uplink.enable
          -> (
            cfg.uplink.id == "" || builtins.match "[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?" cfg.uplink.id != null
          );
        message = ''
          losos.edge.uplink.id must be a DNS label (lowercase letters, digits
          and hyphens): it is this edge's tenant id on the hub and the prefix
          of every relayed service name.
        '';
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
      # No released Traefik decrypts ECH; nix/traefik-ech.nix is the one
      # that does, and only an edge with the key runs it.
      package = lib.mkIf echOn (pkgs.callPackage ../nix/traefik-ech.nix { });
    };

    # /etc/traefik/dynamic: real dir (0755, root) holding the static
    # register*.yml files (symlinked from the store). The `r` lines take
    # away what the registrar wrote there before Traefik discovered the boxes
    # from it (see the top of this file), the boxes' Tor and I2P names
    # included.
    systemd.tmpfiles.rules = [
      "d /etc/traefik/dynamic 0755 root root - -"
      # The tunnel's file, written by the registrar, read by losos-tunnel.
      "d /etc/losos-tunnel 0755 root root - -"
      "L+ /etc/traefik/dynamic/register.yml - - - - ${toString registerYml}"
      "r /etc/traefik/dynamic/losos.yml - - - - -"
      "r /etc/traefik/dynamic/losos-vms.yml - - - - -"
      "r /etc/traefik/dynamic/hidden.yml - - - - -"
    ]
    ++ (
      if echOn then
        [
          "d /var/lib/losos-ech 2750 root traefik - -"
          "L+ /etc/traefik/dynamic/ech.yml - - - - ${echYml}"
        ]
      else
        [ "r /etc/traefik/dynamic/ech.yml - - - - -" ]
    )
    ++ lib.optionals cfg.vms.enable [
      "L+ /etc/traefik/dynamic/register-vm-transfer.yml - - - - ${toString vmTransferYml}"
    ]
    # The zone directory: root writes (the registrar), knot reads.
    ++ lib.optionals dnsCfg.enable [
      "d /var/lib/losos-dns 0755 root root - -"
    ]
    # Longhorn runs iscsiadm and mount.nfs in the host's mount namespace with
    # an FHS PATH, which on NixOS finds neither: they live in
    # /run/current-system/sw/bin. /usr/local/sbin is on that PATH and empty
    # on NixOS, so it points there. The appliance carries the same rule in
    # modules/cluster.nix.
    ++ lib.optionals (meshEnabled && cfg.cluster.longhornChart != null) [
      "L+ /usr/local/sbin - - - - /run/current-system/sw/bin"
    ];

    environment.etc."losos/edge-access-keys.asc" = lib.mkIf (cfg.access.gpgKeys != [ ]) {
      text = lib.concatStringsSep "\n" cfg.access.gpgKeys;
    };

    # ── losos-registrar (registration API + reconciler) ─────────────────
    systemd.services.losos-registrar = {
      description = "losos edge registrar — registration API, Traefik's service discovery, tunnel config";
      wantedBy = [ "multi-user.target" ];
      after = [
        "network-online.target"
      ]
      ++ lib.optional meshEnabled "losos-mesh-rbac.service"
      ++ lib.optional gateOn "losos-stripe-gate.service"
      ++ lib.optional btcOn "bitcoind-losos.service"
      ++ lib.optional xmrOn "losos-monero-wallet.service";
      # Wants, not Requires, on the RBAC extractor: the master-proxy half must
      # keep serving /register and rewriting Traefik on an edge whose mesh
      # apiserver is down or not yet up. A failed extraction costs the join
      # route (503) and nothing else.
      wants = [
        "network-online.target"
      ]
      ++ lib.optional meshEnabled "losos-mesh-rbac.service"
      ++ lib.optional gateOn "losos-stripe-gate.service"
      ++ lib.optional btcOn "bitcoind-losos.service"
      ++ lib.optional xmrOn "losos-monero-wallet.service";
      # A changed seed is read at start, like the box's.
      restartTriggers = lib.optional (
        cfg.access.gpgKeys != [ ]
      ) config.environment.etc."losos/edge-access-keys.asc".source;
      serviceConfig = {
        ExecStart = serveArgs;
        StateDirectory = "losos-registrar";
        StateDirectoryMode = "0700";
        Restart = "always";
        RestartSec = 5;
        # Writes /etc/losos-tunnel; reads 0600 token files and the tunnel's
        # pin.
        # Runs as root; deliberately no ProtectSystem (would make /etc RO).
        PrivateTmp = true;
        NoNewPrivileges = true;
      }
      // lib.optionalAttrs gateOn {
        # Second layer behind the gate: the registrar cannot read the sealed
        # blobs or the gate's credential directory, even as root.
        InaccessiblePaths = map (path: "-${path}") gateSealed ++ [
          "-/run/credentials/losos-stripe-gate.service"
        ];
      }
      // lib.optionalAttrs (registrarCreds != [ ]) {
        # Plaintext only in this unit's credential directory, read per request.
        # `usdc-rpc-<network>` is a chain endpoint, `usdc-releaser-key` the
        # escrow's key, `ens-rpc` the name lookups and `binance-connect-*`
        # the Binance partner's key and token. None is a flag, so none shows
        # up in the process list.
        LoadCredentialEncrypted = registrarCreds;
        # systemd fails the unit when an absolute blob is missing, and an
        # edge whose registrar never starts cannot have anything sealed
        # through `losos edge secret set`. A SetCredential= of the same name
        # is the fallback systemd takes then; the registrar reads the word as
        # "not configured" (503) for every one of these.
        SetCredential = map (c: "${lib.head (lib.splitString ":" c)}:unsealed") registrarCreds;
      };
    };

    # ── bitcoind (Bitcoin orders, losos.edge.market.bitcoin) ─────────────
    # A pruned node on loopback only. The wallet it holds is watch-only and
    # made by the registrar on first use; nothing on this machine can spend.
    services.bitcoind.losos = lib.mkIf btcOn {
      enable = true;
      dataDir = btcDataDir;
      prune = cfg.market.bitcoin.pruneMiB;
      # On the command line, not in bitcoin.conf: outside `main` the file's
      # RPC settings only count inside a [chain] section.
      extraCmdlineOptions = [
        "-chain=${btcChain}"
        "-rpcport=${toString btcRpcPort}"
        "-rpcbind=127.0.0.1"
        "-rpcallowip=127.0.0.1"
        # The journal has every line already (bitcoind prints to stdout
        # under systemd); debug.log in the data directory would only grow.
        "-nodebuglogfile"
      ];
    };

    # ── monerod + wallet service (Monero orders, losos.edge.market.monero) ─
    # A pruned node with restricted RPC on loopback. The registrar asks it
    # which network it follows; the wallet service syncs through it.
    services.monero = lib.mkIf xmrOn {
      enable = true;
      prune = true;
      rpc = {
        address = "127.0.0.1";
        port = xmrDaemonPort;
        restricted = true;
      };
      extraConfig = lib.optionalString (xmrNet != "mainnet") "${xmrNet}=1";
    };

    # The sellers' view-only wallets, one open at a time. A view key sees
    # what arrives and cannot spend, so nothing here can move a coin. The
    # login is in a config file, not on the command line, where `ps` would
    # show it.
    systemd.services.losos-monero-wallet = lib.mkIf xmrOn {
      description = "Monero wallet service for the market's view-only wallets";
      wantedBy = [ "multi-user.target" ];
      after = [ "monero.service" ];
      wants = [ "monero.service" ];
      path = [ pkgs.coreutils ];
      preStart = ''
        umask 077
        mkdir -p ${xmrWalletDir}/wallets
        if [ ! -s ${xmrLogin} ]; then
          printf 'losos:%s' "$(tr -dc A-Za-z0-9 </dev/urandom | head -c 40)" > ${xmrLogin}
        fi
        printf 'rpc-login=%s\n' "$(cat ${xmrLogin})" > ${xmrWalletDir}/wallet-rpc.conf
      '';
      serviceConfig = {
        ExecStart = lib.concatStringsSep " " (
          [
            "${pkgs.monero-cli}/bin/monero-wallet-rpc"
            "--config-file=${xmrWalletDir}/wallet-rpc.conf"
            "--daemon-address=127.0.0.1:${toString xmrDaemonPort}"
            "--trusted-daemon"
            "--wallet-dir=${xmrWalletDir}/wallets"
            "--rpc-bind-ip=127.0.0.1"
            "--rpc-bind-port=${toString xmrWalletPort}"
            "--log-file=${xmrWalletDir}/wallet-rpc.log"
            "--max-log-files=2"
            "--max-log-file-size=10485760"
            "--non-interactive"
          ]
          ++ lib.optional (xmrNet != "mainnet") "--${xmrNet}"
        );
        StateDirectory = "losos-monero-wallet";
        StateDirectoryMode = "0700";
        Restart = "always";
        RestartSec = 5;
        DynamicUser = true;
        NoNewPrivileges = true;
        PrivateTmp = true;
        ProtectSystem = "strict";
        ProtectHome = true;
      };
    };

    # The ECH key exists before Traefik reads ech.yml, which inlines the file
    # once at load, and before the registrar renders the zone. The registrar
    # reads the same file with the same public name, so it never makes a
    # second key; a changed publicName makes a new one here, and Traefik
    # restarts with it.
    systemd.services.losos-ech-key = lib.mkIf echOn {
      description = "Make the edge's Encrypted Client Hello key";
      wantedBy = [ "multi-user.target" ];
      before = [
        "traefik.service"
        "losos-registrar.service"
      ];
      requiredBy = [ "traefik.service" ];
      restartTriggers = [ echPublicName ];
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        UMask = "0027";
        ExecStart = utils.escapeSystemdExecArgs [
          "${registrar}/bin/losos-registrar"
          "ech-key"
          "--key-file"
          echKeyFile
          "--public-name"
          echPublicName
        ];
      };
    };
    systemd.services.traefik.restartTriggers = lib.mkIf echOn [ echPublicName ];

    # ── DNS: the edge's own zone, served by Knot ─────────────────────────
    # losos-registrar renders the zone (edge/registrar/src/zone.rs) into
    # /var/lib/losos-dns, world-readable, with a fresh serial whenever it
    # says something new; Knot loads it whole and keeps no journal, so the
    # file is the whole truth. The path unit below reloads the zone when the
    # registrar replaces the file. Before the first reconcile the file does
    # not exist and Knot answers REFUSED for the zone; that lasts until the
    # registrar's first pass, which runs before its API opens.
    # With dnssec.enable Knot signs what it loads (its default policy: ECDSA
    # P-256, the KSK kept until an operator rolls it, ZSKs rolled by Knot),
    # so the file stays unsigned and the keys stay in Knot's own state
    # directory. ENS reads the wallet records only from a signed zone.
    services.knot = lib.mkIf dnsCfg.enable {
      enable = true;
      settings = {
        server.listen = dnsCfg.listen;
        zone.${dnsCfg.zone} = {
          file = dnsZoneFile;
          zonefile-load = "whole";
          zonefile-sync = "-1";
          journal-content = "none";
          dnssec-signing = dnsCfg.dnssec.enable;
        };
      };
    };

    systemd.paths.losos-dns-reload = lib.mkIf dnsCfg.enable {
      description = "Reload the edge's zone when losos-registrar rewrites it";
      wantedBy = [ "multi-user.target" ];
      pathConfig.PathChanged = dnsZoneFile;
    };

    systemd.services.losos-dns-reload = lib.mkIf dnsCfg.enable {
      description = "Reload the edge's zone in Knot";
      after = [ "knot.service" ];
      # Nothing to reload into when Knot is not running; it reads the file
      # when it starts.
      unitConfig.ConditionPathExists = "/run/knot/knot.sock";
      serviceConfig = {
        Type = "oneshot";
        ExecStart = "${config.services.knot.package}/bin/knotc --socket=/run/knot/knot.sock zone-reload ${dnsCfg.zone}";
      };
    };

    # ── losos-stripe-gate (the only holder of the Stripe key) ────────────
    systemd.services.losos-stripe-gate = lib.mkIf gateOn {
      description = "losos Stripe gate — holds the Stripe key for the registrar";
      wantedBy = [ "multi-user.target" ];
      before = [ "losos-registrar.service" ];
      # The registrar wants, not requires, the gate: see `gateSocket`.
      unitConfig.ConditionPathExists = gateSealed;
      serviceConfig = {
        ExecStart = lib.concatStringsSep " " (
          [
            "${registrar}/bin/losos-registrar"
            "stripe-gate"
            "--socket"
            gateSocket
            "--stripe-key-file"
            "%d/stripe-secret-key"
            "--webhook-secret-file"
            "%d/stripe-webhook-secret"
            "--currency"
            cfg.market.currency
            "--stripe-api"
            (utils.escapeSystemdExecArg cfg.market.stripeApi)
            # The same two values the registrar is given. The gate holds every
            # Checkout and onboarding link to them exactly, so a compromised
            # registrar can neither raise the platform's cut past what the
            # operator set nor send buyers and sellers to a page of its choosing.
            "--fee-bps"
            (toString cfg.market.feeBps)
            "--return-url"
            (utils.escapeSystemdExecArg cfg.market.returnUrl)
          ]
          ++ hardwareArgs "--hardware-catalogue"
        );
        LoadCredentialEncrypted = [
          "stripe-secret-key:${toString cfg.market.stripeSecretKeySealed}"
          "stripe-webhook-secret:${toString cfg.market.webhookSecretSealed}"
        ];
        DynamicUser = true;
        RuntimeDirectory = "losos-stripe-gate";
        RuntimeDirectoryMode = "0755";
        Restart = "always";
        RestartSec = 5;
        NoNewPrivileges = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        PrivateTmp = true;
        PrivateDevices = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectControlGroups = true;
        RestrictAddressFamilies = [
          "AF_UNIX"
          "AF_INET"
          "AF_INET6"
        ];
        RestrictNamespaces = true;
        LockPersonality = true;
        MemoryDenyWriteExecute = true;
        SystemCallArchitectures = "native";
        CapabilityBoundingSet = "";
      };
    };

    # ── The tunnel server (edge/registrar/src/tunnel.rs) ─────────────────
    # Independent of the registrar: it re-reads the file the registrar writes
    # every second, so a registrar restart costs no tunnel and a box that
    # registers is reachable within a second of the registrar's pass. A
    # missing file means no services yet. The certificate is made on the
    # first start and kept in its state directory; every box pins it.
    systemd.services.losos-tunnel = {
      description = "losos tunnel server: boxes dial in over QUIC, or TLS on TCP";
      wantedBy = [ "multi-user.target" ];
      after = [ "network-online.target" ];
      wants = [ "network-online.target" ];
      serviceConfig = {
        ExecStart = tunnelArgs;
        StateDirectory = "losos-tunnel";
        StateDirectoryMode = "0755";
        Restart = "always";
        RestartSec = 5;
        PrivateTmp = true;
        NoNewPrivileges = true;
        ProtectHome = true;
        ProtectSystem = "strict";
        ReadOnlyPaths = [ "/etc/losos-tunnel" ];
        # Binds only on loopback and its own port, both above 1024 by
        # default; Traefik is the one reaching the per-box ports.
        RestrictAddressFamilies = [
          "AF_INET"
          "AF_INET6"
          "AF_UNIX"
        ];
      };
    };

    # ── The uplink: this edge as a spoke (losos.edge.uplink) ─────────────
    # The tunnel client, the one a box runs, on the file the registrar's
    # uplink loop keeps at /etc/losos-tunnel/uplink.json: one service per box
    # the hub accepted, each pointed at that box's local port here, so a
    # request that enters the hub crosses two tunnels to reach the box. The
    # client re-reads the file and reconnects when it changes. The unit is
    # conditioned on the file and started by a path unit when it appears,
    # because the file exists only once the owner has given the registrar a
    # hub (a gateway image ships with none) and the loop has rendered it.
    systemd.services.losos-tunnel-uplink = lib.mkIf cfg.uplink.enable {
      description = "losos tunnel uplink: this edge's outbound tunnel to its hub";
      wantedBy = [ "multi-user.target" ];
      after = [
        "network-online.target"
        "losos-registrar.service"
      ];
      wants = [ "network-online.target" ];
      unitConfig.ConditionPathExists = uplinkTunnelConfig;
      serviceConfig = {
        ExecStart = utils.escapeSystemdExecArgs [
          "${registrar}/bin/losos-registrar"
          "tunnel-client"
          "--config"
          uplinkTunnelConfig
        ];
        Restart = "always";
        RestartSec = 5;
        PrivateTmp = true;
        NoNewPrivileges = true;
      };
    };
    systemd.paths.losos-tunnel-uplink = lib.mkIf cfg.uplink.enable {
      description = "start the uplink tunnel when the registrar has rendered its config";
      wantedBy = [ "multi-user.target" ];
      pathConfig.PathExists = uplinkTunnelConfig;
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

      images = lib.optional backupsOn restServerImage;
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

    # ── On the LAN: let appliances find this edge ────────────────────────
    # DNS-SD over systemd-resolved's mDNS (modules/mdns.nix), which also
    # gives the edge its `.local` name — the one the default
    # losos.edge.lan.url points at. The service carries the registrar API
    # port and a `url=` record; the appliance side (backend/src/edge.rs)
    # prefers the record and falls back to the resolved address and port,
    # then probes /health before believing either. `tunnel=` is where a box
    # that picks this edge dials its tunnel, and `enrol=` tells it (and a
    # person running `resolvectl service`) whether an unprovisioned box can
    # expect to be let in; neither is trusted for anything the box would not
    # have done anyway.
    # Off-loopback binding is forced here only when the operator left the
    # bind at its loopback default, because an advertised edge nobody can
    # dial is worse than none: the box would show "found" and then refuse.
    losos.mdns = lib.mkIf cfg.lan.advertise {
      enable = true;
      services.losos-edge = ''
        Name=losos edge on %H
        Type=_losos-edge._tcp
        Port=${toString cfg.registrarApiPort}
        TxtText=url=${cfg.lan.url} tunnel=${cfg.lan.tunnelEndpoint} enrol=${
          if cfg.lan.openEnrolment then "open" else "closed"
        } txtvers=1
      '';
    };
    losos.edge.registrarApiBind = lib.mkIf cfg.lan.advertise (lib.mkDefault "::");

    # ── Firewall: public web (TCP and QUIC) + the tunnel's port, UDP and TCP ─
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
      # The tunnel's TCP fallback, for boxes whose network drops UDP.
      cfg.tunnelBindPort
    ]
    ++ lib.optional registrarOffBox cfg.registrarApiPort
    ++ lib.optionals meshEnabled [
      cfg.cluster.apiPort
      cfg.cluster.supervisorPort
    ]
    # DNS answers over TCP too: large responses and zone checks need it.
    ++ lib.optional dnsCfg.enable 53;
    # 443 over UDP is HTTP/3 (QUIC), Traefik's websecure; the tunnel's port
    # is QUIC too.
    networking.firewall.allowedUDPPorts = [
      443
      cfg.tunnelBindPort
    ]
    ++ lib.optional dnsCfg.enable 53
    ++ lib.optional relayCfg.enable relayCfg.port;
    networking.firewall.allowedUDPPortRanges = lib.optional relayCfg.enable mediaRange;
  };
}
