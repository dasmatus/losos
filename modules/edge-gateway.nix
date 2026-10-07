# The edge gateway: a losos edge preset for a machine that sits on a LAN
# beside the boxes and needs no provisioning before it is useful
# (wiki/Edge-Federation.md, "Delivery").
#
# `losos.edge.gateway.enable` turns the edge module into the gateway: it
# advertises itself on the LAN, enrols any box that finds it trust-on-first-use
# (so a box installed from the stock ISO is published through it within a
# minute of booting), and relays those boxes to a hub — an official edge — as
# soon as its owner has entered the hub's tenant row with `losos-edge uplink
# set`. The uplink is read from a runtime file, which is what lets one disk
# image serve every site: the image carries no secret and no site name.
#
# This is NOT the appliance, and it does not pretend to be one. The appliance
# has no shell because nothing on it needs a person; a gateway needs one
# sentence typed into it (the uplink), so it has a root console with a known
# first password that the first login must change, and an sshd that is
# installed but stays stopped until `losos-edge ssh on`. Nothing else from the
# appliance's hardening is copied here: this machine holds no owner data, only
# routes and the tokens boxes handed it.
#
# `flake.nix` builds it two ways: `nixosConfigurations.edge-gateway` plus the
# QCOW2 in flake/disk-images.nix (`nix build .#losos-disk-edge-qcow2`, attached
# to every tagged release), and `nixosModules.edge` for an existing NixOS
# machine, which imports this file and sets the one option.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.losos.edge.gateway;
  edge = config.losos.edge;

  # Runtime state the gateway owns: the uplink the owner entered and the
  # marker that the first-login password change has been asked for.
  stateDir = "/var/lib/losos-edge";
  uplinkJson = "${stateDir}/uplink.json";
  uplinkToken = "${stateDir}/uplink.token";
  uplinkBootstrap = "${stateDir}/uplink-bootstrap.token";
  uplinkNoisePub = "${stateDir}/hub-noise.pub";
  uplinkToml = "/etc/rathole/uplink.toml";
  enrolDir = "/var/lib/losos-registrar/enrolled";
  registryJson = "/var/lib/losos-registrar/registry.json";

  # The owner's one command. Plain bash on purpose: it is read on a console
  # by somebody who has never seen this project, so every subcommand says what
  # it did and what to do next.
  cli = pkgs.writeShellApplication {
    name = "losos-edge";
    runtimeInputs = [
      pkgs.coreutils
      pkgs.jq
      pkgs.gawk
      pkgs.gnugrep
      pkgs.iproute2
      pkgs.systemd
      pkgs.hostname
      edge.registrar.package
    ];
    text = ''
      state=${stateDir}
      uplink=${uplinkJson}
      enrol_dir=${enrolDir}
      registry=${registryJson}

      usage() {
        cat <<'USAGE'
      losos-edge — the LosOS edge gateway on this machine

        losos-edge status                      what this gateway is doing
        losos-edge boxes                       the boxes it publishes
        losos-edge forget <id>                 drop a box that left (it re-enrols if it comes back)
        losos-edge uplink set --id <id> --token-file <file> \
                              --registrar <url> --rathole <host:port> \
                              [--bootstrap-token-file <file>]
                                               relay the boxes here through a hub edge,
                                               with the tenant row its operator gave you
        losos-edge uplink show                 the uplink as set
        losos-edge uplink clear                stop relaying
        losos-edge ssh on|off                  start or stop sshd (off at every boot)

      Boxes on this LAN find the gateway on their own (mDNS, _losos-edge._tcp)
      and enrol on first contact; nothing is provisioned for them.
      USAGE
      }

      active() { systemctl is-active "$1" 2>/dev/null || true; }

      addresses() {
        ip -4 -o addr show scope global 2>/dev/null \
          | awk '{ split($4, a, "/"); print a[1] }' \
          | grep -Ev '^(10\.0\.2\.|169\.254\.)' || true
      }

      case "''${1:-}" in
        status)
          echo "gateway:   $(hostname).local  $(addresses | tr '\n' ' ')"
          echo "advert:    ${edge.lan.url}  (tunnel ${edge.lan.ratholeEndpoint}, enrolment ${
            if edge.lan.openEnrolment then "open" else "closed"
          })"
          if [ -s "$uplink" ]; then
            echo "uplink:    $(jq -r '"to \(.registrar_url) as \(.id), tunnel \(.rathole_endpoint)"' "$uplink")"
            echo "           tunnel unit: $(active losos-rathole-uplink.service)"
            if [ -s ${uplinkToml} ]; then
              echo "           relaying: $(grep -c '^\[client.services\."' ${uplinkToml} || true) box(es)"
            else
              echo "           relaying: nothing yet (no box live, or the hub has not answered)"
            fi
          else
            echo "uplink:    not set — boxes are reachable on this LAN only."
            echo "           losos-edge uplink set --id <id> --token-file <file> --registrar <url> --rathole <host:port>"
          fi
          echo "boxes:     $("$0" boxes | grep -c . || true) registered  (losos-edge boxes)"
          echo "units:     registrar $(active losos-registrar.service), tunnel $(active losos-rathole.service), traefik $(active traefik.service)"
          echo "ssh:       $(active sshd.service)  (losos-edge ssh on|off)"
          ;;
        boxes)
          if [ -s "$registry" ]; then
            jq -r '.tenants // {} | to_entries[] | select(.value.via == null) | "\(.key)\t\(.value.hostname)\tport \(.value.port)"' "$registry" \
              | sort
          fi
          ;;
        forget)
          [ -n "''${2:-}" ] || { usage; exit 2; }
          losos-registrar enrol forget --dir "$enrol_dir" "$2"
          ;;
        uplink)
          case "''${2:-}" in
            set)
              shift 2
              id= token_file= registrar= rathole= bootstrap=
              while [ $# -gt 0 ]; do
                case "$1" in
                  --id) id="''${2:?}"; shift 2 ;;
                  --token-file) token_file="''${2:?}"; shift 2 ;;
                  --registrar) registrar="''${2:?}"; shift 2 ;;
                  --rathole) rathole="''${2:?}"; shift 2 ;;
                  --bootstrap-token-file) bootstrap="''${2:?}"; shift 2 ;;
                  *) echo "unknown flag $1" >&2; usage; exit 2 ;;
                esac
              done
              [ -n "$id" ] && [ -n "$token_file" ] && [ -n "$registrar" ] && [ -n "$rathole" ] || { usage; exit 2; }
              printf '%s' "$id" | grep -Eq '^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$' || {
                echo "--id must be a DNS label (the tenant id the hub operator gave you)" >&2; exit 2; }
              case "$registrar" in http://*|https://*) ;; *) echo "--registrar must be an http(s) URL" >&2; exit 2 ;; esac
              [ -s "$token_file" ] || { echo "$token_file is empty or missing" >&2; exit 2; }
              umask 077
              install -d -m 0700 "$state"
              # Copies, so the owner may delete the file they typed the token
              # into; the registrar reads these paths, never the originals.
              install -m 0600 "$token_file" ${uplinkToken}
              args=(--arg registrar "$registrar" --arg rathole "$rathole" --arg id "$id")
              if [ -n "$bootstrap" ]; then
                [ -s "$bootstrap" ] || { echo "$bootstrap is empty or missing" >&2; exit 2; }
                install -m 0600 "$bootstrap" ${uplinkBootstrap}
                args+=(--arg bootstrap ${uplinkBootstrap})
              else
                rm -f ${uplinkBootstrap}
                args+=(--arg bootstrap "")
              fi
              jq -n "''${args[@]}" '{
                registrar_url: $registrar, rathole_endpoint: $rathole, id: $id,
                token_file: "${uplinkToken}",
                noise_public_key_file: "${uplinkNoisePub}"
              } + (if $bootstrap == "" then {} else {bootstrap_token_file: $bootstrap} end)' > "$uplink.new"
              mv "$uplink.new" "$uplink"
              # A hub changed under a running tunnel: the registrar re-reads the
              # file on its next pass and re-renders uplink.toml, which rathole
              # reloads; a pinned key for the old hub must not be kept for the new.
              rm -f ${uplinkNoisePub}
              systemctl try-restart losos-rathole-uplink.service 2>/dev/null || true
              echo "uplink set: $id via $registrar. The boxes here are relayed on the next pass (about ${edge.uplink.interval})."
              echo "watch it: journalctl -fu losos-registrar.service"
              ;;
            show)
              if [ -s "$uplink" ]; then jq . "$uplink"; else echo "no uplink set"; fi
              ;;
            clear)
              rm -f "$uplink" ${uplinkToken} ${uplinkBootstrap} ${uplinkNoisePub}
              systemctl stop losos-rathole-uplink.service 2>/dev/null || true
              rm -f ${uplinkToml}
              echo "uplink cleared: the boxes here are reachable on this LAN only."
              ;;
            *) usage; exit 2 ;;
          esac
          ;;
        ssh)
          case "''${2:-}" in
            on)
              systemctl start sshd.service
              echo "sshd is running until the next boot: ssh root@$(addresses | head -1)"
              ;;
            off) systemctl stop sshd.service; echo "sshd stopped" ;;
            *) usage; exit 2 ;;
          esac
          ;;
        -h|--help|help|"") usage ;;
        *) usage; exit 2 ;;
      esac
    '';
  };
