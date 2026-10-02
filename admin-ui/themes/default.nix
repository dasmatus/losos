# The LosOS look and the LosOS names for Nextcloud and Forgejo, as one store
# path plus the facts each consumer needs about it. Nextcloud is presented as
# "LosOS cloud" and Forgejo as "LosOS Git": the colours, the logos, the names
# and the words on the page are all the appliance's, and nothing links out to
# either project's marketing.
#
# Plain data rather than a module, for modules/nextcloud-stack.nix's reason:
# the same files are installed by NixOS modules (native mode) and baked into
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
# The logos are brand/losos-cloud.svg and brand/losos-git.svg, drawn by
# brand/marks.py from brand/fish.png. Every raster an app asks for (PNG
# logos, touch icons, the .ico) is rendered from those SVGs here, so the SVGs
# are the only artwork in the repository and the rasters cannot fall out of
# step with them.
#
# Layout of the output:
#
#   nextcloud/                 a Nextcloud theme folder, copied whole into
#     defaults.php             the server package's themes/<name>/: the name,
#     core/css/server.css      the stylesheet and its palette, and the
#     core/css/losos-tokens.css   images under core/img/ that Nextcloud
#     core/img/…               looks up there before its own
#
#   forgejo/                   laid out as Forgejo's $FORGEJO_CUSTOM, so it is
#     public/assets/css/…      copied (image) or linked (native) file for file
#     public/assets/img/…      into it: the three pickable themes, the logos
#     templates/…              and favicons, and the two template overrides
{ pkgs }:

