# Virtual machines on the mesh — edge side (losos.edge.vms).
#
# KubeVirt runs the machines and CDI imports their disks, both on the edge's
# rke2 cluster, deployed the way Longhorn is: pinned upstream manifests that
# rke2 applies from its manifests directory, fetched at build time so a
# rebuild at 03:00 resolves nothing new. The registrar (backend-registrar's
# vms.rs and server/vm.rs) sells the replicas and creates the
# VirtualMachines; this file only gives the cluster the parts it needs to
# run them:
#
#   * the KubeVirt and CDI operators and their custom resources. KubeVirt's
#     node agent runs only on boxes (nodes labelled losos.dev/appliance) and
#     tolerates the compute-window taint, because a hosted machine runs
#     around the clock; CDI's importer pods tolerate it too, or a disk could
#     only be imported inside the owner's sharing window.
#   * `losos-vm-local`, a local-path storage class whose volumes live on the
#     box that runs the machine, under losos.edge.vms.hostPath (the shared
#     user's home by default). WaitForFirstConsumer, so a disk is made only
#     once its machine has a node, and Retain, so nothing the provisioner
#     does deletes a buyer's disk. rke2 ships no storage class of its own.
#
# What a box needs in return (kvm, tun and vhost_net) is in
# modules/cluster.nix, gated on the same sharing switch as the join flag.
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.losos.edge;
  vms = cfg.vms;

  # KubeVirt v1.9.0 (2026-07-30) and CDI v1.66.1 (2026-09-06), the operator
  # manifests from each project's GitHub release.
  kubevirtVersion = "v1.9.0";
  cdiVersion = "v1.66.1";
  kubevirtOperator = pkgs.fetchurl {
    url = "https://github.com/kubevirt/kubevirt/releases/download/${kubevirtVersion}/kubevirt-operator.yaml";
    hash = "sha256-8RMHyq/Dwj/+35iH2L61pEGeJpTaJC+mj2PR7IIN4uA=";
  };
  cdiOperator = pkgs.fetchurl {
    url = "https://github.com/kubevirt/containerized-data-importer/releases/download/${cdiVersion}/cdi-operator.yaml";
    hash = "sha256-x9kr0bLuGjlSpZkAAEN8CNWBCfMcC9SKthiOwnoT4iQ=";
  };

  applianceLabel = "losos.dev/appliance";
  windowToleration = {
    key = "losos.dev/compute-window";
    operator = "Exists";
    effect = "NoSchedule";
  };
  onBoxes = {
    nodeAffinity.requiredDuringSchedulingIgnoredDuringExecution.nodeSelectorTerms = [
      {
        matchExpressions = [
          {
            key = applianceLabel;
            operator = "Exists";
          }
        ];
      }
    ];
  };

  kubevirtCr = {
    apiVersion = "kubevirt.io/v1";
    kind = "KubeVirt";
    metadata = {
      name = "kubevirt";
      namespace = "kubevirt";
    };
    spec = {
      certificateRotateStrategy = { };
      configuration = {
        developerConfiguration = {
          featureGates = [ ];
        }
        // lib.optionalAttrs vms.useEmulation { useEmulation = true; };
      };
      customizeComponents = { };
      imagePullPolicy = "IfNotPresent";
      workloadUpdateStrategy = { };
      workloads.nodePlacement = {
        affinity = onBoxes;
        tolerations = [ windowToleration ];
      };
    };
  };

  cdiCr = {
    apiVersion = "cdi.kubevirt.io/v1beta1";
    kind = "CDI";
    metadata.name = "cdi";
    spec = {
      config = {
        featureGates = [ "HonorWaitForFirstConsumer" ];
        # Scratch space (a registry import unpacks there first) is claimed
        # on the same box as the disk.
        scratchSpaceStorageClass = storageClass;
      };
      imagePullPolicy = "IfNotPresent";
      infra.nodeSelector."kubernetes.io/os" = "linux";
      workload = {
        nodeSelector."kubernetes.io/os" = "linux";
        tolerations = [ windowToleration ];
      };
    };
  };

  # rancher/local-path-provisioner v0.0.37's deploy/local-path-storage.yaml,
  # under its own names so it cannot collide with a copy an operator
  # installed: namespace, provisioner name and class are LosOS's.
  storageClass = "losos-vm-local";
  lppNamespace = "losos-vm-storage";
  lppName = "losos-vm-local";
  lppProvisioner = "losos.dev/vm-local";
  lppImage = "docker.io/rancher/local-path-provisioner:v0.0.37";
  helperImage = "docker.io/library/busybox:1.37.0";
  lppConfig = builtins.toJSON {
    nodePathMap = [
      {
        node = "DEFAULT_PATH_FOR_NON_LISTED_NODES";
        paths = [ vms.hostPath ];
      }
    ];
  };
  helperPod = builtins.toJSON {
    apiVersion = "v1";
    kind = "Pod";
    metadata.name = "helper-pod";
    spec = {
      priorityClassName = "system-node-critical";
      tolerations = [
        {
          key = "node.kubernetes.io/disk-pressure";
          operator = "Exists";
          effect = "NoSchedule";
        }
        windowToleration
      ];
      containers = [
        {
          name = "helper-pod";
          image = helperImage;
          imagePullPolicy = "IfNotPresent";
        }
      ];
    };
  };
  localPath = [
    {
      apiVersion = "v1";
      kind = "Namespace";
      metadata.name = lppNamespace;
    }
    {
      apiVersion = "v1";
      kind = "ServiceAccount";
      metadata = {
        name = lppName;
        namespace = lppNamespace;
      };
    }
    {
      apiVersion = "rbac.authorization.k8s.io/v1";
      kind = "Role";
      metadata = {
        name = lppName;
        namespace = lppNamespace;
      };
      rules = [
        {
          apiGroups = [ "" ];
          resources = [ "pods" ];
          verbs = [
            "get"
            "list"
            "watch"
            "create"
            "patch"
            "update"
            "delete"
          ];
        }
      ];
    }
    {
      apiVersion = "rbac.authorization.k8s.io/v1";
      kind = "ClusterRole";
      metadata.name = lppName;
      rules = [
        {
          apiGroups = [ "" ];
          resources = [
            "nodes"
            "persistentvolumeclaims"
            "configmaps"
            "pods"
            "pods/log"
          ];
          verbs = [
            "get"
            "list"
            "watch"
          ];
        }
        {
          apiGroups = [ "" ];
          resources = [ "persistentvolumes" ];
          verbs = [
            "get"
            "list"
            "watch"
            "create"
            "patch"
            "update"
            "delete"
          ];
        }
        {
          apiGroups = [ "" ];
          resources = [ "events" ];
          verbs = [
            "create"
            "patch"
          ];
        }
        {
          apiGroups = [ "storage.k8s.io" ];
          resources = [ "storageclasses" ];
          verbs = [
            "get"
            "list"
            "watch"
          ];
        }
      ];
    }
    {
      apiVersion = "rbac.authorization.k8s.io/v1";
      kind = "RoleBinding";
      metadata = {
        name = lppName;
        namespace = lppNamespace;
      };
      roleRef = {
        apiGroup = "rbac.authorization.k8s.io";
        kind = "Role";
        name = lppName;
      };
      subjects = [
        {
          kind = "ServiceAccount";
          name = lppName;
          namespace = lppNamespace;
        }
      ];
    }
    {
      apiVersion = "rbac.authorization.k8s.io/v1";
      kind = "ClusterRoleBinding";
      metadata.name = lppName;
      roleRef = {
        apiGroup = "rbac.authorization.k8s.io";
        kind = "ClusterRole";
        name = lppName;
      };
      subjects = [
        {
          kind = "ServiceAccount";
          name = lppName;
          namespace = lppNamespace;
        }
      ];
    }
    {
      apiVersion = "apps/v1";
      kind = "Deployment";
      metadata = {
        name = lppName;
        namespace = lppNamespace;
      };
      spec = {
        replicas = 1;
        selector.matchLabels.app = lppName;
        template = {
          metadata.labels.app = lppName;
          spec = {
            serviceAccountName = lppName;
            containers = [
              {
                name = "local-path-provisioner";
                image = lppImage;
                imagePullPolicy = "IfNotPresent";
                command = [
                  "local-path-provisioner"
                  "start"
                  "--config"
                  "/etc/config/config.json"
                  "--provisioner-name"
                  lppProvisioner
                  "--configmap-name"
                  lppName
                  "--service-account-name"
                  lppName
                  "--helper-image"
                  helperImage
                ];
                ports = [
                  {
                    name = "health";
                    containerPort = 8080;
                    protocol = "TCP";
                  }
                ];
                livenessProbe = {
                  httpGet = {
                    path = "/health";
                    port = "health";
                  };
                  initialDelaySeconds = 10;
                  periodSeconds = 10;
                };
                volumeMounts = [
                  {
                    name = "config-volume";
                    mountPath = "/etc/config/";
                  }
                ];
                env = [
                  {
                    name = "POD_NAMESPACE";
                    valueFrom.fieldRef.fieldPath = "metadata.namespace";
                  }
                  {
                    name = "CONFIG_MOUNT_PATH";
                    value = "/etc/config/";
                  }
                  {
                    name = "HEALTH_PORT";
                    value = "8080";
                  }
                ];
              }
            ];
            volumes = [
              {
                name = "config-volume";
                configMap.name = lppName;
              }
            ];
          };
        };
      };
    }
    {
      apiVersion = "storage.k8s.io/v1";
      kind = "StorageClass";
      metadata.name = storageClass;
      provisioner = lppProvisioner;
      volumeBindingMode = "WaitForFirstConsumer";
      reclaimPolicy = "Retain";
    }
    {
      apiVersion = "v1";
      kind = "ConfigMap";
      metadata = {
        name = lppName;
        namespace = lppNamespace;
      };
      data = {
        "config.json" = lppConfig;
        setup = ''
          #!/bin/sh
          set -eu
          mkdir -m 0700 -p "$VOL_DIR"
        '';
        # Never run (the class retains its volumes), but the provisioner
        # refuses a config without one. It removes nothing.
        teardown = ''
          #!/bin/sh
          exit 0
        '';
        "helperPod.yaml" = helperPod;
      };
    }
  ];
in
{
  config = lib.mkIf (cfg.enable && vms.enable) {
    assertions = [
      {
        assertion = cfg.market.enable && cfg.cluster.enable;
        message = ''
          losos.edge.vms.enable requires losos.edge.market.enable and
          losos.edge.cluster.enable: machines are sold on the market and run
          on the mesh.
        '';
      }
    ];

    services.rke2.manifests = {
      losos-kubevirt-operator.source = kubevirtOperator;
      losos-kubevirt.content = kubevirtCr;
      losos-cdi-operator.source = cdiOperator;
      losos-cdi.content = cdiCr;
      losos-vm-local.content = localPath;
    };
  };
}
