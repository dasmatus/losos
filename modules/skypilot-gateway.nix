# Gateway-local SkyPilot (Phase 3).
#
# When a box acts as a local edge gateway (`losos.edge.gateway.enable`) and
# kinda-shared capacity should be schedulable, the gateway runs its own
# SkyPilot API server over a gateway-local k3s cluster. Kinda-shared boxes
# join that cluster (not the official edge's rke2 cluster), so the official
# edge never touches kinda-shared capacity.
#
# This is a skeleton for Phase 3; the pinned SkyPilot manifest/artifacts and
# cluster bootstrap details are to be filled in (pin bot may update hashes).
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.losos.edge.gateway.skypilot;
  gw = config.losos.edge.gateway;
in
{
  options.losos.edge.gateway.skypilot = {
    enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Run a gateway-local SkyPilot API server over a k3s cluster, so
        kinda-shared capacity in this network is schedulable without
        involving the official edge (Phase 3).
      '';
    };
    port = lib.mkOption {
      type = lib.types.port;
      default = 46581;
      description = "Port the gateway-local SkyPilot API server listens on.";
    };
  };

  config = lib.mkIf (config.losos.edge.enable && gw.enable && cfg.enable) {
    # Gateway-local k3s server for SkyPilot scheduling within this network.
    # Separate from the mesh rke2 cluster; uses k3s because gateways are
    # smaller than the edge VPS.
    services.k3s = {
      enable = true;
      role = "server";
      extraFlags = toString [
        "--write-kubeconfig-mode=0640"
        "--disable=traefik"
        "--disable=servicelb"
        "--disable=local-storage"
      ];
    };

    # Pinned SkyPilot API server manifest for the gateway-local cluster.
    # TODO: fill in real release URL/hash (GH Actions pin bot may update).
    environment.etc."rancher/k3s/server/manifests/skypilot-gateway.yaml".text = ''
      # SkyPilot gateway-local server placeholder.
      # Replace with pinned upstream manifest for the gateway role.
      apiVersion: v1
      kind: Namespace
      metadata:
        name: skypilot
    '';

    networking.firewall.allowedTCPPorts = [ cfg.port ];
  };
}
