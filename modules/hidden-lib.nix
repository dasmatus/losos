# Tor and I2P for a box (modules/hidden-services.nix) and for an edge
# (modules/edge-hidden.nix): the part the two share.
#
# Both run the same two daemons the same way. What differs is the list of
# services: a box publishes one, its own front door, and an edge one per box
# it routes, which changes while it runs. So neither writes Tor's or I2P's
# configuration from Nix. Each writes a list,
#
#   [ { "id": "front", "http": 8480, "https": 8443 }, ... ]
#
# and `losos-hidden-sync` turns it into a torrc fragment and an i2pd tunnels
# file under /var/lib/losos-hidden, reloads the daemon whose file changed,
# and collects the addresses the daemons made into
# /var/lib/losos-hidden/addresses.json:
#
#   { "front": { "tor": "<56>.onion", "i2p": "<52>.b32.i2p" } }
#
# Each service gets port 80 and port 443 on its address, delivered to
# 127.0.0.1:<http> and 127.0.0.1:<https>. The keys stay when a service leaves
# the list (/var/lib/tor/onion/losos-<id>, /var/lib/i2pd/losos-<id>.dat), so
# a box that comes back gets the address it had. /var is persisted whole
# (modules/impermanence.nix), so the addresses outlive a reboot too.
#
# Neither daemon opens a port: Tor and I2P both reach their networks
# outbound, and I2P runs firewalled, through introducers. Neither offers a
# proxy to the box: Tor's SOCKS port is off (the NixOS module's default
# without services.tor.client), and i2pd's web console, HTTP and SOCKS
# proxies are off below, since anything on the box (a hostNetwork pod
# included) could otherwise use them.
#
# Tor reads the fragment through `%include` of a directory, so a VM test can
# drop a file of its own beside it (nix/tests/hidden-services.nix points Tor
# at a private network that way). Tor runs in its own RootDirectory, which
# sees only what is bound into it, hence the BindReadOnlyPaths.
#
# How an I2P address is made from the keys file: the file starts with the
# destination, 387 bytes plus the certificate's payload, whose length is the
# big-endian 16-bit number at bytes 385 and 386; the address is the SHA-256
# of the destination in lowercase base32 without padding, plus `.b32.i2p`.
{ lib, pkgs }:

let
  state = "/var/lib/losos-hidden";
  torDir = "${state}/tor";
  i2pdDir = "${state}/i2pd";
