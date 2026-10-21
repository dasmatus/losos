# The LosOS look and the LosOS name for Nextcloud and LosOS Login, as one
# store path plus the facts each consumer needs about it. Nextcloud is
# presented as "LosOS cloud": the colours, the logos, the names and the words
# on the page are all the appliance's, and nothing links out to the project's
# marketing. (LosOS Git's pages are the admin UI's own and need nothing here.)
#
# Plain data rather than a module, for modules/nextcloud-stack.nix's reason:
# the same files are installed by NixOS modules (native mode) and baked into
# the OCI images by nix/images.nix (workload mode), and an image is built
# from the flake where there is no module system to read an option from. So
# everything both paths need is written down here once:
# modules/nextcloud-stack.nix (the Nextcloud package) and
# modules/identity.nix (LosOS Login) import it.
#
# The palette is not copied into this directory. Both themes ship the admin
# SPA's own admin-ui/app/src/styles/tokens.css, byte for byte, under the name
# losos-tokens.css, and map their app's variables onto it. A colour changed
# there changes the homepage, LosOS Git, Nextcloud and LosOS Login in one
# commit; that file's
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
#   nextcloud/                 a Nextcloud theme folder, copied whole into
#     defaults.php             the server package's themes/<name>/: the name,
#     core/css/server.css      the stylesheet and its palette, and the
#     core/css/losos-tokens.css   images under core/img/ that Nextcloud
#     core/img/…               looks up there before its own
#
#   authelia/                  LosOS Login's logo and favicon
{ pkgs }:

let
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
      '';
in
{
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
    # cloud and LosOS Git name themselves. The licence (Apache-2.0) stays in the package.
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
}
