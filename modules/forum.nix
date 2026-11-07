# LosOS Forum scaffold (Phase 0+ edge/federation hooks).
#
# Design: docs/superpowers/specs/2026-11-05-losos-forum-design.md
#
# From-scratch Discourse+Slack hybrid with @LosOS mentions, ActivityPub
# federation and Matrix DM bridging (DMs appear in LosOS Chat), routed via the
# selected edge/proxy network.
#
# SCAFFOLD SAFETY:
# - This module wires options, gates, env/config plumbing, nginx hook points,
#   and explicit integration contracts/stubs only.
# - It does NOT implement ActivityPub delivery, HTTP signature verification,
#   inbox processing, Matrix appservice runtime, or E2EE bridge logic.
# - Federation-facing HTTP hook points return safe placeholder responses until
#   verified handlers are implemented in lososd.
# - No ad hoc unsolicited WAN listeners are opened on the box; traffic is
#   expected through the edge proxy network/front door when deployed.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.losos.forum;
  live = config.lososInternal.forum;
  ap = cfg.activityPub;
  dm = cfg.matrixDmBridge;
  inherit (cfg) edge stateDir;
  statusFile = "${stateDir}/scaffold-status.json";
  contractsFile = "${stateDir}/federation-contracts.json";
  hasFront = config.losos.admin.enable && config.services.nginx.enable;
  lososdPort = config.losos.admin.apiPort;
  # lososd already serves /api/* behind nginx/identity; this is plumbing for a
  # future dedicated forum handler and for observability.
  forumApiUpstream = "http://127.0.0.1:${toString lososdPort}";

  # Safe JSON placeholder served by nginx for federation hook points until a
  # verified lososd implementation exists. Returning 501/placeholder avoids
  # pretending federation is implemented while keeping endpoint shape visible.
  scaffoldJson =
    name: extra:
    pkgs.writeText "losos-forum-${name}.json" (
      builtins.toJSON (
        {
          scaffold = true;
          implemented = false;
          service = "losos-forum";
          hook = name;
          inherit (cfg) edgeLocalRouting;
          design = "docs/superpowers/specs/2026-11-05-losos-forum-design.md";
        }
        // extra
      )
    );

  webfingerStub = scaffoldJson "webfinger" {
    note = "ActivityPub WebFinger hook point. Real actor discovery is not implemented.";
  };
  nodeinfoStub = scaffoldJson "nodeinfo" {
    note = "NodeInfo discovery hook point. Real node metadata is not implemented.";
    links = [ ];
  };
  apStub = scaffoldJson "activitypub" {
    note = "ActivityPub hook point. Actor/inbox/outbox HTTP signature handling is not implemented.";
  };
  dmStub = scaffoldJson "matrix-dm-bridge" {
    note = "Matrix DM bridge hook point. Appservice/sync/E2EE logic is not implemented.";
    inherit (dm) mode e2eePolicy;
    inherit (dm) requiresChat;
  };