in
{
  options.losos.edge.gateway.enable = lib.mkOption {
    type = lib.types.bool;
    default = false;
    description = ''
      Make this machine a LosOS edge gateway (wiki/Edge-Federation.md): the
      edge module with LAN advertising and open enrolment on, the uplink
      read from ${uplinkJson} (written by `losos-edge uplink set`), a root
      console whose first password is `losos` and must be changed at first
      login, and sshd installed but stopped until `losos-edge ssh on`. The
      shape of the `losos-disk-edge-qcow2` image; import nixosModules.edge
      and set this on an existing NixOS machine for the same thing without
      the image.
    '';
  };

  config = lib.mkIf cfg.enable {
    losos.edge = {
      enable = true;
      lan.advertise = lib.mkDefault true;
      lan.openEnrolment = lib.mkDefault true;
      uplink.enable = lib.mkDefault true;
      uplink.configFile = lib.mkDefault uplinkJson;
      # Traefik's ACME needs a public name this machine does not have; the
      # failure is logged and the LAN routes work regardless, as in the LAN
      # demo. The address is a placeholder for an account that is never made.
      acmeEmail = lib.mkDefault "gateway@losos.cfd";
    };

    # ── Secrets the edge module expects to find ──────────────────────────
    # The rathole bootstrap token is a transport secret every box shares
    # with the edge; with open enrolment no box knows it in advance, and
    # every box service carries its own token, so its value is immaterial.
    # Generated once, before the seed and the registrar read it, so the image
    # boots with no provisioning step and no two gateways share a token.
    systemd.services.losos-edge-gateway-secrets = {
      description = "losos edge gateway — first-boot secrets";
      wantedBy = [ "multi-user.target" ];
      before = [
        "losos-rathole-seed.service"
        "losos-registrar.service"
      ];
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
      };
      path = [ pkgs.coreutils ];
      script = ''
        set -eu
        umask 077
        install -d -m 0700 /var/secrets ${stateDir}
        token=${toString edge.bootstrapTokenFile}
        if [ ! -s "$token" ]; then
          head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n' > "$token.new"
          mv "$token.new" "$token"
        fi
      '';
    };

    # ── The console ──────────────────────────────────────────────────────
    # One known first password, changed at first login (`chage -d 0`, asked
    # once: the marker survives reboots). mutableUsers (the default) is what
    # lets the change persist; nothing here declares a hash.
    users.users.root.initialPassword = "losos";
    systemd.services.losos-edge-gateway-first-login = {
      description = "losos edge gateway — expire the first root password";
      wantedBy = [ "multi-user.target" ];
      after = [ "losos-edge-gateway-secrets.service" ];
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
      };
      path = [
        pkgs.coreutils
        pkgs.shadow
      ];
      script = ''
        set -eu
        marker=${stateDir}/.first-password-expired
        [ -e "$marker" ] && exit 0
        chage -d 0 root
        touch "$marker"
      '';
    };

    # sshd is on the machine and off at every boot: `losos-edge ssh on` for
    # one session of remote work. The port is open to a closed socket in the
    # meantime, because the firewall cannot be toggled from a running system
    # and a refused connection is the same to an attacker as a dropped one.
    services.openssh = {
      enable = true;
      settings = {
        PermitRootLogin = "yes";
        PasswordAuthentication = true;
      };
    };
    systemd.services.sshd.wantedBy = lib.mkForce [ ];

    # What tty1 shows before anyone logs in: agetty fills `\4` with the
    # machine's IPv4 address and `\n` with its hostname when it prints
    # /etc/issue, so the banner is right without a service of its own.
    services.getty = {
      greetingLine = ''<<< LosOS edge gateway: \n.local (\4) >>>'';
      helpLine = ''
        Boxes on this network find this gateway on their own and are reachable
        through it on the LAN. To reach them from anywhere, log in as root
        (first password: losos) and run:  losos-edge uplink set ...
        Everything else:  losos-edge status
      '';
    };

    environment.systemPackages = [
      cli
      pkgs.jq
      pkgs.avahi
      pkgs.curl
    ];
    # Resolve the boxes' `.local` names from the gateway's own shell.
    services.avahi.nssmdns4 = true;
  };
}
