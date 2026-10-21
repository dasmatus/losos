# The local cluster's own workload: Forgejo, as a Kubernetes *static pod*.
#
# `losos.<svc>.mode == "container"` used to mean a systemd-nspawn container on a
# private veth; it now means a pod in this box's own k3s server.
# modules/cluster.nix runs the cluster (and preloads the images listed in
# losos.workloads.*), modules/containers.nix keeps the Nginx front door that
# proxies to them, and this file owns the Kubernetes objects plus the config
# they mount. Nextcloud ran here too; it is gone, and LosOS Cloud
# (modules/cloud.nix) takes in what a box's Nextcloud held.
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
# appliance comes up with no Git and no shell to find out why.
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
#   * every listener binds 127.0.0.1 explicitly (Forgejo's HTTP_ADDR). Nginx stays the only thing holding a public port.
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
# The rendered files carry option values (the host name, the owner's site
# settings), and an image cannot be rebuilt on a mini-PC at 03:00 with a tmpfs
# root. They are rendered in module context and hostPath-mounted, so changing
# an option changes a store path, which changes the manifest, which changes the
# tmpfiles symlink, and the kubelet restarts the pod on the next
# `nixos-rebuild switch`. No image is ever rebuilt.
#
# Every hostPath volume declares an explicit `type:`. Without it the kubelet
# CREATES A DIRECTORY at a missing path, which turns a missing socket into an
# empty directory the pod then waits on forever.
{
  pkgs,
  lib,
  config,
  ...
}:

