# The LosOS look and the LosOS name for Forgejo ("LosOS Git") and LosOS
# Login, as one store path plus the facts each consumer needs about it: the
# colours, the logos, the names and the words on the page are all the
# appliance's, and nothing links out to either project's marketing.
#
# Plain data rather than a module: the same files are installed by NixOS
# modules (native mode) and baked into the OCI image by nix/images.nix
# (workload mode), and an image is built from the flake where there is no
# module system to read an option from. So everything both paths need is
# written down here once.
#
# The palette is not copied into this directory. Both themes ship the admin
# SPA's own admin-ui/app/src/styles/tokens.css, byte for byte, under the name
# losos-tokens.css, and map their app's variables onto it. A colour changed
# there changes the homepage, LosOS Git and LosOS Login in one commit; that file's
# header says what is allowed to live in it so this stays true.
#
# The logos are brand/losos-cloud.svg and brand/losos-git.svg, drawn by
# brand/marks.py around brand/salmon.png (a live coho salmon cut out of a
# NOAA Fisheries photo, see brand/CREDITS.md; the SVGs embed it as a data:
# URI so each stands alone).
# Every raster an app asks for (PNG logos, touch icons, the .ico) is
# rendered from those SVGs here, so the rasters cannot fall out of step
# with them.
#
# Layout of the output:
#
#   office/branding.css        tokens.css and office/branding.css in one
#                              file, served as Collabora's branding.css
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

  # Rasters: <size> <svg> <out>. rsvg-convert draws the embedded salmon
  # through gdk-pixbuf; ImageMagick only packs already-rendered PNGs into
  # the .ico, so its own SVG renderer is never involved.
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
        # ── LosOS Office: Collabora's branding.css ────────────────────────
        # One file: the palette, then the rules that dress the editor in it
        # (./office/branding.css says why both are one file).
        install -d "$out/office"
        cat ${tokens} ${./office/branding.css} > "$out/office/branding.css"

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

        # ── Authelia: server.asset_path ───────────────────────────────────
        # The sign-in pages' logo and favicon. Authelia serves a file from
        # this directory in place of its own when one is there
        # (internal/server/handlers.go, AssetOverride).
        au="$out/authelia"
        install -Dm0644 ${brand}/salmon.png "$au/logo.png"
        ${render 16 "${brand}/losos-cloud.svg" "au-16.png"}
        ${render 32 "${brand}/losos-cloud.svg" "au-32.png"}
        ${render 48 "${brand}/losos-cloud.svg" "au-48.png"}
        magick au-16.png au-32.png au-48.png "$au/favicon.ico"

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
  # LosOS Office (modules/office.nix): the stylesheet coolwsd's editor page
  # links as /browser/<version>/branding.css.
  office.branding = "${package}/office/branding.css";

  inherit package;

  authelia = {
    # server.asset_path: logo.png and favicon.ico.
    assets = "${package}/authelia";

    # Authelia's web portal, rebuilt with the LosOS look. Its pages are
    # Tailwind and shadcn/ui like the admin SPA's, so the whole palette is
    # one file: ./authelia/losos-authelia.css replaces its
    # src/themes/tailwind-themes.css and points shadcn's names at tokens.css.
    # Two more edits take the upstream name off the pages: the portal is
    # called LosOS Login, so its page titles read "Login - LosOS Login" (the
    # name is kept base64-encoded in
    # src/constants/constants.ts, as an array of character codes), and the
    # "Powered by" line under the form names LosOS Login instead, as LosOS
    # Git names itself. The licence (Apache-2.0) stays in the package.
    web =
      web:
      web.overrideAttrs (old: {
        postPatch = (old.postPatch or "") + ''
          install -m0644 ${./authelia/losos-authelia.css} src/themes/tailwind-themes.css
          install -m0644 ${tokens} src/themes/losos-tokens.css
          substituteInPlace src/constants/constants.ts \
            --replace-fail 'export const EncodedName = [81, 88, 86, 48, 97, 71, 86, 115, 97, 87, 69, 61];' \
                           'export const EncodedName = [84, 71, 57, 122, 84, 49, 77, 103, 84, 71, 57, 110, 97, 87, 52, 61];'
          cat > src/components/Brand.tsx <<'TSX'
        export interface Props {}

        const Brand = function () {
            return <p className="mt-4 text-center text-sm text-muted-foreground">LosOS Login</p>;
        };

        export default Brand;
        TSX
        '';
      });
  };

  forgejo = {
    # Everything here belongs in Forgejo's $FORGEJO_CUSTOM  forgejo = {
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