in
{
  config = lib.mkMerge [
    (lib.mkIf cfg.enable {
      assertions = [
        {
          assertion = edge.requireProxy -> config.losos.proxy.enable;
          message = ''
            losos.forum.enable requires losos.proxy.enable when
            losos.forum.edge.requireProxy is true (edge proxy network /
            edge-local routing dependency). The full mesh-join gate
            (losos.cluster.enable / rke2 agent) is an open design decision; see
            losos.forum.edge.requireMesh and
            docs/superpowers/specs/2026-11-05-losos-forum-design.md.
          '';
        }
        {
          assertion = (!edge.requireMesh) || config.losos.cluster.enable;
          message = ''
            losos.forum.edge.requireMesh=true requires losos.cluster.enable.
            This gate is intentionally optional/open by design.
          '';
        }
        {
          assertion = config.losos.admin.enable && config.losos.admin.ui != null;
          message = "losos.forum.enable requires the admin UI (losos.admin.enable and losos.admin.ui).";
        }
        {
          assertion = (!dm.enable) || (!dm.requireChat) || config.losos.chat.enable;
          message = ''
            losos.forum.matrixDmBridge.enable requires losos.chat.enable when
            losos.forum.matrixDmBridge.requireChat is true, because Forum DMs
            must appear in LosOS Chat.
          '';
        }
        {
          assertion =
            cfg.assistant.provider == "disabled" || cfg.assistant.provider == "stub" || cfg.assistant.enable;
          message = "losos.forum.assistant.provider requires losos.forum.assistant.enable (except disabled/stub placeholders).";
        }
      ];

      systemd.tmpfiles.rules = [
        "d ${stateDir} 0750 root root -"
        "d ${stateDir}/federation 0750 root root -"
        "d ${stateDir}/bridge 0750 root root -"
      ];
    })

    (lib.mkIf live {
      # Placeholder status and contract artifacts for observability/debugging.
      # Replaced by a real forum service in later phases.
      systemd.services.losos-forum-scaffold = {
        description = "LosOS Forum scaffold status/contracts (placeholder; full service pending)";
        wantedBy = [ "multi-user.target" ];
        after = [ "network.target" ];
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
        };
        script = ''
          ${pkgs.coreutils}/bin/cat > ${statusFile} <<'JSON'
          ${builtins.toJSON {
            scaffold = true;
            implemented = false;
            phase = 0;
            inherit (cfg) edgeLocalRouting;
            edge = {
              inherit (edge)
                requireProxy
                requireMesh
                registrationMode
                pathFile
                ;
            };
            activityPub = {
              inherit (ap)
                enable
                publicFederation
                webfingerPath
                nodeinfoPath
                ;
              inherit (ap) basePath sharedInboxPath outboxHookPath;
            };
            matrixDmBridge = {
              inherit (dm) enable mode requireChat;
              inherit (dm) appserviceNamespacePrefix e2eePolicy dmRoomMapping;
            };
            assistant = {
              enable = cfg.assistant.enable;
              provider = cfg.assistant.provider;
            };
            design = "docs/superpowers/specs/2026-11-05-losos-forum-design.md";
          }}
          JSON
          ${pkgs.coreutils}/bin/cat > ${contractsFile} <<'JSON'
          ${builtins.toJSON {
            activityPub = {
              implemented = false;
              safety = "stub-contract-only; no signature verification, delivery, or inbox processing";
              endpoints = {
                webfinger = ap.webfingerPath;
                nodeinfo = ap.nodeinfoPath;
                actor = "${ap.basePath}/users/{id}";
                inbox = "${ap.basePath}/users/{id}/inbox";
                outbox = "${ap.basePath}/users/{id}/outbox";
                sharedInbox = ap.sharedInboxPath;
                followers = "${ap.basePath}/users/{id}/followers";
                following = "${ap.basePath}/users/{id}/following";
                object = "${ap.basePath}/objects/{id}";
              };
              notes = [
                "Federation traffic must use edge-local routing through the selected edge/proxy path."
                "Public Fediverse reachability remains an open decision (activityPub.publicFederation default false)."
              ];
            };
            matrixDmBridge = {
              implemented = false;
              safety = "stub-contract-only; no appservice runtime, room sync, or E2EE bridge logic";
              inherit (dm) mode e2eePolicy;
              requiresLososChat = dm.requireChat;
              mapping = {
                contract = dm.dmRoomMapping;
                forumDmId = "forum-dm:<conversation-id>";
                matrixRoom = "m.direct room for participating LosOS identities in Synapse/LosOS Chat";
                identityNamespacePrefix = dm.appserviceNamespacePrefix;
              };
              notes = [
                "DMs handled/federated by Forum must appear in LosOS Chat; no isolated DM store."
                "Bridge mechanism/E2EE are open design decisions; this file records hooks only."
              ];
            };
          }}
          JSON
          ${pkgs.coreutils}/bin/chmod 0640 ${statusFile} ${contractsFile}
        '';
      };

      # Runtime/service/env plumbing for lososd and future forum service.
      systemd.services.lososd.environment = {
        LOSOS_FORUM_SCAFFOLD = "1";
        LOSOS_FORUM_STATE_DIR = stateDir;
        LOSOS_FORUM_STATUS_FILE = statusFile;
        LOSOS_FORUM_CONTRACTS_FILE = contractsFile;
        LOSOS_FORUM_PORT = toString cfg.port;
        LOSOS_FORUM_API_PATH = cfg.apiPath;
        LOSOS_FORUM_UI_PATH = cfg.uiPath;
        LOSOS_FORUM_EDGE_LOCAL = if cfg.edgeLocalRouting then "1" else "0";
        LOSOS_FORUM_EDGE_REQUIRE_PROXY = if edge.requireProxy then "1" else "0";
        LOSOS_FORUM_EDGE_REQUIRE_MESH = if edge.requireMesh then "1" else "0";
        LOSOS_FORUM_EDGE_REGISTRATION_MODE = edge.registrationMode;
        LOSOS_FORUM_EDGE_PATH_FILE = if edge.pathFile == null then "" else edge.pathFile;
        LOSOS_FORUM_ACTIVITYPUB = if ap.enable then "1" else "0";
        LOSOS_FORUM_ACTIVITYPUB_PUBLIC = if ap.publicFederation then "1" else "0";
        LOSOS_FORUM_ACTIVITYPUB_BASE_PATH = ap.basePath;
        LOSOS_FORUM_ACTIVITYPUB_WEBFINGER_PATH = ap.webfingerPath;
        LOSOS_FORUM_ACTIVITYPUB_NODEINFO_PATH = ap.nodeinfoPath;
        LOSOS_FORUM_ACTIVITYPUB_SHARED_INBOX = ap.sharedInboxPath;
        LOSOS_FORUM_ACTIVITYPUB_OUTBOX_HOOK = ap.outboxHookPath;
        LOSOS_FORUM_MATRIX_DM_BRIDGE = if dm.enable then "1" else "0";
        LOSOS_FORUM_MATRIX_DM_BRIDGE_MODE = dm.mode;
        LOSOS_FORUM_MATRIX_DM_REQUIRE_CHAT = if dm.requireChat then "1" else "0";
        LOSOS_FORUM_MATRIX_DM_NAMESPACE_PREFIX = dm.appserviceNamespacePrefix;
        LOSOS_FORUM_MATRIX_DM_E2EE_POLICY = dm.e2eePolicy;
        LOSOS_FORUM_ASSISTANT = if cfg.assistant.enable then "1" else "0";
        LOSOS_FORUM_ASSISTANT_PROVIDER = cfg.assistant.provider;
      };

      # Edge proxy routing hook points (nginx/front door).
      # The box's existing tunnel/registrar registration carries traffic to
      # nginx; these locations expose safe scaffold endpoints and reserve path
      # shapes for future verified handlers. They avoid ad hoc WAN listeners.
      services.nginx.virtualHosts."losos-front".locations = lib.mkIf hasFront (
        lib.mkMerge [
          # Internal/admin API hook path. /api/* is normally proxied by the front
          # door/identity stack; this explicit location keeps the hook visible and
          # stable for a future dedicated forum handler.
          {
            "${cfg.apiPath}/" = {
              proxyPass = "${forumApiUpstream}/api/forum/";
              recommendedProxySettings = false;
              extraConfig = ''
                proxy_set_header Host $host;
                proxy_set_header X-Forwarded-For $remote_addr;
                proxy_set_header X-Forwarded-Proto $scheme;
                proxy_set_header X-Losos-Forum-Scaffold 1;
                proxy_set_header X-Losos-Edge-Local ${if cfg.edgeLocalRouting then "1" else "0"};
                proxy_read_timeout 120s;
              '';
            };
          }

          # ActivityPub hook points: SCAFFOLD placeholders. They do NOT accept or
          # verify signatures, do NOT enqueue deliveries, and do NOT create state.
          # Real handlers must be implemented in lososd with careful verification.
          (lib.mkIf ap.enable {
            "=${ap.webfingerPath}" = {
              alias = "${webfingerStub}";
              extraConfig = ''
                default_type application/jrd+json;
                add_header Access-Control-Allow-Origin "*" always;
                add_header X-Losos-Forum-Scaffold "1" always;
              '';
            };
            "=${ap.nodeinfoPath}" = {
              alias = "${nodeinfoStub}";
              extraConfig = ''
                default_type application/json;
                add_header Access-Control-Allow-Origin "*" always;
                add_header X-Losos-Forum-Scaffold "1" always;
              '';
            };
            "${ap.basePath}/" = {
              alias = "${apStub}";
              extraConfig = ''
                default_type application/activity+json;
                add_header Access-Control-Allow-Origin "*" always;
                add_header X-Losos-Forum-Scaffold "1" always;
              '';
            };
          })

          # Matrix DM bridge status contract hook (local/admin observability only;
          # not a Matrix federation endpoint). Real bridge traffic belongs in
          # lososd/chat integration paths, not a public scaffold endpoint.
          (lib.mkIf dm.enable {
            "=/api/forum/bridge/matrix-dm/contract" = {
              alias = "${dmStub}";
              extraConfig = ''
                default_type application/json;
                add_header Cache-Control "no-store" always;
                add_header X-Losos-Forum-Scaffold "1" always;
              '';
            };
          })
        ]
      );
    })
  ];
}
