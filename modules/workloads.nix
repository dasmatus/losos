# The local cluster's own workloads: Nextcloud and Forgejo, as Kubernetes
# *static pods*.
#
# `losos.<svc>.mode == "container"` used to mean a systemd-nspawn container on a
# private veth; it now means a pod in this box's own k3s server.
# modules/cluster.nix runs the cluster (and preloads the images listed in
# losos.workloads.*), modules/containers.nix keeps the Nginx front door that
# proxies to them, modules/nextcloud-common.nix still holds the Nextcloud truth
# both modes share, and this file owns the Kubernetes objects plus the config
# they mount.
#
# ── Why static pods and not Deployments ──────────────────────────────────────
# The local cluster runs --flannel-backend=none, so the kubelet reports
# NetworkReady=false, the node-lifecycle controller parks
# node.kubernetes.io/not-ready:NoSchedule on the node, and this node is
# legitimately NotReady for the life of the cluster. A Deployment's pods would
# sit Pending behind that taint forever. A static pod is created by the kubelet
# straight from a directory on disk: no scheduler, no controller-manager, no
# ReplicaSet, exactly one instance on this node by construction — which is also
# why the box serves its own data while the apiserver is still coming up.
# Anyone converting these to Deployments must add tolerations for
# node.kubernetes.io/not-ready, both NoSchedule *and* NoExecute, or the
# appliance comes up with no Nextcloud and no shell to find out why.
#
# The manifests are symlinked into a losos-owned directory selected with
# --kubelet-arg=pod-manifest-path (see modules/cluster.nix), not into
# services.k3s.manifests: that option is the addon/auto-deploy path, which goes
# through the apiserver and the scheduler and therefore hits exactly the
# problem above. The directory is deliberately not k3s's own default under
# /var/lib/rancher/k3s either, so wiping the k3s data dir cannot take the
# appliance's workloads with it.
#
# ── Why hostNetwork, and what it costs ───────────────────────────────────────
# No CNI means no pod network, so every container shares the host's netns:
#   * every listener binds 127.0.0.1 explicitly (listen.conf below, and
#     Forgejo's HTTP_ADDR). Nginx stays the only thing holding a public port.
#   * dnsPolicy is Default — the host's /etc/resolv.conf.
#     ClusterFirstWithHostNet would be wrong twice over: coredns is disabled and
#     there is no kube-proxy to reach a ClusterIP with.
#   * containerPort entries would be decoration under hostNetwork, so there are
#     none; the real binding comes from the mounted config.
#   * nginx sees these pods as 127.0.0.1 or as the box's own LAN address. See
#     the `lanOnly` comment in modules/containers.nix for what that costs the
#     admin guard.
#
# ── Why the config is rendered here and not baked into the image ─────────────
# losos.nextcloud.apachePort is tunable from the settings SPA, so it must not be
# an image layer: a ~2.6 GiB Nextcloud image cannot be rebuilt on a mini-PC at
# 03:00 with a tmpfs root. The port-bearing files are rendered in module context
# and hostPath-mounted, so changing the port changes a store path, which changes
# the manifest, which changes the tmpfiles symlink, and the kubelet restarts the
# pod on the next `nixos-rebuild switch`. No image is ever rebuilt.
#
# Every hostPath volume declares an explicit `type:`. Without it the kubelet
# CREATES A DIRECTORY at a missing path — which silently turns the admin
# password file into a directory and Nextcloud's first-run install into a loop.
{
  pkgs,
  lib,
  config,
  ...
}:

