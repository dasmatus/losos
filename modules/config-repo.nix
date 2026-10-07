# The option document and the configuration repository.
#
# Two things the admin UI's Settings → Advanced and History panes stand on:
#
#   * /etc/losos/options.json — every `losos.*` option this box declares,
#     with its type, default, description and the value the last rebuild
#     merged, generated from the declarations by flake/options-doc.nix. lososd
#     serves it at GET /api/options with the current overrides.nix joined in,
#     and checks every Apply against it (backend/src/options.rs), so a key the
#     pane could not draw is a key the daemon refuses.
#
#   * the configuration repository — /etc/nixos is a git repository (the
#     installer makes it one), lososd commits every change there, and with
#     `losos.configRepo.enable` it keeps that repository in step with a private
#     one on the box's own Forgejo, owned by the owner's account
#     (backend/src/config_repo.rs). The daemon needs to know where Forgejo
#     listens and where the bootstrap (flake/forgejo-bootstrap.nix) left the
#     bot account's token; that is the environment below.
#
# Nothing here is the installer's business: the ISO imports neither this
# module nor daemon.nix.
{
  lib,
  config,
  options,
  pkgs,
  ...
}:

let
  cfg = config.losos.configRepo;
  daemonOn = config.losos.backend.package != null;
  # The repository needs Forgejo; without it the daemon commits locally and
  # the History pane says so.
  publish = daemonOn && cfg.enable && config.losos.forgejo.enable;

  # Forgejo's loopback listener. Both ports are literals where they are set
  # (modules/workloads.nix for the pod, modules/services.nix for the native
  # service) and nothing off-box ever sees either, so they are repeated here
  # rather than promoted to options.
  forgejoPort = if config.losos.forgejo.mode == "container" then 3000 else 8888;

  # Same path in both modes: the pod's hostPath and services.forgejo.stateDir
  # are both /var/lib/forgejo, persisted by impermanence's whole-/var mount.
  tokenFile = "/var/lib/forgejo/.losos-token";

  doc = import ../flake/options-doc.nix { inherit lib options config; };
in
{
  config = lib.mkIf daemonOn {
    # Evaluated at build time, against this box's own configuration — so
    # `current` is what this generation merged, and a declaration with a type
    # flake/options-doc.nix cannot classify fails the build here rather than
    # the pane at run time.
    environment.etc."losos/options.json".source = pkgs.writeText "losos-options.json" (
      builtins.toJSON doc
    );

    systemd.services = lib.mkMerge [
      {
        lososd.environment = {
          LOSOS_OPTIONS_FILE = "/etc/losos/options.json";
          LOSOS_CONFIG_DIR = "/etc/nixos";
        }
        // lib.optionalAttrs publish {
          LOSOS_CONFIG_REPO = "${cfg.owner}/${cfg.name}";
          LOSOS_FORGEJO_URL = "http://127.0.0.1:${toString forgejoPort}";
          LOSOS_FORGEJO_TOKEN_FILE = tokenFile;
        };
      }

      # The bot account for the native service. The workload image carries
      # the same snippet in its entrypoint (flake/images.nix). The mkIf sits
      # on the whole attrset, not on `preStart`: a definition under
      # `systemd.services.forgejo` that evaluates to nothing still creates the
      # `forgejo` entry, and NixOS then writes an empty forgejo.service on a
      # container-mode box that has no such service.
      (lib.mkIf (config.losos.forgejo.mode == "native" && config.losos.forgejo.enable) {
        forgejo.preStart = lib.mkAfter (
          import ../flake/forgejo-bootstrap.nix {
            forgejo = lib.getExe config.services.forgejo.package;
            tokenFile = "${config.services.forgejo.stateDir}/.losos-token";
          }
        );
      })
    ];
  };
}
