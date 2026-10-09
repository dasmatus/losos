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
# are as useless as before but also as harmless; their help line points
# back here instead of at a manual no one can log in to read.
#
# With the boot splash on (modules/splash.nix, the default) the same rows also
# go to Plymouth, which draws them in a panel under the logo; the text banner
# is what tty1 shows wherever Plymouth is not drawing.
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

  # "LosOS v0.1.8 is ready", with the release tag modules/branding.nix sets.
  ready = lib.concatStringsSep " " (
    [ "LosOS" ]
    ++ lib.optional (config.system.image.version != null) config.system.image.version
    ++ [ "is ready" ]
  );

  # A keyfile box says what that costs on the one screen every owner sees
  # (wiki/TPM.md has the long form). The admin pages say the same.
  noTpm = !config.losos.tpm.enable;

  splash = config.losos.splash.enable;

  banner = pkgs.writeShellApplication {
    name = "losos-console-banner";
    runtimeInputs = [
      pkgs.coreutils
      pkgs.gawk
      pkgs.gnugrep
      pkgs.iproute2
    ]
    ++ lib.optional splash config.boot.plymouth.package;
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
        local right=$(( width - pad - ''${#text} ))
        (( right < 0 )) && right=0
        printf '\033[44;97m%s%*s%s%*s\033[0m\n' \
          "$style" "$pad" "" "$text" "$right" ""
      }

      # The status as rows, each led by a style character: # headline,
      # @ address, ! warning headline, ~ secondary text, a space for plain
      # text. An empty row is a gap. The splash theme
      # (modules/splash/losos.script) reads the same characters. Fills the
      # caller's `body`.
      rows() {
        body=(${lib.escapeShellArg "#${ready}"} "")
        local ips
        ips=$(addresses)
        if [ -n "$ips" ]; then
          body+=("~On any computer on this network," "~open a web browser and go to:" "")
          local ip
          while read -r ip; do
            [ -n "$ip" ] && body+=("@http://$ip" "")
          done <<<"$ips"
          # The name second, and qualified: a computer that does not resolve
          # mDNS (the host of a libvirt VM, for one) gets "server not found"
          # from it, and an owner who picked it over the address above had
          # no way to know the two were not equally good.
          body+=("~or, on computers that find the box by name:" "@http://$fqdn" "")
        else
          body+=(
            " No network address yet."
            "~Plug in an Ethernet cable; this screen updates by itself."
            ""
            "~Once connected, open a web browser and go to:"
            "@http://$fqdn"
            ""
          )
        fi
        ${lib.optionalString noTpm ''
          body+=(
            "!This box has no TPM chip."
            "~Its disk key is on the unencrypted boot partition,"
            "~so anyone who takes the disk can read your files."
            ""
          )
        ''}
      }

      draw() {
        local cols lines
        read -r lines cols < <(stty -F /dev/tty1 size 2>/dev/null || echo "25 80")
        (( cols > 0 )) || cols=80
        (( lines > 0 )) || lines=25

        local -a body
        rows
        body=("" "''${body[@]}")

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
            [#@!]*) line "''${text:1}" "$width" $'\033[1m' ;;
            *) line "''${text:1}" "$width" ;;
          esac
        done
        line "" "$width"
      }
      ${lib.optionalString splash ''

        # The rows without the trailing gap, joined with | for the splash
        # theme. Plymouth carries at most 254 bytes in one message, so a
        # longer text (a box without a TPM chip, or with several addresses)
        # goes in parts the theme joins: `losos+:` for each part but the
        # last, `losos:` for the last. Sent whenever Plymouth runs. The text banner is drawn as
        # well: under a drawing splash tty1 is in graphics mode and the text
        # stays out of sight, and a splash that found no display in its
        # first seconds runs in text mode, where the banner is what shows.
        send() {
          local -a body
          rows
          local IFS='|' LC_ALL=C
          local text="''${body[*]:0:''${#body[@]}-1}"
          while (( ''${#text} > 200 )); do
            plymouth update --status="losos+:''${text:0:200}"
            text=''${text:200}
          done
          plymouth update --status="losos:$text"
        }
      ''}

      last=
      ticks=0
      while true; do
        # Redraw when something changed, and every ~30 s regardless: a stray
        # kernel message lands on top of the banner and nothing else removes
        # it.
        state="$(addresses | tr '\n' ' ')$(stty -F /dev/tty1 size 2>/dev/null || true)"
        if [ "$state" != "$last" ] || (( ticks >= 10 )); then
          ${lib.optionalString splash "plymouth --ping 2>/dev/null && send"}
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
  # Printed under the banner of every other console's login prompt. No one
  # can log in there: neither user has a password.
  services.getty.helpLine = lib.mkForce "\nThis box has no console login. Its address is on tty1 (Alt+F1).";

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
      StandardOutput = "tty";
      StandardError = "journal";
      TTYPath = "/dev/tty1";
      UtmpIdentifier = "tty1";
      UtmpMode = "user";
    }
    # Taking the terminal resets and hangs it up, which would pull it from
    # under a running splash. Without one the banner owns tty1 outright.
    // lib.optionalAttrs (!splash) {
      StandardInput = "tty";
      TTYReset = true;
      TTYVHangup = true;
      TTYVTDisallocate = true;
    };
  };
}
