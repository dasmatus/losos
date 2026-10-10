# The boot screen, and the status box on it: Plymouth with a LosOS theme
# (modules/splash/losos.script) that draws the logo on the dark palette and,
# under it, a panel. While the box boots the panel says it is starting; once
# it is up, modules/console.nix sends it the rows it would otherwise print on
# tty1 (the address to open, and the warning a box without a TPM chip
# carries), and the splash stays on screen as the box's face.
#
# Plymouth normally quits when the boot finishes. Here nothing asks it to:
# plymouth-quit and plymouth-quit-wait are taken out of multi-user.target,
# and there is no display manager to quit it either. tty1 then keeps the
# picture, and kernel messages no longer land on top of the status the way
# they do on the text banner. Esc shows the boot log and Esc again hides it.
# The other consoles keep their login prompts behind Alt+F2 to Alt+F6.
#
# Plymouth draws on the display it finds in its first seconds, which on a
# UEFI machine is the firmware's framebuffer. A BIOS machine hands GRUB's
# graphics mode on to the kernel for the same reason. Where there is still
# none (a machine whose graphics driver loads late, no screen at all)
# Plymouth runs in text mode, and tty1 shows the text banner. With
# losos.splash.enable = false the box boots in text and tty1 shows the text
# banner too.
#
# The installer medium boots under the same screen. There the panel is the
# installer's (modules/installer.nix, backend/src/install_screen.rs): the
# firmware menu answered with a key press, the steps as they run, and how
# the install ended. Without a display it runs in text on tty1 as before.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  enable = config.losos.splash.enable;
  plymouth = config.boot.plymouth.package;

  art = import ./brand-art.nix {
    inherit pkgs;
    version = import ../flake/version.nix;
  };
  inherit (art.palette)
    ground
    ink
    muted
    accent
    ;

  # The theme's colours as the 0..1 fractions Plymouth's script wants:
  # tokens.css (art.palette), plus the warning yellow from
  # modules/branding.nix's console palette. The panel is a picture in the
  # blue that palette puts behind the tty1 banner.
  colour =
    name: hex:
    lib.concatMapStrings
      (c: "${name}.${c.n} = ${toString ((lib.fromHexString (builtins.substring c.i 2 hex)) / 255.0)};\n")
      [
        {
          n = "r";
          i = 1;
        }
        {
          n = "g";
          i = 3;
        }
        {
          n = "b";
          i = 5;
        }
      ];

  # The caption: "LosOS v0.1.8 is starting", or on the installer medium
  # "LosOS v0.1.8 installer is starting".
  name = lib.concatStringsSep " " (
    [ "LosOS" ]
    ++ lib.optional (config.system.image.version != null) config.system.image.version
    ++ lib.optional config.losos.installer.autorun "installer"
  );

  theme = pkgs.runCommand "losos-plymouth-theme" { nativeBuildInputs = [ pkgs.imagemagick ]; } ''
    dir=$out/share/plymouth/themes/losos
    mkdir -p $dir
    cp ${art}/splash-logo.png $dir/logo.png
    magick -size 8x8 xc:'#0d2a2f' PNG24:$dir/panel.png
    magick -size 8x8 xc:'${accent}' PNG24:$dir/accent.png
    cat > $dir/losos.script <<'EOF'
    ${colour "ground" ground}${colour "ink" ink}${colour "muted" muted}${colour "accent" accent}${colour "warn" "#dbb35e"}name = "${name}";
    EOF
    cat ${./splash/losos.script} >> $dir/losos.script
    cat > $dir/losos.plymouth <<EOF
    [Plymouth Theme]
    Name=LosOS
    Description=The LosOS logo and the box's status
    ModuleName=script

    [script]
    ImageDir=$dir
    ScriptFile=$dir/losos.script
    EOF
  '';

  # The theme asks for DejaVu Sans and DejaVu Sans Bold by name. The NixOS
  # module copies one font file into the initrd; this folder has both, and
  # the running system carries it at the same path, because plymouthd moves
  # into the real root with the rest of the boot and draws the status rows
  # from there.
  fonts = pkgs.runCommand "losos-plymouth-fonts" { } ''
    mkdir -p $out
    cp ${pkgs.dejavu_fonts}/share/fonts/truetype/DejaVuSans{,-Bold}.ttf $out
  '';
in
lib.mkIf enable {
  boot.plymouth = {
    enable = true;
    theme = "losos";
    themePackages = [ theme ];
    logo = "${art}/splash-logo.png";
  };

  # GRUB already draws in this mode (modules/branding.nix); "keep" leaves it
  # to the kernel as a framebuffer Plymouth can draw on from stage 1.
  boot.loader.grub.gfxpayloadBios = "keep";

  boot.initrd.systemd.contents."/etc/plymouth/fonts".source = lib.mkForce fonts;
  environment.etc."plymouth/fonts".source = fonts;

  # The firmware's framebuffer shows the kernel's messages until Plymouth
  # takes the screen in stage 1. Errors still print, and every message is in
  # the journal and behind Esc.
  boot.kernelParams = [
    "quiet"
    # A serial console on the command line would otherwise turn the local
    # screen's splash into the text log, and the status is for that screen.
    "plymouth.ignore-serial-consoles"
  ];
  boot.consoleLogLevel = lib.mkDefault 3;

  # Nothing quits the splash at the end of the boot.
  systemd.services.plymouth-quit.wantedBy = lib.mkForce [ ];
  systemd.services.plymouth-quit-wait.wantedBy = lib.mkForce [ ];

  # At shutdown the running splash says the box is restarting or shutting
  # down. Plymouth's own reboot and poweroff units start a second plymouthd,
  # which finds this one running and leaves it be, so the mode comes from
  # here. Ordered before losos-console, so it is stopped after it and no
  # status row lands on top. Never restarted by a switch: a nightly
  # upgrade must not read as a shutdown.
  systemd.services.losos-splash-mode = {
    description = "Tell the boot screen when the box shuts down";
    wantedBy = [ "multi-user.target" ];
    after = [ "plymouth-start.service" ];
    before = [ "losos-console.service" ];
    restartIfChanged = false;
    path = [
      plymouth
      config.systemd.package
      pkgs.gnugrep
    ];
    serviceConfig = {
      Type = "oneshot";
      RemainAfterExit = true;
      ExecStart = "${pkgs.coreutils}/bin/true";
    };
    preStop = ''
      if systemctl list-jobs --no-legend | grep -Eq '(reboot|kexec|soft-reboot)\.target'; then
        plymouth update --status=losos-mode:reboot || true
      else
        plymouth update --status=losos-mode:shutdown || true
      fi
    '';
  };
}