let
  inherit (pkgs) lib;

  tokens = ../app/src/styles/tokens.css;
  brand = ./brand;

  # Rasters: <size> <svg> <out>. rsvg-convert keeps the fish's pixels square
  # (it honours the marks' shape-rendering="crispEdges"); ImageMagick only
  # packs already-rendered PNGs into the .ico, so its own SVG renderer is
  # never involved.
  render = size: svg: out: ''
    install -d "$(dirname "${out}")"
    rsvg-convert -w ${toString size} -h ${toString size} ${svg} -o "${out}"
  '';

  package =
    pkgs.runCommand "losos-themes"
      {
        nativeBuildInputs = [
          pkgs.librsvg
          pkgs.imagemagick
        ];
      }
      ''
        # ── Nextcloud: themes/<name>/ ─────────────────────────────────────
        nc="$out/nextcloud"
        install -Dm0644 ${./nextcloud/defaults.php} "$nc/defaults.php"
        install -Dm0644 ${./nextcloud/server.css} "$nc/core/css/server.css"
        install -Dm0644 ${tokens} "$nc/core/css/losos-tokens.css"

        # The names Nextcloud's templates ask for: logo/logo.svg in the header
        # and on the login page, logo/logo.png in e-mails, favicon.ico and
        # favicon-touch.png in every page's <head>, favicon-mask.svg for
        # Safari's pinned tab, favicon.png for the theming app's fallback.
        install -Dm0644 ${brand}/losos-cloud.svg "$nc/core/img/logo/logo.svg"
        install -Dm0644 ${brand}/losos-cloud.svg "$nc/core/img/favicon.svg"
        install -Dm0644 ${brand}/losos-cloud.svg "$nc/core/img/favicon-mask.svg"
        ${render 512 "${brand}/losos-cloud.svg" "$nc/core/img/logo/logo.png"}
        ${render 512 "${brand}/losos-cloud.svg" "$nc/core/img/favicon-touch.png"}
        ${render 32 "${brand}/losos-cloud.svg" "$nc/core/img/favicon.png"}
        ${render 16 "${brand}/losos-cloud.svg" "ico-16.png"}
        ${render 48 "${brand}/losos-cloud.svg" "ico-48.png"}
        magick ico-16.png "$nc/core/img/favicon.png" ico-48.png "$nc/core/img/favicon.ico"

        # ── Forgejo: $FORGEJO_CUSTOM ──────────────────────────────────────
        fj="$out/forgejo"
        install -Dm0644 -t "$fj/public/assets/css" ${./forgejo}/*.css
        install -Dm0644 ${tokens} "$fj/public/assets/css/losos-tokens.css"

        install -Dm0644 ${brand}/losos-git.svg "$fj/public/assets/img/logo.svg"
        install -Dm0644 ${brand}/losos-git.svg "$fj/public/assets/img/favicon.svg"
        ${render 512 "${brand}/losos-git.svg" "$fj/public/assets/img/logo.png"}
        ${render 180 "${brand}/losos-git.svg" "$fj/public/assets/img/apple-touch-icon.png"}
        ${render 32 "${brand}/losos-git.svg" "$fj/public/assets/img/favicon.png"}

        install -Dm0644 ${./forgejo/templates/home.tmpl} "$fj/templates/home.tmpl"
        install -Dm0644 ${./forgejo/templates/custom/header.tmpl} "$fj/templates/custom/header.tmpl"

        # The list below is what native mode links; a file built here and
        # missing from it would exist in the image and silently not natively.
        ( cd "$fj" && find . -type f | sed 's|^\./||' | sort ) > files
        printf '%s\n' ${lib.escapeShellArgs forgejoFiles} | sort | diff -u - files
      '';

  forgejoFiles = [
    "public/assets/css/losos-forgejo.css"
    "public/assets/css/losos-tokens.css"
    "public/assets/css/theme-losos-auto.css"
    "public/assets/css/theme-losos-dark.css"
    "public/assets/css/theme-losos-light.css"
    "public/assets/img/apple-touch-icon.png"
    "public/assets/img/favicon.png"
    "public/assets/img/favicon.svg"
    "public/assets/img/logo.png"
    "public/assets/img/logo.svg"
    "templates/custom/header.tmpl"
    "templates/home.tmpl"
  ];
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

    # Images Nextcloud reads by absolute path under core/img/, without asking
    # the theme folder: the theming app's favicon and touch-icon routes fall
    # back to core/img/favicon.png and favicon-touch.png, and build icons from
    # core/img/logo/logo.svg when PHP has Imagick. Those are what the web-app
    # manifest points at ("add to home screen"). modules/nextcloud-stack.nix
    # overwrites these in the package with the theme folder's copies.
    coreImages = [
      "favicon.png"
      "favicon-touch.png"
      "logo/logo.svg"
    ];

    # config.php keys that take the remaining Nextcloud material out:
    settings = {
      # No "Nextcloud Manual.pdf", "Nextcloud intro.mp4", "Reasons to use
      # Nextcloud.pdf" or Readme dropped into each new account. The
      # templates directory (blank documents to start from) is separate and
      # stays.
      skeletondirectory = "";
      # Public share pages: no "Get your own free account" link to
      # nextcloud.com/signup under every shared file.
      "simpleSignUpLink.shown" = false;
      # No Help entry in the user menu, which opens docs.nextcloud.com.
      knowledgebaseenabled = false;
    };
  };

  forgejo = {
    # Everything here belongs in Forgejo's $FORGEJO_CUSTOM, at the same
    # relative path: css beside where Forgejo serves its stock themes, so the
    # relative @imports resolve; img/ where its templates load the logo and
    # favicons from; templates/ where it looks for overrides. The files in
    # customDir provide the stylesheets; ui.THEMES registers them in Appearance.
    customDir = "${package}/forgejo";

    # app.ini, in services.forgejo.settings' shape (the image renders it to
    # INI). Merged under each mode's own settings.
    settings = {
      # The name in every page title, on the front page, in the OpenGraph
      # tags and in e-mails. Forgejo's default is "Forgejo: Beyond coding. We
      # forge."
      DEFAULT.APP_NAME = "LosOS Git";

      # Auto, as on the homepage: follow the browser until someone chooses.
      # Forgejo copies DEFAULT_THEME into each account when it is created, so
      # this reaches new accounts; an existing account keeps the theme it had
      # until its owner picks LosOS under Settings → Appearance.
      ui = {
        THEMES = "auto,forgejo-auto,forgejo-light,forgejo-dark,gitea,arc-green,losos-auto,losos-light,losos-dark";
        DEFAULT_THEME = "losos-auto";
      };

      # The <meta> tags of pages that are not a repository's. Upstream's
      # describe Forgejo.
      "ui.meta" = {
        AUTHOR = "LosOS Git";
        DESCRIPTION = "The git host on your LosOS box";
        KEYWORDS = "git,losos";
      };

      # The footer's left half: "Powered by Forgejo", the version and the
      # template timings. With all three off it is empty; the language picker
      # and the licences stay on the right.
      other = {
        SHOW_FOOTER_POWERED_BY = false;
        SHOW_FOOTER_VERSION = false;
        SHOW_FOOTER_TEMPLATE_LOAD_TIME = false;
      };
    };
  };
}
