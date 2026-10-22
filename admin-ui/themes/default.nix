# The LosOS look and the LosOS name for LosOS Login, as one store path plus
# the facts its module needs about it: the colours, the logo, the name and
# the words on the page are all the appliance's, and nothing links out to
# the upstream project's marketing. (LosOS Cloud's and LosOS Git's pages are
# the admin UI's own and need nothing here.)
#
# Plain data rather than a module, so modules/identity.nix (LosOS Login) and
# nix/packages.nix (the losos-themes package) read the same facts.
#
# The palette is not copied into this directory. The sign-in page ships the
# admin SPA's own admin-ui/app/src/styles/tokens.css, byte for byte, under
# the name losos-tokens.css, and maps its variables onto it. A colour changed
# there changes the homepage, LosOS Cloud, LosOS Git and LosOS Login in one
# commit; that file's header says what is allowed to live in it so this
# stays true.
#
# The logos are brand/losos-cloud.svg and brand/losos-git.svg, drawn by
# brand/marks.py around brand/salmon.png (a live coho salmon cut out of a
# NOAA Fisheries photo, see brand/CREDITS.md; the SVGs embed it as a data:
# URI so each stands alone).
# The favicon's rasters are rendered from those SVGs here, so they cannot
# fall out of step with them.
#
# Layout of the output:
#
#   office/branding.css        tokens.css and office/branding.css in one
#                              file, served as Collabora's branding.css
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
        # ── LosOS Office: Collabora's branding.css ────────────────────────
        # One file: the palette, then the rules that dress the editor in it
        # (./office/branding.css says why both are one file).
        install -d "$out/office"
        cat ${tokens} ${./office/branding.css} > "$out/office/branding.css"


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
}
