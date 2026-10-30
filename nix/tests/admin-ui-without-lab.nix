# The admin UI for a VM test that never opens Diagrams.
#
# The SPA imports the Diagrams screen lazily, so `tsc` and Vite still
# resolve its three WebAssembly packages (src/lab/{core,virt,render}-pkg),
# and those take most of an hour to build. Stand-ins with the same module
# names take their place; every page and the normal `bun run build` stay.
# Each stand-in's init throws, so a page that did open Diagrams would show
# its error rather than half a canvas.
#
#   withoutLab = import ./admin-ui-without-lab.nix;
#   ui = withoutLab lososPkgs.losos-admin-ui;
#   before = withoutLab (lososPkgs.losos-admin-ui.overrideAttrs { src = /path/to/main/admin-ui/app; });
ui:
ui.overrideAttrs (_: {
  pname = "losos-admin-ui-without-lab";
  preBuild = ''
    stub() {
      mkdir -p "$(dirname "$1")"
      cat > "$1.js" <<'JS'
    export default async function init() { throw new Error("no Diagrams in this build"); }
    export class Lab {}
    export class LabCanvas {}
    export class VirtClient {}
    export class VirtConsole {}
    JS
      cat > "$1.d.ts" <<'TS'
    export default function init(...args: any[]): Promise<any>;
    export class Lab { static [k: string]: any; constructor(...args: any[]); [k: string]: any }
    export class LabCanvas { static [k: string]: any; constructor(...args: any[]); [k: string]: any }
    export class VirtClient { static [k: string]: any; constructor(...args: any[]); [k: string]: any }
    export class VirtConsole { static [k: string]: any; constructor(...args: any[]); [k: string]: any }
    TS
      : > "$1_bg.wasm"
    }
    stub src/lab/core-pkg/losos_lab_core
    stub src/lab/virt-pkg/losos_lab_virt
    stub src/lab/render-pkg/webgl2/losos_lab_render
    stub src/lab/render-pkg/webgpu/losos_lab_render
  '';
})
