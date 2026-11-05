/* Every page is an MDX file that places its blocks (src/pages/README.md).
 * The defaults are read here, at build time, by the same losos-mdx the box
 * reads an administrator's page with (src/mdx/pkg, the wasm-bindgen
 * package), so a box nobody customized draws its pages with no parser in
 * the browser, and a default that does not read, or places a block
 * blocks.json does not list, fails the build here.
 *
 * The bundle carries the HTML they become as `virtual:losos-pages`
 * (scripts/bundler.ts wires the plugin). blocks.json and the default files
 * are also copied to dist/pages/, where the `pages` plugin
 * (backend/plugins/pages/) reads them: to check what an administrator saves
 * against the same table, and to show the editor the default it starts
 * from. nginx sends /pages/ nowhere, as it does /basic/. */

const DIR = "src/pages";

interface Registry {
  pages: Record<string, unknown>;
}

interface Rendered {
  html: string;
  problems: { place: { line: number; column: number } | null; message: string }[];
}

type PageFn = (source: string, registry: string, page: string) => string;

let loaded: Promise<PageFn> | null = null;

/* The wasm-bindgen package, initialized from its bytes: no fetch, which
 * the web target would otherwise try at the module's own URL. */
function parser(root: string): Promise<PageFn> {
  loaded ??= (async () => {
    const pkg = `${root}/src/mdx/pkg`;
    const mod = (await import(`${pkg}/losos_mdx.js`)) as {
      initSync: (input: { module: Uint8Array }) => unknown;
      page: PageFn;
    };
    mod.initSync({ module: await Bun.file(`${pkg}/losos_mdx_bg.wasm`).bytes() });
    return mod.page;
  })();
  return loaded;
}

export async function compilePages(root: string): Promise<{
  registry: string;
  sources: Record<string, string>;
  html: Record<string, string>;
}> {
  const registry = await Bun.file(`${root}/${DIR}/blocks.json`).text();
  const ids = Object.keys((JSON.parse(registry) as Registry).pages).sort();
  const page = await parser(root);
  const sources: Record<string, string> = {};
  const html: Record<string, string> = {};
  const wrong: string[] = [];
  for (const id of ids) {
    const file = `${root}/${DIR}/${id}.mdx`;
    if (!(await Bun.file(file).exists())) {
      wrong.push(`${DIR}/${id}.mdx: blocks.json lists the page and there is no file`);
      continue;
    }
    const source = await Bun.file(file).text();
    let rendered: Rendered;
    try {
      rendered = JSON.parse(page(source, registry, id)) as Rendered;
    } catch (e) {
      wrong.push(`${DIR}/${id}.mdx: ${String(e)}`);
      continue;
    }
    for (const p of rendered.problems) {
      wrong.push(`${DIR}/${id}.mdx:${p.place?.line ?? 0}:${p.place?.column ?? 0}: ${p.message}`);
    }
    sources[id] = source;
    html[id] = rendered.html;
  }
  if (wrong.length > 0) throw new Error(`the default pages do not read:\n${wrong.join("\n")}`);
  return { registry, sources, html };
}

export const PAGES_DIR = DIR;