let
  nextcloudWorkload = config.losos.nextcloud.mode == "container";
  # losos.forgejo.enable is consulted in *both* modes; see modules/containers.nix.
  forgejoWorkload = config.losos.forgejo.mode == "container" && config.losos.forgejo.enable;
  anyWorkload = nextcloudWorkload || forgejoWorkload;

  hostName = config.losos.hostName;
  proxied = config.losos.proxy.enable;
  proxyHostName = config.losos.proxy.hostname;
  apachePort = config.losos.nextcloud.apachePort;
  adminpassFile = toString config.losos.nextcloud.adminpassFile;
  gpu = config.losos.gpu.enable;

  nextcloudImage = config.losos.workloads.nextcloudImage;
  forgejoImage = config.losos.workloads.forgejoImage;

  # Durable state. Both are covered by impermanence's whole-/var bind mount, so
  # nothing has to be added to modules/impermanence.nix for them.
  nextcloudState = "/var/lib/nextcloud";
  forgejoState = "/var/lib/forgejo";

  # Host sockets the pods reach their databases through. Postgres and Redis run
  # natively rather than in the pods: under hostNetwork an in-pod Postgres binds
  # the *host's* 0.0.0.0:5432 unless hand-configured, and "nothing but Nginx
  # holds a public port" only survives the native form. See modules/services.nix.
  pgSocketDir = "/run/postgresql";
  redisSocketDir = "/run/redis-nextcloud";

  # Fixed uids, pinned in modules/configuration.nix next to notshared/shared.
  # They are not cosmetic: Postgres peer auth over /run/postgresql compares the
  # *uid* of the connecting process, and the pod shares the host's user
  # namespace, so the pod's runAsUser is what Postgres maps to a role. gid ==
  # uid by construction there, hence one binding each.
  nextcloudUid = 1002;
  forgejoUid = 1003;

  # Redis' socket is mode 0660, group redis-nextcloud, and a container does not
  # inherit the *host* user's supplementary groups: only what the pod spec asks
  # for. So the pod can reach the socket only if that group has a fixed gid we
  # can name here. If it is left dynamic this stays null, the pod runs with
  # group 1002 alone, and Nextcloud fails to reach Redis with EACCES on
  # /run/redis-nextcloud/redis.sock — pin users.groups."redis-nextcloud".gid in
  # modules/configuration.nix rather than widening unixSocketPerm, which would
  # open the cache to every uid on the box.
  redisGid = config.users.groups."redis-nextcloud".gid or null;

  staticPodDir = "/var/lib/losos/k3s-static-pods";

  yaml = pkgs.formats.yaml { };
  ini = pkgs.formats.ini { };

  # dockerTools images carry their own name/tag; the pods reference them by that
  # string with imagePullPolicy: Never, and losos-workload-images.service
  # (modules/cluster.nix) imports the same derivations into the native
  # containerd's k8s.io namespace before k3s starts. Nothing is ever fetched
  # from a container registry; the images are substituted from our own binary
  # cache.
  #
  # That file spells this helper identically, and has to: the ref the pod asks
  # for and the ref the import produces are compared as strings by the kubelet,
  # and a mismatch under imagePullPolicy: Never is ImagePullBackOff forever with
  # nothing in any log naming the two halves. Change one, change the other.
  imageRef = image: "${image.imageName}:${image.imageTag}";

  # A directory of *real files*, not symlinks. pkgs.linkFarm would be the
  # obvious tool and is the wrong one: it produces symlinks into /nix/store, and
  # the kubelet bind-mounts only this one directory into the pod. The store is
  # not in the pod's mount namespace, so every entry would dangle and the
  # entrypoint would read nothing. For the same reason nothing rendered in here
  # may reference a second store path — the files must be self-contained
  # scalars.
  confDir =
    name: files:
    pkgs.runCommand name { } (
      ''
        mkdir -p "$out"
      ''
      + lib.concatStrings (
        lib.mapAttrsToList (fileName: src: ''
          cp ${src} "$out/${fileName}"
        '') files
      )
    );

  # ── Nextcloud's rendered config ──────────────────────────────────────────
  # PHP source, so option-derived values are escaped as PHP rather than
  # interpolated: losos.hostName and losos.proxy.hostname are operator-supplied
  # strings, and a stray quote in one would turn the config into a parse error
  # on a box with no shell to fix it from.
  phpStr = s: "'" + lib.escape [ "\\" "'" ] s + "'";
  phpList = xs: "[" + lib.concatMapStringsSep ", " phpStr xs + "]";

  nextcloudPhpConfig = {
    # Subpath deployment keys: the front vhost proxies /nextcloud with the
    # path preserved, so Nextcloud must generate /nextcloud-prefixed URLs.
    overwritewebroot = phpStr "/nextcloud";
    "htaccess.RewriteBase" = phpStr "/nextcloud";
    "overwrite.cli.url" = phpStr (
      if proxied then "https://${proxyHostName}/nextcloud" else "http://${hostName}.local/nextcloud"
    );
    # Requests arrive with the appliance's mDNS name (direct) or the
    # master-proxy public hostname (tunnel) — both must be trusted, or
    # Nextcloud rejects tunnel traffic with "Untrusted domain".
    trusted_domains = phpList ([ "${hostName}.local" ] ++ lib.optional proxied proxyHostName);
    # Loopback, not a container address: the veth is gone and Nginx now
    # reaches the pod over the host's own loopback. Without this Nextcloud
    # sees one client address for every request, so the brute-force throttle
    # and per-IP blocking protect nothing and the audit log records a single
    # source.
    trusted_proxies = phpList [ "127.0.0.1" ];
  }
  # Behind the master proxy Traefik terminates TLS and the pod is reached over
  # plain HTTP. Without these Nextcloud derives http:// absolute URLs and
  # embeds them in an https:// page — mixed content, blocked by browsers, and
  # clients redirected back to http.
  // lib.optionalAttrs proxied {
    overwriteprotocol = phpStr "https";
    overwritehost = phpStr proxyHostName;
  };

  nextcloudConf = confDir "losos-nextcloud-conf" {
    # The one file carrying losos.nextcloud.apachePort. 127.0.0.1 is
    # load-bearing under hostNetwork: httpd would otherwise bind every address
    # on the box, including the LAN one, behind Nginx's back.
    "listen.conf" = pkgs.writeText "listen.conf" "Listen 127.0.0.1:${toString apachePort}\n";
    "losos.config.php" = pkgs.writeText "losos.config.php" ''
      <?php
      // Rendered by modules/workloads.nix and bind-mounted read-only from the
      // store. Editing it on the box is pointless: the next rebuild replaces it.
      $CONFIG = [
      ${
        lib.concatStrings (
          lib.mapAttrsToList (key: value: "  ${phpStr key} => ${value},\n") nextcloudPhpConfig
        )
      }];
    '';
  };

  # ── Forgejo's rendered config ────────────────────────────────────────────
  forgejoConf = confDir "losos-forgejo-conf" {
    "losos.ini" = ini.generate "losos.ini" {
      server = {
        # Same reasoning as Nextcloud's Listen line: under hostNetwork an
        # unqualified bind is a public one. The port is a literal on both ends
        # (here and in modules/containers.nix) on purpose — nothing off-box ever
        # sees it, so an option would be a knob with no reason to be turned.
        HTTP_ADDR = "127.0.0.1";
        HTTP_PORT = 3000;
        # The front vhost strips the /forgejo prefix, so Forgejo has to be told
        # it is served under a subpath or every link it generates is wrong.
        ROOT_URL =
          if proxied then "https://${proxyHostName}/forgejo/" else "http://${hostName}.local/forgejo/";
      };
      service = {
        # /forgejo/ is the one admin-free route published through the
        # master-proxy tunnel, and Forgejo's default is open sign-up. Without
        # this anyone on the internet could create an account and push to this
        # box.
        DISABLE_REGISTRATION = true;
      };
      security = {
        # Close the first-run installer page. It is normally locked by
        # completing the wizard, but a declarative deployment never runs it —
        # leaving /forgejo/install reachable, and that page rewrites the
        # database and admin credentials.
        INSTALL_LOCK = true;
      };
      actions = {
        # Actions is remote code execution by design and this appliance
        # registers no runner, so enabling it on an internet-reachable route
        # buys nothing and exposes the runner-registration API. Flip to true
        # together with an actual runner.
        ENABLED = false;
      };
    };
  };

  # ── The pod shape both workloads share ───────────────────────────────────
  # Kept in one place so the load-bearing fields (hostNetwork, dnsPolicy, the
  # dropped capabilities) cannot drift between the two manifests.
  #
  # priorityClassName is system-node-critical because these *are* the appliance:
  # if the kubelet starts evicting under pressure, the box's own file storage
  # and git host are the last things that should go. The kubelet resolves the
  # system-* classes locally for static pods, so this needs no apiserver object.
  staticPod =
    {
      name,
      image,
      uid,
      groups ? [ ],
      mounts,
      volumes,
    }:
    yaml.generate "${name}.yaml" {
      apiVersion = "v1";
      kind = "Pod";
      metadata = {
        inherit name;
        namespace = "kube-system";
        labels = {
          "losos.dev/workload" = name;
        };
      };
      spec = {
        hostNetwork = true;
        dnsPolicy = "Default";
        priorityClassName = "system-node-critical";
        restartPolicy = "Always";
        securityContext = {
          runAsUser = uid;
          runAsGroup = uid;
          # Inert for hostPath volumes — the kubelet only applies fsGroup to
          # volume plugins that manage ownership, and hostPath deliberately does
          # not (recursively chowning /run/postgresql would be a catastrophe).
          # It is here for its second effect: the container process gets the gid
          # as a supplementary group.
          fsGroup = uid;
        }
        // lib.optionalAttrs (groups != [ ]) { supplementalGroups = groups; };
        containers = [
          {
            inherit name image;
            imagePullPolicy = "Never";
            volumeMounts = mounts;
            securityContext = {
              allowPrivilegeEscalation = false;
              capabilities.drop = [ "ALL" ];
            };
          }
        ];
        inherit volumes;
      };
    };

  nextcloudPod = staticPod {
    name = "nextcloud";
    image = imageRef nextcloudImage;
    uid = nextcloudUid;
    groups = lib.optional (redisGid != null) redisGid;
    mounts = [
      {
        name = "state";
        mountPath = nextcloudState;
      }
      {
        name = "conf";
        mountPath = "/etc/losos/nextcloud";
        readOnly = true;
      }
      {
        name = "adminpass";
        mountPath = adminpassFile;
        readOnly = true;
      }
      {
        name = "pgsock";
        mountPath = pgSocketDir;
      }
      {
        name = "redissock";
        mountPath = redisSocketDir;
      }
    ]
    ++ lib.optional gpu {
      name = "dri";
      mountPath = "/dev/dri";
    };
    volumes = [
      {
        name = "state";
        hostPath = {
          path = nextcloudState;
          type = "Directory";
        };
      }
      {
        name = "conf";
        hostPath = {
          path = "${nextcloudConf}";
          type = "Directory";
        };
      }
      {
        # type: File, so a missing password file is a loud pod failure instead
        # of an empty directory the kubelet helpfully creates in its place.
        name = "adminpass";
        hostPath = {
          path = adminpassFile;
          type = "File";
        };
      }
      {
        name = "pgsock";
        hostPath = {
          path = pgSocketDir;
          type = "Directory";
        };
      }
      {
        name = "redissock";
        hostPath = {
          path = redisSocketDir;
          type = "Directory";
        };
      }
    ]
    # Mounting the device nodes is necessary but may not be sufficient: the
    # nspawn path also granted `DeviceAllow = char-drm rw` on the unit, and a pod
    # has no equivalent knob short of privileged: true or a device plugin. If
    # VA-API comes back "permission denied" inside the pod, that cgroup device
    # rule — not this mount — is what is missing.
    ++ lib.optional gpu {
      name = "dri";
      hostPath = {
        path = "/dev/dri";
        type = "Directory";
      };
    };
  };

  forgejoPod = staticPod {
    name = "forgejo";
    image = imageRef forgejoImage;
    uid = forgejoUid;
    mounts = [
      {
        name = "state";
        mountPath = forgejoState;
      }
      {
        name = "conf";
        mountPath = "/etc/losos/forgejo";
        readOnly = true;
      }
      {
        name = "pgsock";
        mountPath = pgSocketDir;
      }
    ];
    volumes = [
      {
        name = "state";
        hostPath = {
          path = forgejoState;
          type = "Directory";
        };
      }
      {
        name = "conf";
        hostPath = {
          path = "${forgejoConf}";
          type = "Directory";
        };
      }
      {
        name = "pgsock";
        hostPath = {
          path = pgSocketDir;
          type = "Directory";
        };
      }
    ];
  };