let
  # losos.forgejo.enable is consulted in *both* modes; see modules/containers.nix.
  forgejoWorkload = config.losos.forgejo.mode == "container" && config.losos.forgejo.enable;
  anyWorkload = forgejoWorkload;

  hostName = config.losos.hostName;
  proxied = config.losos.proxy.enable;
  proxyHostName = config.losos.proxy.hostname;

  forgejoImage = config.losos.workloads.forgejoImage;

  # Durable state. Covered by impermanence's whole-/var bind mount, so
  # nothing has to be added to modules/impermanence.nix for it.
  forgejoState = "/var/lib/forgejo";

  # Host socket the pod reaches its database through. Postgres runs natively
  # rather than in the pod: under hostNetwork an in-pod Postgres binds
  # the *host's* 0.0.0.0:5432 unless hand-configured, and "nothing but Nginx
  # holds a public port" only survives the native form. See modules/services.nix.
  pgSocketDir = "/run/postgresql";

  # Fixed uids, pinned in modules/configuration.nix next to notshared/shared.
  # They are not cosmetic: Postgres peer auth over /run/postgresql compares the
  # *uid* of the connecting process, and the pod shares the host's user
  # namespace, so the pod's runAsUser is what Postgres maps to a role. gid ==
  # uid by construction there, hence one binding.
  signIn = import ./app-sign-in.nix { inherit lib; };
  forgejoUid = 1003;

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

  # ── Forgejo's rendered config ────────────────────────────────────────────
  forgejoConf = confDir "losos-forgejo-conf" {
    # The owner's box-wide settings (losos.forgejo.site.*,
    # lososInternal.forgejoSite) under LosOS's own, as natively.
    "losos.ini" = ini.generate "losos.ini" (
      lib.recursiveUpdate
        (lib.recursiveUpdate config.lososInternal.forgejoSite (
          # Sign-in with LosOS Login (modules/app-sign-in.nix): nginx names
          # the person, Forgejo signs them in. Under LosOS's own sections so
          # nothing below can turn DISABLE_REGISTRATION off with it.
          lib.optionalAttrs config.lososInternal.appSignIn.forgejo signIn.forgejo
        ))
        {
          server = {
            # Under hostNetwork an unqualified bind is a public one. The port is a literal on both ends
            # (here and in modules/containers.nix) on purpose — nothing off-box ever
            # sees it, so an option would be a knob with no reason to be turned.
            HTTP_ADDR = "127.0.0.1";
            HTTP_PORT = 3000;
            # The front vhost strips the /forgejo prefix, so Forgejo has to be told
            # it is served under a subpath or every link it generates is wrong.
            # With losos.tls on (the default) the wizard moves the owner to
            # https, so that is the address: an http ROOT_URL put Forgejo's
            # "configured to be served on a different URL" banner on every page
            # the owner opened, sign-in included. :80 still answers.
            ROOT_URL =
              if proxied then
                "https://${proxyHostName}/forgejo/"
              else if config.losos.tls.enable then
                "https://${hostName}.local/forgejo/"
              else
                "http://${hostName}.local/forgejo/";
          };
          session = {
            # Forgejo marks its session cookie Secure whenever ROOT_URL is
            # https, and a browser drops a Secure cookie set over http. :80
            # keeps serving for an owner who has not trusted the box's
            # certificate yet, and sign-in there has to keep working.
            COOKIE_SECURE = false;
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
            # Actions is remote code execution by design and nothing on the box
            # runs jobs: the on-box runner that existed for the official-edge
            # key ceremony is gone (the ceremony runs on the operator's machine,
            # `losos-registrar provision`, behind a GitHub sign-in). Enabled with
            # nothing to run jobs, it would only expose the runner-registration
            # API on an internet-reachable route. Same setting as native mode
            # (modules/services.nix).
            ENABLED = false;
          };
          federation = {
            # ActivityPub: nodeinfo, one actor per account and repository,
            # federated stars and follows (modules/options.nix says what the
            # shipped Forgejo does with it). The front vhost adds the two
            # /.well-known routes other servers discover the box by
            # (modules/containers.nix); Forgejo serves everything else under
            # ROOT_URL. Same two lines as native mode (modules/services.nix).
            # On only while the box shares its disk (lososInternal.federation).
            ENABLED = config.lososInternal.federation.forgejo;
            # nodeinfo's `usage` block: account totals and activity counts of a
            # household's box, published to anyone who asks. Off. Forgejo's own
            # default is on.
            SHARE_USER_STATISTICS = false;
          };
        }
    );
  };

  # ── The pod shape ────────────────────────────────────────────────────────
  # Kept apart from the pod so the load-bearing fields (hostNetwork, dnsPolicy,
  # the dropped capabilities) read in one place.
  #
  # priorityClassName is system-node-critical because these *are* the appliance:
  # if the kubelet starts evicting under pressure, the box's own git host is
  # the last thing that should go. The kubelet resolves the
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

  # One host path in a pod: the volume and where the container sees it.
  bind =
    {
      name,
      path,
      at ? path,
      type ? "Directory",
      readOnly ? false,
    }:
    {
      mount = {
        inherit name;
        mountPath = at;
      }
      // lib.optionalAttrs readOnly { inherit readOnly; };
      volume = {
        inherit name;
        hostPath = { inherit path type; };
      };
    };
  binds = list: {
    mounts = map (b: b.mount) list;
    volumes = map (b: b.volume) list;
  };

  forgejoPod = staticPod (
    {
      name = "forgejo";
      image = imageRef forgejoImage;
      uid = forgejoUid;
    }
    // binds (
      map bind [
        {
          name = "state";
          path = forgejoState;
        }
        {
          name = "conf";
          path = "${forgejoConf}";
          at = "/etc/losos/forgejo";
          readOnly = true;
        }
        {
          name = "pgsock";
          path = pgSocketDir;
        }
      ]
    )
  );
in
{
  assertions = [
    {
      assertion = forgejoWorkload -> forgejoImage != null;
      message = ''
        losos.forgejo.mode = "container" needs losos.workloads.forgejoImage,
        which modules/defaults.nix wires to this flake's own image derivation
        (nix/images.nix). Either restore that wiring, set
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
    # Nextcloud's pod is gone. A box that ran it still has its manifest link
    # here, and tmpfiles' --remove does not drop a line nobody lists any more,
    # so the kubelet would go on running it beside LosOS Cloud's move of its
    # files (modules/cloud.nix stops it too, for the boot that races this).
    ++ [ "r ${staticPodDir}/nextcloud.yaml" ]
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
          # The shared domain must be fully unlocked or fully locked before any
          # pod can see it — never half-way through (modules/fscrypt.nix).
          "losos-fscrypt-shared.service"
        ];
        wants = [ "postgresql.service" ];
      };
    })
  ];
}
