# The option document and the configuration repository.
#
# Two things the admin UI's Settings → Advanced and History panes stand on:
#
#   * /etc/losos/options.json — every `losos.*` option this box declares,
#     with its type, default, description and the value the last rebuild
#     merged, generated from the declarations by nix/options-doc.nix. lososd
#     serves it at GET /api/options with the current overrides.nix joined in,
#     and checks every Apply against it (backend/src/options.rs), so a key the
#     pane could not draw is a key the daemon refuses.
#
#   * the configuration repository — /etc/nixos is a git repository (the
#     installer makes it one), lososd commits every change there, and with
#     `losos.configRepo.enable` it keeps that repository in step with a private
#     one on LosOS Git, owned by the owner's account
#     (backend/src/config_repo.rs). The daemon needs to know where LosOS Git
#     listens and where it left its admin token (modules/git.nix); that is
#     the environment below.
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
  # The repository needs LosOS Git; without it the daemon commits locally
  # and the History pane says so.
  publish = daemonOn && cfg.enable && config.losos.git.enable;

  doc = import ../nix/options-doc.nix { inherit lib options config; };
in
{
  config = lib.mkIf daemonOn {
    # Evaluated at build time, against this box's own configuration — so
    # `current` is what this generation merged, and a declaration with a type
    # nix/options-doc.nix cannot classify fails the build here rather than
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
        // lib.optionalAttrs cfg.upstream.enable {
          LOSOS_UPSTREAM_URL = cfg.upstream.url;
          LOSOS_UPSTREAM_BRANCH = cfg.upstream.branch;
        }
        // lib.optionalAttrs publish {
          LOSOS_CONFIG_REPO = "${cfg.owner}/${cfg.name}";
          # modules/git.nix: its loopback listener and the token it writes.
          LOSOS_GIT_URL = "http://127.0.0.1:3000";
          LOSOS_GIT_TOKEN_FILE = "/var/lib/losos-git/admin-token";
        };
      }

      # Following the published tree: right before the nightly build, ask
      # lososd to move /etc/nixos to the newest published commit, keeping the
      # box's own files (`losos-ctl config --update`, backend/src/losos.rs
      # cmd_config_update). Through lososd rather than a git command here,
      # because lososd is the one writer of /etc/nixos and holds its lock
      # against a concurrent Apply. The `-` keeps a failure from stopping the
      # update: a box with no internet builds the tree it has, as before.
      # Gated on autoUpgrade, not on `ExecStartPre` alone: a definition under
      # `systemd.services.nixos-upgrade` that evaluates to nothing still
      # creates the entry, and NixOS then writes an empty unit.
      (lib.mkIf (cfg.upstream.enable && config.system.autoUpgrade.enable) {
        nixos-upgrade = {
          wants = [ "lososd.service" ];
          after = [ "lososd.service" ];
          serviceConfig.ExecStartPre = [
            "-${config.losos.backend.package}/bin/losos-ctl config --update"
          ];
        };
      })
    ];
  };
}
