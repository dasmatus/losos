# What nix/tests/diagrams.nix and nix/tests/diagrams-together.nix share:
# the admin UI with LosOS Diagrams drawing on SVG only, and a diagram made
# in another tool.
{
  pkgs,
  # The admin UI package to build it from.
  shipped,
}:

let
  # The page imports the GPU canvas lazily; a module that refuses to start
  # keeps it on SVG, as a browser without WebGPU or WebGL2 does.
  noGpu = ''
    for api in webgpu webgl2; do
      mkdir -p src/lab/render-pkg/$api
      echo 'export default async function () { throw new Error("no GPU canvas in this test"); }' \
        > src/lab/render-pkg/$api/losos_lab_render.js
      printf '\0asm\1\0\0\0' > src/lab/render-pkg/$api/losos_lab_render_bg.wasm
    done
  '';
  ui = shipped.overrideAttrs (o: {
    pname = "losos-admin-ui-svg-only";
    preBuild = o.passthru.labCorePkg + o.passthru.labVirtPkg + noGpu;
    # vite alone: the type check needs the real module's declarations.
    buildPhase = ''
      runHook preBuild
      bun --bun vite build
      runHook postBuild
    '';
  });

  # A diagram made elsewhere, as Obsidian Canvas writes one: a group, cards
  # in Markdown, shapes and colours with Advanced Canvas's names, labelled
  # arrows, and one key from another plugin on the document and a card.
  launch = pkgs.writeText "launch.canvas" (
    builtins.toJSON {
      nodes = [
        {
          id = "week";
          type = "group";
          label = "Moving the office to LosOS";
          x = -620;
          y = -300;
          width = 1240;
          height = 620;
          color = "5";
        }
        {
          id = "nas";
          type = "text";
          text = "## Back up the old NAS\n- [x] photos\n- [x] shared drive\n- [ ] invoices from 2024";
          x = -580;
          y = -240;
          width = 300;
          height = 180;
          "x-tasks-plugin" = {
            due = "2026-10-30";
          };
        }
        {
          id = "copied";
          type = "text";
          text = "Everything copied?";
          x = -200;
          y = -220;
          width = 240;
          height = 140;
          color = "3";
          styleAttributes = {
            shape = "diamond";
          };
        }
        {
          id = "cloud";
          type = "text";
          text = "**LosOS Cloud**\nfiles for the whole team";
          x = 140;
          y = -230;
          width = 260;
          height = 120;
          color = "4";
          styleAttributes = {
            shape = "database";
          };
        }
        {
          id = "dns";
          type = "text";
          text = "Point `office.example` at the box";
          x = 140;
          y = 0;
          width = 260;
          height = 90;
          color = "6";
          styleAttributes = {
            shape = "pill";
          };
        }
        {
          id = "fix";
          type = "text";
          text = "> [!warning] Not yet\n> copy what is missing, then check again";
          x = -580;
          y = 40;
          width = 320;
          height = 140;
          color = "1";
          styleAttributes = {
            border = "dashed";
          };
        }
        {
          id = "guide";
          type = "link";
          url = "https://losos.dasmat.us";
          x = 440;
          y = -230;
          width = 190;
          height = 120;
        }
      ];
      edges = [
        {
          id = "e1";
          fromNode = "nas";
          fromSide = "right";
          toNode = "copied";
          toSide = "left";
          toEnd = "arrow";
        }
        {
          id = "e2";
          fromNode = "copied";
          fromSide = "right";
          toNode = "cloud";
          toSide = "left";
          toEnd = "arrow";
          label = "yes";
        }
        {
          id = "e3";
          fromNode = "copied";
          fromSide = "bottom";
          toNode = "fix";
          toSide = "top";
          toEnd = "arrow";
          label = "no";
          styleAttributes = {
            path = "short-dashed";
          };
        }
        {
          id = "e4";
          fromNode = "cloud";
          fromSide = "bottom";
          toNode = "dns";
          toSide = "top";
          toEnd = "arrow";
        }
      ];
      "x-board-plugin" = {
        lane = "this week";
      };
    }
  );

in
{
  inherit ui launch;
}
