# SkyPilot on the edge (losos.edge.skypilot).
#
# The edge runs the SkyPilot API server against its rke2 mesh cluster, so AI
# jobs (model serving, fine-tuning, batch) can be scheduled over the boxes
# that join the mesh. This module deploys a pinned SkyPilot release onto the
# cluster the way modules/edge-vms.nix deploys KubeVirt/CDI: pinned upstream
# manifests fetched at build time, applied by rke2 from its manifests
# directory, so a rebuild at 03:00 resolves nothing new.
#
# Phase 1 Task 2: module skeleton + option. The pinned artifact hash is a
# builder step (fill in `skypilotVersion`/`skypilotManifest` from a real
# SkyPilot release). Until that hash lands the module is inert.
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.losos.edge.skypilot;

  # TODO (builder): pin a real SkyPilot release and its rendered manifest hash.
  skypilotVersion = "1.0.0.dev0";
  skypilotManifest = pkgs.fetchurl {
    url = "https://github.com/skypilot-org/skypilot/releases/download/v${skypilotVersion}/skypilot-k8s.yaml";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  };
in
{
  config = lib.mkIf (config.losos.edge.enable && cfg.enable) {
    # Drop the pinned manifest into rke2's auto-applied manifests dir.
    environment.etc."rancher/rke2/server/manifests/skypilot.yaml".source = skypilotManifest;

    # Surface the API server port to the edge's reverse proxy (TLS + auth
    # handled by the edge's existing front door, like the registrar).
    networking.firewall.allowedTCPPorts = [ cfg.port ];
  };
}
