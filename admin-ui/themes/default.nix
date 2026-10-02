# The LosOS look for Nextcloud and Forgejo, as one store path plus the facts
# each consumer needs about it.
#
# Plain data rather than a module, for modules/nextcloud-stack.nix's reason:
# the same themes are installed by NixOS modules (native mode) and baked into
# the OCI images by flake/images.nix (workload mode), and an image is built
# from the flake where there is no module system to read an option from. So
# everything both paths need is written down here once — and three callers
# import it: modules/nextcloud-stack.nix (the Nextcloud package), and, for
# Forgejo, modules/services.nix (native) and flake/images.nix (the image).
#
# The palette is not copied into this directory. Both themes ship the admin
# SPA's own admin-ui/app/src/styles/tokens.css, byte for byte, under the name
# losos-tokens.css, and map their app's variables onto it. A colour changed
# there changes the homepage, Nextcloud and Forgejo in one commit; that file's
# header says what is allowed to live in it so this stays true.
#
# Layout of the output:
#
#   nextcloud/core/css/server.css        the theme folder's body; Nextcloud
#   nextcloud/core/css/losos-tokens.css  serves it from themes/<name>/
#   forgejo/theme-losos-{auto,light,dark}.css   the three pickable themes
#   forgejo/losos-forgejo.css                   the mapping they share
#   forgejo/losos-tokens.css                    the palette
#
# Only path literals reach the store, so this derivation's inputs are four
# small CSS files and nothing from admin-ui/app beyond tokens.css: the SPA's
# node_modules and sources stay out of both closures.
{ pkgs }:

let
  tokens = ../app/src/styles/tokens.css;

  package = pkgs.runCommand "losos-themes" { } ''
    install -Dm0644 ${./nextcloud/server.css} "$out/nextcloud/core/css/server.css"
    install -Dm0644 ${tokens} "$out/nextcloud/core/css/losos-tokens.css"

    install -Dm0644 -t "$out/forgejo" ${./forgejo}/*.css
    install -Dm0644 ${tokens} "$out/forgejo/losos-tokens.css"
  '';
in
{
  inherit package;

  nextcloud = {
    # The `theme` system config value, and the directory name under the
    # server's themes/. Nextcloud ties the two by name and by nothing else.
    name = "losos";
    # Copied into themes/<name>/ of the server package itself: Nextcloud
    # resolves themes against its *real* server root, which is the package
    # path, so a symlink in the webroot would never be found.
    dir = "${package}/nextcloud";
  };

  forgejo = {
    # Everything here belongs in Forgejo's $FORGEJO_CUSTOM/public/assets/css/,
    # beside where it serves its stock themes from, so the relative @imports
    # in the theme files resolve. Forgejo lists theme-*.css from that
    # directory on start; nothing needs to enumerate them in app.ini.
    cssDir = "${package}/forgejo";
    files = [
      "losos-forgejo.css"
      "losos-tokens.css"
      "theme-losos-auto.css"
      "theme-losos-dark.css"
      "theme-losos-light.css"
    ];
    # Auto, as on the homepage: follow the browser until someone chooses.
    # Forgejo copies DEFAULT_THEME into each account when it is created, so
    # this reaches new accounts; an existing account keeps the theme it had
    # until its owner picks LosOS under Settings → Appearance.
    ui.DEFAULT_THEME = "losos-auto";
  };
}
