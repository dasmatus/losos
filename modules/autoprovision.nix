# autoprovision, box side (backend/src/autoprovision.rs):
#
# with `losos.autoprovision.enable`, lososd's configuration reconciler
# (backend/src/config_repo.rs) also keeps `/etc/nixos` in step with the mesh's
# configuration mirror: every mesh box runs the same `modules/overrides.nix`,
# and a netbooted machine installs with whatever the mirror holds
# (`losos-ctl provision`).
#
# Nothing else changes on the box: the mirror is plain git, takes no secret,
# and every take from it goes through the same gates an Apply does
# (`backend/src/losos.rs` `gate_overrides`) before anything is written.
#
# The installer side lives in the same module set's `modules/installer.nix`
# (the environment its `losos-autoprovision` wrapper carries) and in the
# netboot flavor of the medium (`nixosConfigurations.netboot`, flake.nix).
{
  lib,
  config,
  ...
}:

let
  cfg = config.losos.autoprovision;
  daemonOn = config.losos.backend.package != null;
  on = daemonOn && cfg.enable && cfg.mirror != null;
  slugOk = builtins.match "[a-z0-9][a-z0-9-]{0,31}" cfg.box != null;
in
{
  assertions = [
    {
      assertion = cfg.enable -> cfg.mirror != null;
      message =
        "losos.autoprovision.enable needs the mesh's mirror URL in" + " losos.autoprovision.mirror";
    }
    {
      assertion = !cfg.enable || slugOk;
      message =
        "losos.autoprovision.box must be lowercase letters, digits and" + " dashes, at most 32 characters";
    }
  ];

  config = lib.mkIf on {
    # The two fields `backend/src/autoprovision.rs` reads.
    systemd.services.lososd.environment = {
      LOSOS_MESH_CONFIG_URL = cfg.mirror;
      LOSOS_MESH_CONFIG_BOX = cfg.box;
    };

    # The mirror a provision of this mesh runs against, for an operator
    # standing at a netbooted box (`losos-autoprovision` names it so no typed
    # address is needed, and `--mirror` outranks it anyway).
    environment.variables.LOSOS_MESH_CONFIG_URL = cfg.mirror;
  };
}
