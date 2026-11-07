# LosOS Forum scaffold (Phase 0).
#
# Design: docs/superpowers/specs/2026-11-05-losos-forum-design.md
#
# From-scratch Discourse+Slack hybrid with @LosOS mentions, ActivityPub
# federation and Matrix DM bridging (DMs appear in LosOS Chat), routed via the
# selected edge/proxy network. This module is an early scaffold: it wires
# options/gating/placeholders; the full service, adapters, UI, and bridge are
# not implemented yet.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.losos.forum;
  live = config.lososInternal.forum;
  stateDir = "/var/lib/losos/forum";
  statusFile = "${stateDir}/scaffold-status.json";
in
{
  config = lib.mkMerge [
    (lib.mkIf cfg.enable {
      assertions = [
        {
          assertion = config.losos.proxy.enable;
          message = ''
            losos.forum.enable requires losos.proxy.enable (edge proxy network /
            edge-local routing dependency). The full mesh-join gate
            (losos.cluster.enable / rke2 agent) is an open design decision; see
            docs/superpowers/specs/2026-11-05-losos-forum-design.md.
          '';
        }
        {
          assertion = config.losos.admin.enable && config.losos.admin.ui != null;
          message = "losos.forum.enable requires the admin UI (losos.admin.enable and losos.admin.ui).";
        }
      ];

      systemd.tmpfiles.rules = [
        "d ${stateDir} 0750 root root -"
      ];
    })

    (lib.mkIf live {
      # Placeholder status for observability; replaced by a real forum service.
      systemd.services.losos-forum-scaffold = {
        description = "LosOS Forum scaffold status (placeholder; full service pending)";
        wantedBy = [ "multi-user.target" ];
        after = [ "network.target" ];
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
        };
        script = ''
          ${pkgs.coreutils}/bin/cat > ${statusFile} <<JSON
          {
            "scaffold": true,
            "implemented": false,
            "edgeLocalRouting": ${if cfg.edgeLocalRouting then "true" else "false"},
            "activityPub": ${if cfg.activityPub.enable then "true" else "false"},
            "matrixDmBridge": ${if cfg.matrixDmBridge.enable then "true" else "false"},
            "assistant": ${if cfg.assistant.enable then "true" else "false"},
            "design": "docs/superpowers/specs/2026-11-05-losos-forum-design.md"
          }
          JSON
          ${pkgs.coreutils}/bin/chmod 0640 ${statusFile}
        '';
      };

      # Placeholders for future lososd integration.
      systemd.services.lososd.environment = {
        LOSOS_FORUM_SCAFFOLD = "1";
        LOSOS_FORUM_STATE_DIR = stateDir;
        LOSOS_FORUM_EDGE_LOCAL = if cfg.edgeLocalRouting then "1" else "0";
        LOSOS_FORUM_ACTIVITYPUB = if cfg.activityPub.enable then "1" else "0";
        LOSOS_FORUM_MATRIX_DM_BRIDGE = if cfg.matrixDmBridge.enable then "1" else "0";
        LOSOS_FORUM_ASSISTANT = if cfg.assistant.enable then "1" else "0";
      };
    })
  ];
}