in
{
  assertions = [
    {
      assertion = nextcloudWorkload -> nextcloudImage != null;
      message = ''
        losos.nextcloud.mode = "container" needs losos.workloads.nextcloudImage,
        which modules/defaults.nix wires to this flake's own image derivation
        (flake/images.nix). Either restore that wiring or set
        losos.nextcloud.mode = "native".
      '';
    }
    {
      assertion = forgejoWorkload -> forgejoImage != null;
      message = ''
        losos.forgejo.mode = "container" needs losos.workloads.forgejoImage,
        which modules/defaults.nix wires to this flake's own image derivation
        (flake/images.nix). Either restore that wiring, set
        losos.forgejo.mode = "native", or set losos.forgejo.enable = false.
      '';
    }
  ];

  # ── The static-pod directory ─────────────────────────────────────────────
  # tmpfiles runs at sysinit.target, long before k3s, so the manifests are in
  # place before the kubelet first reads the directory — and
  # systemd-tmpfiles-resetup.service re-runs `systemd-tmpfiles --create
  # --remove` on every `nixos-rebuild switch` (it carries a restart trigger on
  # /etc/tmpfiles.d), which is what repoints an L+ symlink when a rendered
  # config changes its store path.
  #
  # The `r` lines are the other half of that and are easy to leave out: dropping
  # a workload only removes its L+ line, and --remove does not clean up entries
  # that are no longer listed. Without an explicit removal, flipping a service
  # from "container" back to "native" would leave a live static pod fighting the
  # native service for the same state directory and the same port, with nothing
  # in the config to suggest why.
  #
  # The same mechanism carries the MESH instance's airgap image symlinks under
  # /var/lib/rancher/rke2/agent/images, which is why nothing losos ships may
  # rm -rf that rancher tree: those symlinks are made once at sysinit and a
  # later wipe leaves the agent with no images at all until the next boot. The
  # local cluster's images are not symlinks and are not there — it runs a native
  # containerd, and its images live in /var/lib/containerd (modules/cluster.nix).
  systemd.tmpfiles.rules =
    lib.optionals anyWorkload [
      # lososd's StateDirectory. Declared here as well because tmpfiles creates
      # missing parents with the default 0755, and this one holds state.json.
      "d /var/lib/losos 0700 root root -"
      "d ${staticPodDir} 0700 root root -"
    ]
    ++ (
      if nextcloudWorkload then
        [
          # Numeric ids, not names: in container mode the host may have no
          # `nextcloud` user at all, and these are the same literals the pod's
          # runAsUser carries, from the same binding.
          "d ${nextcloudState} 0750 ${toString nextcloudUid} ${toString nextcloudUid} -"
          # modules/nextcloud-common.nix mints the password file 0600
          # root:root, and the pod runs as uid 1002 — so it is mounted and
          # unreadable, which Nextcloud reports as a failed install rather than
          # as a permission problem. Root still reads it either way, so the
          # native path is unaffected. `z` fixes a file that already exists;
          # the ExecStartPost below covers the first boot, where the file is
          # created at multi-user.target, long after tmpfiles has run.
          "z ${adminpassFile} 0600 ${toString nextcloudUid} ${toString nextcloudUid} -"
          "L+ ${staticPodDir}/nextcloud.yaml - - - - ${nextcloudPod}"
        ]
      else
        [ "r ${staticPodDir}/nextcloud.yaml" ]
    )
    ++ (
      if forgejoWorkload then
        [
          "d ${forgejoState} 0750 ${toString forgejoUid} ${toString forgejoUid} -"
          "L+ ${staticPodDir}/forgejo.yaml - - - - ${forgejoPod}"
        ]
      else
        [ "r ${staticPodDir}/forgejo.yaml" ]
    );

  systemd.services = lib.mkMerge [
    # ── What the kubelet needs before it starts these pods ─────────────────
    # Every one of these is a hostPath the pods mount with an explicit `type:`,
    # so a directory that does not exist yet is a pod that never starts, not a
    # directory the kubelet invents. Ordering against a unit that does not exist
    # on this box is silently ignored by systemd (unlike Requires), so one list
    # covers both service modes.
    (lib.mkIf (anyWorkload && config.services.k3s.enable) {
      k3s = {
        after = [
          "postgresql.service"
          "redis-nextcloud.service"
          "losos-nextcloud-adminpass.service"
          # The shared domain must be fully unlocked or fully locked before any
          # pod can see it — never half-way through (modules/fscrypt.nix).
          "losos-fscrypt-shared.service"
        ];
        wants = [
          "postgresql.service"
          "redis-nextcloud.service"
        ];
        # Requires, not merely After: the adminpass file is mounted with
        # hostPath type: File. Scoped to the Nextcloud path so a Forgejo-only
        # box does not hold its whole cluster hostage to a Nextcloud secret.
        requires = lib.optional nextcloudWorkload "losos-nextcloud-adminpass.service";
      };
    })

    # ── Re-order the adminpass generator onto this path ────────────────────
    # modules/nextcloud-common.nix orders it before nextcloud-setup.service and
    # phpfpm-nextcloud.service, and neither of those exists in container mode —
    # so on the k8s path the ordering constrained nothing at all and the kubelet
    # could reach the mount before the file existed. This merges k3s.service
    # into that `before` list (the module system concatenates the two
    # definitions) rather than editing the generator, which the native path
    # still needs unchanged.
    (lib.mkIf nextcloudWorkload {
      losos-nextcloud-adminpass = {
        before = [ "k3s.service" ];
        # See the `z` tmpfiles line above: this is the first-boot half, run in
        # the same unit that mints the secret, because tmpfiles has long since
        # passed by the time it exists.
        serviceConfig.ExecStartPost = "${pkgs.coreutils}/bin/chown ${toString nextcloudUid}:${toString nextcloudUid} ${adminpassFile}";
      };
    })
  ];
}
