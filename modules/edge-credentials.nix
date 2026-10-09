# Secrets sealed on the edge from GitHub Actions.
#
# `.github/workflows/edge-credentials.yml` reads the Stripe and Claude keys
# from the repository's Actions secrets and pipes each one over SSH into
# `losos-seal-credential` here, which checks its shape and seals it with
# `systemd-creds` (TPM2 where the edge has one, the host key otherwise). The
# plaintext travels in the SSH session and in this script's memory only: it is
# never written to disk on either side, and the blob is all `/var` ever holds.
#
# The deploy key is root's only because sealing with the host key and
# restarting the gate need root. `restrict,command=` pins it to this one
# script: no shell, no forwarding, no pty, and the only thing the workflow can
# choose is which entry of `losos.edge.credentials.secrets` to replace.
#
# The Stripe key's mode (test or live) is read off the key by the gate itself
# (backend-registrar/src/stripe_mode.rs); this script only reports it.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.losos.edge;
  creds = cfg.credentials.secrets;

  # One `case` arm per credential: where it goes, the shape every line must
  # have, and the units to restart.
  arm = name: c: ''
    ${lib.escapeShellArg name})
      sealed=${lib.escapeShellArg (toString c.sealed)}
      pattern=${lib.escapeShellArg c.pattern}
      units=(${lib.escapeShellArgs c.units})
      ;;
  '';

  seal = pkgs.writeShellApplication {
    name = "losos-seal-credential";
    runtimeInputs = [
      pkgs.coreutils
      config.systemd.package
    ];
    text = ''
      # The name comes from the SSH command line under the forced command, or
      # from the first argument when an operator runs this by hand.
      name=''${SSH_ORIGINAL_COMMAND:-''${1:-}}
      case "$name" in
      ${lib.concatStrings (lib.mapAttrsToList arm creds)}
      *)
        echo "losos-seal-credential: unknown credential '$name'; known: ${lib.concatStringsSep " " (lib.attrNames creds)}" >&2
        exit 2
        ;;
      esac

      # 64 KiB is far more than any key; a larger stream is a mistake.
      secret=$(head -c 65537)
      if [ "''${#secret}" -gt 65536 ]; then
        echo "losos-seal-credential: $name is larger than 64 KiB; refused" >&2
        exit 1
      fi
      lines=0
      while IFS= read -r line; do
        [ -z "$line" ] && continue
        if ! [[ "$line" =~ ^($pattern)$ ]]; then
          echo "losos-seal-credential: $name is not shaped like one; nothing was sealed" >&2
          exit 1
        fi
        lines=$((lines + 1))
      done <<<"$secret"
      if [ "$lines" -eq 0 ]; then
        echo "losos-seal-credential: $name is empty; nothing was sealed" >&2
        exit 1
      fi

      case "$secret" in
      sk_test_* | rk_test_*) echo "$name: a test key (no real money moves)" ;;
      sk_live_* | rk_live_*) echo "$name: a live key (real payments)" ;;
      esac

      dir=$(dirname "$sealed")
      install -d -m 0700 "$dir"
      tmp=$(mktemp "$sealed.XXXXXX")
      trap 'rm -f "$tmp"' EXIT
      printf '%s' "$secret" | systemd-creds encrypt --name="$name" - "$tmp"
      chmod 0600 "$tmp"
      mv -f "$tmp" "$sealed"
      trap - EXIT
      echo "$name: sealed to $sealed"

      for unit in "''${units[@]}"; do
        if systemctl cat "$unit" >/dev/null 2>&1; then
          systemctl restart "$unit"
          echo "$name: restarted $unit"
        fi
      done
    '';
  };
in
{
  config = lib.mkIf cfg.enable (
    lib.mkMerge [
      {
        losos.edge.credentials.secrets = {
          stripe-secret-key = {
            sealed = lib.mkDefault cfg.market.stripeSecretKeySealed;
            # A secret or restricted key whose mode can be read off it. The
            # gate refuses any other, so sealing one would only stop it.
            pattern = lib.mkDefault "(sk|rk)_(test|live)_[A-Za-z0-9]+";
            units = [ "losos-stripe-gate.service" ];
          };
          stripe-webhook-secret = {
            sealed = lib.mkDefault cfg.market.webhookSecretSealed;
            pattern = lib.mkDefault "whsec_[A-Za-z0-9]+";
            units = [ "losos-stripe-gate.service" ];
          };
          claude-api-key = {
            sealed = lib.mkDefault "/var/secrets/losos-claude-api-key.cred";
            pattern = lib.mkDefault "sk-ant-[A-Za-z0-9_-]+";
          };
        };

        environment.systemPackages = [ seal ];
      }
      (lib.mkIf (cfg.credentials.deployKey != null) {
        services.openssh.enable = true;
        users.users.root.openssh.authorizedKeys.keys = [
          ''restrict,command="${seal}/bin/losos-seal-credential" ${cfg.credentials.deployKey}''
        ];
      })
    ]
  );
}