in
rec {
  inherit state;
  addressesFile = "${state}/addresses.json";
  # The addresses an edge made for this box (edge/registrar/src/announce.rs
  # writes them from the heartbeat's answer).
  edgeFile = "${state}/edge.json";

  # The box's front door for what Tor and I2P deliver, on loopback only and
  # on the front vhost only (modules/containers.nix, modules/tls.nix): plain
  # http, and the same after nginx's own TLS (`streamPort`, below), which
  # LosOS Login believes is https (modules/identity.nix, `$losos_proto`).
  # Never port 80: the edge's manage server (modules/proxy.nix) listens
  # there for loopback, and a Tor client chooses its own Host header.
  plainPort = 8480;
  tlsPort = 8481;
  streamPort = 8443;
  # The edge's Traefik entry points for the same (modules/edge-hidden.nix).
  edgePlainPort = 8480;
  edgeTlsPort = 8444;

  # The front vhost's extra listeners, for whichever of the two is on.
  frontListen =
    on:
    lib.optionals on [
      {
        addr = "127.0.0.1";
        port = plainPort;
      }
      {
        addr = "127.0.0.1";
        port = tlsPort;
      }
    ];

  # `hops`: tunnel length for I2P, null for i2pd's default (3). A private
  # test network has too few routers for three.
  sync =
    {
      tor,
      i2p,
      hops ? null,
    }:
    pkgs.writeShellApplication {
      name = "losos-hidden-sync";
      runtimeInputs = [
        pkgs.coreutils
        pkgs.diffutils
        pkgs.gawk
        pkgs.gnugrep
        pkgs.jq
        pkgs.systemd
      ];
      text = ''
        services="$1"
        install -d -m 0755 ${state} ${torDir} ${i2pdDir}

        # The list, with anything that is not a plain name and two ports
        # dropped: every id lands in a file name and a config line.
        list=$(jq -c '[.[]? | select(
            (.id | type) == "string"
            and (.id | test("^[a-z0-9]([a-z0-9.-]{0,62}[a-z0-9])?$"))
            and ([.http, .https] | all(type == "number" and . == floor and . > 0 and . < 65536))
          )] | unique_by(.id)' "$services" 2>/dev/null || echo '[]')

        # Write $2 to $1 when it differs; succeed only when it changed.
        put() {
          local tmp
          tmp=$(mktemp "$1.XXXXXX")
          printf '%s' "$2" > "$tmp"
          chmod 0644 "$tmp"
          if cmp -s "$tmp" "$1"; then
            rm -f "$tmp"
            return 1
          fi
          mv "$tmp" "$1"
        }

        reload() {
          if systemctl -q is-active "$1"; then
            systemctl reload "$1" || true
          fi
        }

        ${lib.optionalString tor ''
          torconf=$(jq -r '.[] |
            "HiddenServiceDir /var/lib/tor/onion/losos-\(.id)\n"
            + "HiddenServicePort 80 127.0.0.1:\(.http)\n"
            + "HiddenServicePort 443 127.0.0.1:\(.https)"' <<<"$list")
          if put ${torDir}/services.conf "$(printf '# Written by losos-hidden-sync.\n%s\n' "$torconf")"; then
            reload tor.service
          fi
        ''}
        ${lib.optionalString i2p ''
          tunnel() {
            printf '[losos-%s-%s]\ntype = server\nhost = 127.0.0.1\nport = %s\ninport = %s\nkeys = losos-%s.dat\n' \
              "$1" "$2" "$3" "$4" "$1"
            ${lib.optionalString (hops != null) ''
              printf 'inbound.length = ${toString hops}\noutbound.length = ${toString hops}\n'
            ''}
          }
          i2pconf="# Written by losos-hidden-sync."
          while read -r id http https; do
            i2pconf+=$'\n\n'"$(tunnel "$id" http "$http" 80)"
            i2pconf+=$'\n\n'"$(tunnel "$id" https "$https" 443)"
          done < <(jq -r '.[] | "\(.id) \(.http) \(.https)"' <<<"$list")
          if put ${i2pdDir}/services.conf "$i2pconf"; then
            reload i2pd.service
          fi
        ''}

        b32() {
          local len hex
          len=$(od -An -tu1 -j385 -N2 "$1" | awk 'NF == 2 { print $1 * 256 + $2 }')
          [ -n "$len" ] || return 1
          hex=$(head -c $((387 + len)) "$1" | sha256sum | cut -c1-64)
          printf '%s' "$hex" | tr a-f A-F | basenc --base16 -d | basenc --base32 -w0 | tr -d '=' | tr A-Z a-z
        }

        out='{}'
        while read -r id; do
          [ -n "$id" ] || continue
          onion=""
          i2p=""
          ${lib.optionalString tor ''
            f=/var/lib/tor/onion/losos-$id/hostname
            if [ -s "$f" ]; then
              onion=$(head -c 128 "$f" | tr -d '\n')
              printf '%s' "$onion" | grep -Eqx '[a-z2-7]{56}\.onion' || onion=""
            fi
          ''}
          ${lib.optionalString i2p ''
            for k in /var/lib/i2pd/losos-$id.dat /var/lib/private/i2pd/losos-$id.dat; do
              if [ -s "$k" ]; then
                i2p="$(b32 "$k" || true).b32.i2p"
                printf '%s' "$i2p" | grep -Eqx '[a-z2-7]{52}\.b32\.i2p' || i2p=""
                break
              fi
            done
          ''}
          out=$(jq -c --arg id "$id" --arg tor "$onion" --arg i2p "$i2p" \
            '. + {($id): {tor: (if $tor == "" then null else $tor end), i2p: (if $i2p == "" then null else $i2p end)}}' <<<"$out")
        done < <(jq -r '.[].id' <<<"$list")
        if put ${state}/addresses.json "$(jq -S . <<<"$out")"; then
          echo "addresses: $out"
        fi
      '';
    };

  # The NixOS configuration both sides share. `servicesFile` is the list
  # above; `after` the units that write it.
  config =
    {
      tor,
      i2p,
      servicesFile,
      hops ? null,
    }:
    let
      syncPkg = sync { inherit tor i2p hops; };
      daemons = lib.optional tor "tor.service" ++ lib.optional i2p "i2pd.service";
    in
    lib.mkMerge [
      {
        systemd.tmpfiles.rules = [
          "d ${state} 0755 root root - -"
          "d ${torDir} 0755 root root - -"
          "d ${i2pdDir} 0755 root root - -"
          # Never an empty directory for Tor's `%include`.
          "f ${torDir}/services.conf 0644 root root - -"
          "f ${i2pdDir}/services.conf 0644 root root - -"
        ];

        # Once before the daemons start, so they come up with the services
        # already in their files, then whenever the list changes and every
        # half minute: an address appears a moment after its daemon reads
        # the file, and this is what notices.
        systemd.services.losos-hidden-sync = {
          description = "Tor and I2P addresses for this machine's services";
          wantedBy = [ "multi-user.target" ];
          before = daemons;
          after = [ "systemd-tmpfiles-setup.service" ];
          serviceConfig = {
            Type = "oneshot";
            ExecStart = "${syncPkg}/bin/losos-hidden-sync ${servicesFile}";
          };
        };
        systemd.timers.losos-hidden-sync = {
          wantedBy = [ "timers.target" ];
          timerConfig = {
            OnActiveSec = "20s";
            OnUnitInactiveSec = "30s";
          };
        };
        systemd.paths.losos-hidden-sync = {
          wantedBy = [ "paths.target" ];
          pathConfig.PathChanged = servicesFile;
        };
      }
      (lib.mkIf tor {
        services.tor = {
          enable = true;
          settings."%include" = "${torDir}/";
        };
        systemd.services.tor = {
          after = [ "losos-hidden-sync.service" ];
          serviceConfig.BindReadOnlyPaths = [ torDir ];
        };
      })
      (lib.mkIf i2p {
        services.i2pd = {
          enable = true;
          settings = {
            tunnelsdir = i2pdDir;
            # 32 KB/s for other people's tunnels: a box on a home line is
            # not a relay, but taking part keeps it a good I2P citizen.
            bandwidth = "L";
            http.enabled = false;
            httpproxy.enabled = false;
            socksproxy.enabled = false;
          };
        };
        systemd.services.i2pd = {
          after = [ "losos-hidden-sync.service" ];
          # i2pd re-reads its tunnels on SIGHUP: new ones start, gone ones
          # stop, the rest carry on.
          serviceConfig.ExecReload = "${pkgs.coreutils}/bin/kill -HUP $MAINPID";
        };
      })
    ];
}
