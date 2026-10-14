# The startup sound: the box plays the last seven seconds of the launch
# film's score once it is up, as the splash opens its status panel.
#
# The sound is code, not a recording. modules/startup-sound/render.py
# synthesises it at build time (numpy and scipy, fixed seeds, so every build
# renders the same samples) into a WAV in the store.
#
# It plays on every playback device ALSA finds, all at once: a built-in
# speaker, the headphone jack and each HDMI or DisplayPort output. Ports with
# nothing plugged in just fail, and nothing waits on them. The box has no
# shell for the owner to set a volume from, so `alsactl init` first unmutes
# the outputs at ALSA's default levels. A box without a sound card plays
# nothing.
#
# The box reboots itself every night at 00:07, and after an upgrade that
# changed the kernel. Those reboots stay quiet: when the box goes down for a
# reboot it leaves a marker, and the next start skips the sound and removes
# the marker. So the sound plays when the box is switched on, or comes back
# after a power cut, and not in the middle of the night.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  sound =
    pkgs.runCommand "losos-startup-sound.wav"
      {
        nativeBuildInputs = [
          (pkgs.python3.withPackages (ps: [
            ps.numpy
            ps.scipy
          ]))
        ];
      }
      ''
        python3 ${./startup-sound/render.py} $out
      '';

  quiet = "/var/lib/losos-startup-sound/quiet";

  play = pkgs.writeShellApplication {
    name = "losos-startup-sound";
    runtimeInputs = [
      pkgs.alsa-utils
      pkgs.coreutils
      pkgs.gnugrep
    ];
    text = ''
      if [ -e ${quiet} ]; then
        rm -f ${quiet}
        exit 0
      fi

      # Started more than five minutes after boot, this is a configuration
      # switch bringing the unit in (the first update that carries it), not a
      # boot.
      read -r up _ </proc/uptime
      (( ''${up%.*} < 300 )) || exit 0

      # Sound drivers load from udev, sometimes after this unit starts.
      for _ in $(seq 20); do
        grep -q playback /proc/asound/pcm 2>/dev/null && break
        sleep 0.5
      done
      [ -r /proc/asound/pcm ] || exit 0

      alsactl init >/dev/null 2>&1 || true

      # Lines look like "00-03: HDMI 0 : HDMI 0 : playback 1".
      while IFS=: read -r id _ _ rest; do
        case $rest in
          *playback*) ;;
          *) continue ;;
        esac
        card=$((10#''${id%%-*}))
        device=$((10#''${id##*-}))
        aplay -q -D "plughw:$card,$device" ${sound} >/dev/null 2>&1 &
      done </proc/asound/pcm
      wait
    '';
  };

  # Runs when the unit stops, which is at shutdown: a pending reboot leaves
  # the marker for the next start.
  mark = pkgs.writeShellScript "losos-startup-sound-mark" ''
    if ${pkgs.systemd}/bin/systemctl list-jobs --no-legend | ${pkgs.gnugrep}/bin/grep -qE '(reboot|kexec)\.target'; then
      : > ${quiet}
    fi
  '';
in
{
  config = lib.mkIf config.losos.startupSound.enable {
    systemd.services.losos-startup-sound = {
      description = "Play the LosOS startup sound";
      wantedBy = [ "multi-user.target" ];
      # With the console up, the splash has its panel, so the sound and the
      # panel arrive together.
      after = [
        "sound.target"
        "losos-console.service"
      ];
      # A nightly switch must not play it again, nor run the stop step.
      restartIfChanged = false;
      serviceConfig = {
        # simple, so nothing waits seven seconds for the sound; the unit then
        # stays active until shutdown, when ExecStop leaves the marker.
        Type = "simple";
        RemainAfterExit = true;
        ExecStart = lib.getExe play;
        ExecStop = mark;
        StateDirectory = "losos-startup-sound";
        ProtectSystem = "strict";
        ProtectHome = true;
        PrivateNetwork = true;
        PrivateTmp = true;
        NoNewPrivileges = true;
      };
    };
  };
}
