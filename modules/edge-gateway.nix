# The edge gateway: a losos edge preset for a machine that sits on a LAN
# beside the boxes and needs no provisioning before it is useful
# (handbook/docs/in-depth/edge-federation.md, "Delivery").
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
# QCOW2 in nix/disk-images.nix (`nix build .#losos-disk-edge-qcow2`, attached
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
  uplinkPin = "${stateDir}/hub-tunnel.pin";
  uplinkTunnel = "/etc/losos-tunnel/uplink.json";
  enrolDir = "/var/lib/losos-registrar/enrolled";
  registryJson = "/var/lib/losos-registrar/registry.json";
  mainJson = "/var/lib/losos-registrar/main-cluster.json";
  clusterJson = edge.secondary.file;
  # The registrar as this machine reaches it, whatever address it binds.
  localApi =
    let
      b = edge.registrarApiBind;
      host =
        if b == "0.0.0.0" || b == "::" || b == "[::]" then
          "127.0.0.1"
        else if lib.hasInfix ":" b && !lib.hasPrefix "[" b then
          "[${b}]"
        else
          b;
    in
    "http://${host}:${toString edge.registrarApiPort}";

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
      pkgs.curl
      pkgs.openssl
      edge.registrar.package
    ];
    text = ''
      state=${stateDir}
      uplink=${uplinkJson}
      enrol_dir=${enrolDir}
      registry=${registryJson}
      cluster=${clusterJson}
      api=${localApi}

      usage() {
        cat <<'USAGE'
      losos-edge — the LosOS edge gateway on this machine

        losos-edge status                      what this gateway is doing
        losos-edge boxes                       the boxes it publishes
        losos-edge forget <id>                 drop a box that left (it re-enrols if it comes back)
        losos-edge uplink set --id <id> --token-file <file> \
                              --registrar <url> --tunnel <host:port>
                                               relay the boxes here through a hub edge,
                                               with the tenant row its operator gave you
        losos-edge uplink show                 the uplink as set
        losos-edge uplink clear                stop relaying
        losos-edge cluster create <name>       start a secondary cluster with this gateway
        losos-edge cluster join <name> <secret> <url>
                                               join the cluster another gateway started
        losos-edge cluster show                the cluster and the members that answer
        losos-edge cluster leave               stand alone again
        losos-edge main                        the main cluster: gateways on primary links
        losos-edge ssh on|off                  start or stop sshd (off at every boot)
        losos-edge manage-key                  the key the manage page signs in with

      Boxes on this LAN find the gateway on their own (mDNS, _losos-edge._tcp)
      and enrol on first contact; nothing is provisioned for them.

      Every link has a tier: primary from this gateway to the hub (the main
      cluster), secondary from a box to a gateway of its cluster, tertiary
      from a box straight to an official edge.
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
          echo "advert:    ${edge.lan.url}  (tunnel ${edge.lan.tunnelEndpoint}, enrolment ${
            if edge.lan.openEnrolment then "open" else "closed"
          })"
          if [ -s "$uplink" ]; then
            echo "uplink:    $(jq -r '"to \(.registrar_url) as \(.id), tunnel \(.tunnel_endpoint // .rathole_endpoint)"' "$uplink")"
            echo "           tunnel unit: $(active losos-tunnel-uplink.service)"
            if [ -s ${uplinkTunnel} ]; then
              echo "           relaying: $(jq '.services | length' ${uplinkTunnel}) box(es)"
            else
              echo "           relaying: nothing yet (no box live, or the hub has not answered)"
            fi
          else
            echo "uplink:    not set — boxes are reachable on this LAN only."
            echo "           losos-edge uplink set --id <id> --token-file <file> --registrar <url> --tunnel <host:port>"
          fi
          if [ -s "$cluster" ]; then
            echo "cluster:   secondary cluster $(jq -r '.name' "$cluster") as $(jq -r '.member' "$cluster")  (losos-edge cluster show)"
          else
            echo "cluster:   none: boxes here hold secondary links to this gateway alone  (losos-edge cluster create <name>)"
          fi
          if [ -s "$uplink" ]; then
            echo "tiers:     primary to the hub; secondary from the boxes here"
          else
            echo "tiers:     secondary from the boxes here; no primary link"
          fi
          echo "boxes:     $("$0" boxes | grep -c . || true) registered  (losos-edge boxes)"
          echo "units:     registrar $(active losos-registrar.service), tunnel $(active losos-tunnel.service), traefik $(active traefik.service)"
          echo "ssh:       $(active sshd.service)  (losos-edge ssh on|off)"
          ;;
        boxes)
          if [ -s "$registry" ]; then
            jq -r '.tenants // {} | to_entries[] | select(.value.via == null) | "\(.key)\t\(.value.hostname)\tport \(.value.port)"' "$registry" \
              | sort
          fi
          ;;
        manage-key)
          key=/var/lib/losos-registrar/manage-key
          if [ -s "$key" ]; then
            cat "$key"
            echo "Sign in at ${edge.lan.url}/manage/ with it. A box shows its settings there once"
            echo "its owner lets this gateway manage it, on the box's Mesh page."
          else
            echo "No key yet: the registrar makes it when it starts ($(active losos-registrar.service))." >&2
            exit 1
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
              id="" token_file="" registrar="" tunnel=""
              while [ $# -gt 0 ]; do
                case "$1" in
                  --id) id="''${2:?}"; shift 2 ;;
                  --token-file) token_file="''${2:?}"; shift 2 ;;
                  --registrar) registrar="''${2:?}"; shift 2 ;;
                  --tunnel) tunnel="''${2:?}"; shift 2 ;;
                  *) echo "unknown flag $1" >&2; usage; exit 2 ;;
                esac
              done
              [ -n "$id" ] && [ -n "$token_file" ] && [ -n "$registrar" ] && [ -n "$tunnel" ] || { usage; exit 2; }
              printf '%s' "$id" | grep -Eq '^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$' || {
                echo "--id must be a DNS label (the tenant id the hub operator gave you)" >&2; exit 2; }
              case "$registrar" in http://*|https://*) ;; *) echo "--registrar must be an http(s) URL" >&2; exit 2 ;; esac
              [ -s "$token_file" ] || { echo "$token_file is empty or missing" >&2; exit 2; }
              umask 077
              install -d -m 0700 "$state"
              # Copies, so the owner may delete the file they typed the token
              # into; the registrar reads these paths, never the originals.
              install -m 0600 "$token_file" ${uplinkToken}
              jq -n --arg registrar "$registrar" --arg tunnel "$tunnel" --arg id "$id" '{
                registrar_url: $registrar, tunnel_endpoint: $tunnel, id: $id,
                token_file: "${uplinkToken}",
                pin_file: "${uplinkPin}"
              }' > "$uplink.new"
              mv "$uplink.new" "$uplink"
              # A hub changed under a running tunnel: the registrar re-reads the
              # file on its next pass and re-renders the tunnel's file, which
              # the client follows; a pin for the old hub must not be kept for
              # the new one.
              rm -f ${uplinkPin}
              systemctl try-restart losos-tunnel-uplink.service 2>/dev/null || true
              echo "uplink set: $id via $registrar. The boxes here are relayed on the next pass (about ${edge.uplink.interval})."
              echo "watch it: journalctl -fu losos-registrar.service"
              ;;
            show)
              if [ -s "$uplink" ]; then jq . "$uplink"; else echo "no uplink set"; fi
              ;;
            clear)
              rm -f "$uplink" ${uplinkToken} ${uplinkPin}
              # The rendered config goes first: losos-tunnel-uplink.path
              # starts the service again whenever the file exists and the
              # service is not running, so stopping before removing races it.
              rm -f ${uplinkTunnel}
              systemctl stop losos-tunnel-uplink.service 2>/dev/null || true
              echo "uplink cleared: the boxes here are reachable on this LAN only."
              ;;
            *) usage; exit 2 ;;
          esac
          ;;
        cluster)
          member=$(hostname | tr '[:upper:]' '[:lower:]')
          label='^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$'
          write_cluster() {
            umask 077
            install -d -m 0700 "$(dirname "$cluster")"
            printf '%s\n' "$1" > "$cluster.new"
            mv "$cluster.new" "$cluster"
          }
          case "''${2:-}" in
            create)
              name="''${3:-}"
              printf '%s' "$name" | grep -Eq "$label" || { echo "the cluster name must be a DNS label" >&2; exit 2; }
              [ -s "$cluster" ] && { echo "already in cluster $(jq -r .name "$cluster"); losos-edge cluster leave first" >&2; exit 1; }
              secret=$(openssl rand -hex 32)
              write_cluster "$(jq -n --arg n "$name" --arg m "$member" --arg s "$secret" \
                '{name: $n, member: $m, secret: $s, peers: []}')"
              echo "cluster $name started with this gateway as $member."
              echo "On each other gateway of this network, run:"
              echo "  losos-edge cluster join $name $secret ${edge.lan.url}"
              ;;
            join)
              name="''${3:-}" secret="''${4:-}" peer="''${5:-}"
              [ -n "$name" ] && [ -n "$secret" ] && [ -n "$peer" ] || { usage; exit 2; }
              printf '%s' "$name" | grep -Eq "$label" || { echo "the cluster name must be a DNS label" >&2; exit 2; }
              printf '%s' "$secret" | grep -Eq '^[0-9a-fA-F]{32,}$' || { echo "the secret is the hex line cluster create printed" >&2; exit 2; }
              case "$peer" in http://*|https://*) ;; *) echo "the url must be the other gateway's http(s) address" >&2; exit 2 ;; esac
              write_cluster "$(jq -n --arg n "$name" --arg m "$member" --arg s "$secret" --arg p "$peer" \
                '{name: $n, member: $m, secret: $s, peers: [$p]}')"
              echo "joined cluster $name as $member through $peer. The gateways sync on the next pass (about ${edge.secondary.interval})."
              ;;
            show)
              if [ ! -s "$cluster" ]; then
                echo "in no secondary cluster: boxes here hold secondary links to this gateway alone."
                exit 0
              fi
              echo "cluster:   $(jq -r .name "$cluster")  (this gateway: $(jq -r .member "$cluster"))"
              if info=$(curl -fsS --max-time 5 "$api/cluster/secondary"); then
                echo "$info" | jq -r '.gateways[] | "member:    \(.member)  \(.url)  tunnel \(.tunnel)"'
              else
                echo "the registrar does not answer ($(active losos-registrar.service))" >&2
                exit 1
              fi
              ;;
            leave)
              rm -f "$cluster"
              echo "left the cluster: the boxes here stay on this gateway, the others no longer fail over to it."
              ;;
            *) usage; exit 2 ;;
          esac
          ;;
        main)
          if [ -s ${mainJson} ]; then
            jq -r '.[] | "\(.site)\t\(if .gateway == "" then "-" else .gateway end)\t\(.cluster // "-")\t\(.boxes) box(es)\tprimary"' ${mainJson}
          else
            echo "no main cluster yet: set an uplink (losos-edge uplink set) to join one."
          fi
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
      Make this machine a LosOS edge gateway
      (handbook/docs/in-depth/edge-federation.md): the edge module with LAN
      advertising and open enrolment on, the uplink read from ${uplinkJson}
      (written by `losos-edge uplink set`), a root console whose first password
      is `losos` and must be changed at first login, and sshd installed but
      stopped until `losos-edge ssh on`. The shape of the
      `losos-disk-edge-qcow2` image; import nixosModules.edge and set this on an
      existing NixOS machine for the same thing without the image.
    '';
  };

  config = lib.mkIf cfg.enable {
    losos.edge = {
      enable = true;
      lan.advertise = lib.mkDefault true;
      lan.openEnrolment = lib.mkDefault true;
      manage.enable = lib.mkDefault true;
      uplink.enable = lib.mkDefault true;
      uplink.configFile = lib.mkDefault uplinkJson;
      secondary.enable = lib.mkDefault true;
      # Traefik's ACME needs a public name this machine does not have; the
      # failure is logged and the LAN routes work regardless, as in the LAN
      # demo. The address is a placeholder for an account that is never made.
      acmeEmail = lib.mkDefault "gateway@losos.cfd";
    };

    # ── The gateway's own state ──────────────────────────────────────────
    # Where `losos-edge uplink set` keeps the uplink, its token and the hub's
    # pin. Nothing in it is made in advance: the tunnel's certificate is the
    # tunnel server's own, and the hub's pin is fetched on first contact.
    systemd.tmpfiles.rules = [ "d ${stateDir} 0700 root root - -" ];

    # ── The console ──────────────────────────────────────────────────────
    # One known first password, changed at first login (`chage -d 0`, asked
    # once: the marker survives reboots). mutableUsers (the default) is what
    # lets the change persist; nothing here declares a hash.
    users.users.root.initialPassword = "losos";
    systemd.services.losos-edge-gateway-first-login = {
      description = "losos edge gateway — expire the first root password";
      wantedBy = [ "multi-user.target" ];
      after = [ "systemd-tmpfiles-setup.service" ];
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
    # Only errors reach the console: at the default level the kernel's
    # informational messages (VLAN filters, regulatory database) land on top
    # of the banner and the login prompt, and the gateway's console is the
    # one screen its operator reads.
    boot.consoleLogLevel = lib.mkDefault 3;
    services.getty = {
      greetingLine = ''<<< LosOS edge gateway: \n.local (\4) >>>'';
      helpLine = ''
        Boxes on this network find this gateway on their own and are reachable
        through it on the LAN. To reach them from anywhere, log in as root
        (first password: losos) and run:  losos-edge uplink set ...
        Everything else:  losos-edge status
      '';
    };

    # The registrar binary is on the shell's PATH too (not only inside the
    # CLI): lan.openEnrolment's documentation names `losos-registrar enrol`.
    environment.systemPackages = [
      cli
      edge.registrar.package
      pkgs.jq
      pkgs.curl
    ];
    # Resolve the boxes' `.local` names from the gateway's own shell, even
    # with lan.advertise turned off (modules/mdns.nix); `resolvectl query
    # mattbox.local` and `resolvectl service` are the lookups by hand.
    losos.mdns.enable = true;
  };
}
