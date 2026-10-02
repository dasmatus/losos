# The box's physical console: a big banner on tty1 saying where to point a
# browser.
#
# This appliance has no SSH and no shell logins, and the only way to use it is
# a web UI at an address the owner has no other way to learn. A login prompt
# on tty1 would be a dead end (neither user has a password), so tty1 shows the
# address instead — the box's current LAN IPv4 address, and the `<hostName>.local`
# name Avahi publishes — in a full-screen banner that redraws as the address
# changes (DHCP arrives some seconds after boot, and a lease can move).
#
# It replaces getty on tty1 only. tty2..tty6 keep their login prompts, which
# are as useless as before but also as harmless.
#
# The URL is plain http:// on purpose: the self-signed certificate
# modules/tls.nix makes is issued for `<hostName>.local`, so https://<ip> would
# put a name-mismatch warning in front of the very first page, and :80 stays
# open for exactly this (see the tail of that module's header).
{
  pkgs,
  lib,
  config,
  ...
}:

let
  fqdn = "${config.losos.hostName}.local";

  banner = pkgs.writeShellApplication {
    name = "losos-console-banner";
    runtimeInputs = [
      pkgs.coreutils
      pkgs.gawk
      pkgs.gnugrep
      pkgs.iproute2
    ];
    text = ''
      fqdn=${lib.escapeShellArg fqdn}

      # Global IPv4 addresses on real interfaces. The local Kubernetes cluster
      # and any container runtime add bridges and veths whose addresses mean
      # nothing to a person on the LAN.
      addresses() {
        ip -4 -o addr show scope global 2>/dev/null \
          | awk '{ split($4, a, "/"); print $2, a[1] }' \
          | grep -Ev '^(veth|cni|flannel|cali|docker|br-|virbr|vxlan|kube|lxc|tun|wg)' \
          | awk '{ print $2 }' \
          || true
      }

      # One banner row: the text centred in `width` columns, white on blue.
      row() {
        local text=$1 width=$2 style=''${3:-}
        local pad=$(( (width - ''${#text}) / 2 ))
        (( pad < 0 )) && pad=0
        printf '\033[44;97m%s%*s%s%*s\033[0m\n' \
          "$style" "$pad" "" "$text" "$(( width - pad - ''${#text} ))" ""
      }

      draw() {
        local cols lines
        read -r lines cols < <(stty size 2>/dev/null || echo "25 80")
        (( cols > 0 )) || cols=80
        (( lines > 0 )) || lines=25

        local -a body=("" "LosOS is ready" "")
        local ips
        ips=$(addresses)
        if [ -n "$ips" ]; then
          body+=("On any computer on this network," "open a web browser and go to:" "")
          local ip
          while read -r ip; do
            [ -n "$ip" ] && body+=("http://$ip" "")
          done <<<"$ips"
          body+=("or: http://$fqdn" "")
        else
          body+=(
            "No network address yet."
            "Plug in an Ethernet cable; this screen updates by itself."
            ""
            "Once connected, open a web browser and go to:"
            "http://$fqdn"
            ""
          )
        fi

        # Vertically centred, and a little narrower than the screen so the
        # blue block reads as a panel rather than a background.
        local width=$(( cols > 64 ? cols - 8 : cols ))
        local top=$(( (lines - ''${#body[@]} - 2) / 2 ))
        (( top < 0 )) && top=0
        local margin=$(( (cols - width) / 2 ))

        printf '\033[0m\033[H\033[2J\033[?25l'
        local i
        for ((i = 0; i < top; i++)); do printf '\n'; done
        line() { printf '%*s' "$margin" ""; row "$@"; }
        line "" "$width"
        for text in "''${body[@]}"; do
          case $text in
            http://*) line "$text" "$width" $'\033[1m' ;;
            "LosOS is ready") line "$text" "$width" $'\033[1m' ;;
            *) line "$text" "$width" ;;
          esac
        done
        line "" "$width"
      }

      last=
      ticks=0
      while true; do
        # Redraw when something changed, and every ~30 s regardless: a stray
        # kernel message lands on top of the banner and nothing else removes it.
        state="$(addresses | tr '\n' ' ')$(stty size 2>/dev/null || true)"
        if [ "$state" != "$last" ] || (( ticks >= 10 )); then
          draw
          last=$state
          ticks=0
        fi
        ticks=$((ticks + 1))
        sleep 3
      done
    '';
  };
in
{
  # Same pattern services.cage uses to take over a VT.
  systemd.services."getty@tty1".enable = false;
  systemd.services."autovt@tty1".enable = false;

  systemd.services.losos-console = {
    description = "Show the appliance address on tty1";
    wantedBy = [ "multi-user.target" ];
    after = [
      "systemd-user-sessions.service"
      "systemd-vconsole-setup.service"
    ];
    conflicts = [
      "getty@tty1.service"
      "autovt@tty1.service"
    ];
    before = [ "getty.target" ];
    serviceConfig = {
      ExecStart = lib.getExe banner;
      Restart = "always";
      RestartSec = 2;
      StandardInput = "tty";
      StandardOutput = "tty";
      StandardError = "journal";
      TTYPath = "/dev/tty1";
      TTYReset = true;
      TTYVHangup = true;
      TTYVTDisallocate = true;
      UtmpIdentifier = "tty1";
      UtmpMode = "user";
    };
  };
}
