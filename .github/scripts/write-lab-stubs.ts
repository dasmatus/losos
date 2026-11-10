import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

// Throwaway tsc stubs for gitignored lab/mdx wasm pkg dirs (nix/labPkg supply
// the real wasm-bindgen outputs; the #ci shell has no wasm-bindgen). Surfaces
// match what admin-ui/app/src imports so module resolution stays true.
const root = process.cwd();

function write(file: string, text: string) {
  writeFileSync(join(root, file), text, "utf8");
}

for (const dir of [
  "src/lab/core-pkg",
  "src/lab/virt-pkg",
  "src/mdx/pkg",
  "src/lab/render-pkg/webgl2",
  "src/lab/render-pkg/webgpu",
]) {
  mkdirSync(join(root, dir), { recursive: true });
}

write("src/lab/core-pkg/losos_lab_core.js", "export default {};\n");
write(
  "src/lab/core-pkg/losos_lab_core.d.ts",
  [
    "export class Lab { constructor(...args: any[]); [key: string]: any; }",
    "export type InitInput = any;",
    "export function initSync(module?: any): any;",
    "export default function __wbg_init(module_or_path?: any): Promise<any>;",
    "",
  ].join("\n"),
);

write("src/lab/virt-pkg/losos_lab_virt.js", "export default {};\n");
write(
  "src/lab/virt-pkg/losos_lab_virt.d.ts",
  [
    "export class VirtClient { [key: string]: any; static [key: string]: any; }",
    "export class VirtConsole { [key: string]: any; }",
    "export type InitInput = any;",
    "export function initSync(module?: any): any;",
    "export default function __wbg_init(module_or_path?: any): Promise<any>;",
    "",
  ].join("\n"),
);

write("src/mdx/pkg/losos_mdx.js", "export default {};\n");
write(
  "src/mdx/pkg/losos_mdx.d.ts",
  [
    "export function date(value: string, long: boolean, lang: string): string;",
    "export function mathml(tex: string, block: boolean): string | undefined;",
    "export function page(source: string, registry: string, page: string): string;",
    "export function render(source: string, flavour_name: string, resolver?: any, lang?: string | null): string;",
    "export function renderNote(source: string, flavour_name: string, resolver?: any): string;",
    "export function toNote(source: string): string;",
    "export function tree(source: string, flavour_name: string): string;",
    "export type InitInput = any;",
    "export function initSync(module?: any): any;",
    "export default function __wbg_init(module_or_path?: any): Promise<any>;",
    "",
  ].join("\n"),
);

for (const [dir, base] of [
  ["src/lab/core-pkg", "losos_lab_core"],
  ["src/lab/virt-pkg", "losos_lab_virt"],
  ["src/mdx/pkg", "losos_mdx"],
] as const) {
  write(`${dir}/${base}_bg.wasm`, "");
  write(`${dir}/${base}_bg.wasm.d.ts`, "declare const url: string;\nexport default url;\n");
}

for (const api of ["webgl2", "webgpu"]) {
  write(`src/lab/render-pkg/${api}/losos_lab_render.js`, "export default {};\n");
  write(
    `src/lab/render-pkg/${api}/losos_lab_render.d.ts`,
    [
      "export default function init(opts?: any): Promise<any>;",
      "export declare const Lab: any;",
      "export declare class LabCanvas { constructor(...args: any[]); [key: string]: any; static [key: string]: any; }",
      "",
    ].join("\n"),
  );
  write(`src/lab/render-pkg/${api}/losos_lab_render_bg.wasm`, "");
}
