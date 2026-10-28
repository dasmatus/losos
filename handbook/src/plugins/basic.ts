import fs from 'node:fs/promises';
import path from 'node:path';
import {createSlugger, parseMarkdownHeadingId} from '@docusaurus/utils';
import type {LoadContext, Plugin} from '@docusaurus/types';

/* The handbook without JavaScript. The same source builds a second time with
 * LOSOS_HANDBOOK_BASIC=1, under basic/ of the site's own base
 * (docusaurus.config.ts), and this plugin is what makes that copy work with
 * scripts off and the usual copy send a browser there:
 *
 *   - in the usual build it writes a <noscript> refresh into the head of
 *     every page, to the same page under basic/, as the admin UI's
 *     index.html does for its own /basic/ pages;
 *   - in the basic build it adds the Contents page (every page and its
 *     sections, which the phone's menu button and the search box open in
 *     place of the sheet and the search index they need a script for), then
 *     takes every <script> out of the finished HTML, deletes the bundles,
 *     the search page and its index, and points each page's canonical
 *     address back at the usual copy, so a search engine lists one of the
 *     two.
 *
 * Everything else the pages need without a script is the theme's own
 * server-rendered HTML; the few controls that only a script could drive
 * render differently when siteConfig.customFields.basic is set (src/theme). */

export type BasicOptions = {
  /** This build is the copy without JavaScript. */
  basic: boolean;
  /** The site's own base, "/" or "/handbook/", without basic/. */
  siteBase: string;
};

/** One line of the Contents page: a page or a chapter, and what is under it. */
export type ContentsEntry = {
  label: string;
  /** Absolute, with the base. Absent for a chapter without a page. */
  to?: string;
  /** The page's sections (##), each with its subsections (###). */
  sections?: ContentsSection[];
  items?: ContentsEntry[];
};

export type ContentsSection = {
  label: string;
  to: string;
  items: ContentsSection[];
};

// The two shapes this plugin reads out of the docs plugin's loaded content.
// Typed here rather than imported: the docs plugin's own types are internal.
type LoadedDoc = {
  id: string;
  title: string;
  permalink: string;
  source: string;
  frontMatter: {sidebar_label?: string};
};
type SidebarItem =
  | {type: 'doc' | 'ref'; id: string; label?: string}
  | {type: 'link'; href: string; label: string}
  | {type: 'html'}
  | {
      type: 'category';
      label: string;
      items: SidebarItem[];
      link?: {type: 'doc'; id: string} | {type: 'generated-index'; permalink: string};
    };
type LoadedVersion = {docs: LoadedDoc[]; sidebars: Record<string, SidebarItem[]>};

/** The ## and ### headings of a Markdown page, with the ids Docusaurus gives
 * them. Ids come from the same slugger over the heading's text with its
 * Markdown taken out; an explicit {#id} wins, as it does in the docs plugin.
 * A wrong id is not silent: the page links each one, and onBrokenAnchors
 * fails the build. */
function headings(markdown: string, permalink: string): ContentsSection[] {
  const slugger = createSlugger();
  const out: ContentsSection[] = [];
  let fence: string | null = null;
  for (const line of markdown.split('\n')) {
    const open = /^\s*(`{3,}|~{3,})/.exec(line);
    if (open) {
      const mark = open[1]!;
      if (fence === null) fence = mark;
      else if (mark.startsWith(fence)) fence = null;
      continue;
    }
    if (fence !== null) continue;
    const match = /^(#{1,6})\s+(.*?)\s*#*\s*$/.exec(line);
    if (!match) continue;
    const level = match[1]!.length;
    const {text, id: explicit} = parseMarkdownHeadingId(match[2]!);
    const label = text
      .replace(/!\[[^\]]*\]\([^)]*\)/g, '')
      .replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
      .replace(/<[^>]+>/g, '')
      .replace(/`([^`]*)`/g, '$1')
      .replace(/(\*\*|__)(.*?)\1/g, '$2')
      .replace(/(\*|_)(.*?)\1/g, '$2')
      .trim();
    // The slugger keeps count of every heading it has seen, the page's title
    // included, so that a repeated heading gets "-1" just as on the page.
    const id = explicit ?? slugger.slug(label);
    if (level === 2) {
      out.push({label, to: `${permalink}#${id}`, items: []});
    } else if (level === 3 && out.length > 0) {
      out[out.length - 1]!.items.push({label, to: `${permalink}#${id}`, items: []});
    }
  }
  return out;
}

async function contents(
  siteDir: string,
  version: LoadedVersion,
): Promise<ContentsEntry[]> {
  const docs = new Map(version.docs.map((doc) => [doc.id, doc]));
  const page = async (id: string, label?: string): Promise<ContentsEntry> => {
    const doc = docs.get(id);
    if (doc === undefined) throw new Error(`basic: no doc ${id} for the Contents page`);
    const file = path.join(siteDir, doc.source.replace(/^@site\//, ''));
    const markdown = await fs.readFile(file, 'utf8');
    return {
      label: label ?? doc.frontMatter.sidebar_label ?? doc.title,
      to: doc.permalink,
      sections: headings(markdown, doc.permalink),
    };
  };
  const walk = async (items: SidebarItem[]): Promise<ContentsEntry[]> => {
    const out: ContentsEntry[] = [];
    for (const item of items) {
      if (item.type === 'doc' || item.type === 'ref') {
        out.push(await page(item.id, item.label));
      } else if (item.type === 'link') {
        out.push({label: item.label, to: item.href});
      } else if (item.type === 'category') {
        const entry: ContentsEntry = {label: item.label, items: await walk(item.items)};
        if (item.link?.type === 'doc') {
          const index = await page(item.link.id, item.label);
          entry.to = index.to;
          entry.sections = index.sections;
        } else if (item.link?.type === 'generated-index') {
          entry.to = item.link.permalink;
        }
        out.push(entry);
      }
    }
    return out;
  };
  return (await Promise.all(Object.values(version.sidebars).map(walk))).flat();
}

/** Every HTML file the build wrote for its routes. Routes, not a walk of the
 * output directory: the default locale's directory also holds the other
 * locales' copies. */
function htmlFiles(outDir: string, baseUrl: string, routesPaths: string[]): string[] {
  return [
    ...new Set(
      routesPaths.map((route) => {
        const rel = route.startsWith(baseUrl) ? route.slice(baseUrl.length) : route;
        return rel.endsWith('.html') ? rel : path.posix.join(rel, 'index.html');
      }),
    ),
  ];
}

/** The page with every <script> element and script preload taken out. Run
 * to a fixed point, so that taking one out cannot join the pieces around it
 * into another; the build also fails if a `<script` is left afterwards. */
function withoutScripts(html: string): string {
  let out = html;
  for (;;) {
    const next = out
      .replace(/<script\b[^>]*>[\s\S]*?<\/script\s*>/gi, '')
      .replace(/<link\b[^>]*\bas="script"[^>]*>/gi, '');
    if (next === out) return out;
    out = next;
  }
}

/** Whether every file in `dir` is also in `other`, at the same size. */
async function sameFiles(dir: string, other: string): Promise<boolean> {
  const names = await fs.readdir(dir).catch(() => [] as string[]);
  for (const name of names) {
    const [a, b] = await Promise.all([
      fs.stat(path.join(dir, name)),
      fs.stat(path.join(other, name)).catch(() => null),
    ]);
    if (b === null || a.size !== b.size) return false;
  }
  return names.length > 0;
}

// Options arrive untyped (a PluginModule takes `unknown`); the config
// passes a BasicOptions and checks it with `satisfies`.
export default function basicPlugin(context: LoadContext, options: unknown): Plugin {
  const {basic, siteBase} = options as BasicOptions;
  const {baseUrl, siteDir, outDir} = context;
  // The locale's part of the base ("sk/" or ""), which both copies share.
  const locale = baseUrl.slice((basic ? `${siteBase}basic/` : siteBase).length);
  const usual = `${siteBase}${locale}`;
  const plain = `${siteBase}basic/${locale}`;

  return {
    name: 'losos-handbook-basic',

    async allContentLoaded({allContent, actions}) {
      if (!basic) return;
      const docs = allContent['docusaurus-plugin-content-docs']?.['default'] as
        | {loadedVersions: LoadedVersion[]}
        | undefined;
      const version = docs?.loadedVersions[0];
      if (version === undefined) throw new Error('basic: the docs plugin loaded nothing');
      const data = await actions.createData(
        'contents.json',
        JSON.stringify(await contents(siteDir, version)),
      );
      actions.addRoute({
        path: `${baseUrl}contents`,
        component: '@site/src/components/contents/index.tsx',
        exact: true,
        modules: {contents: data},
      });
    },

    async postBuild({routesPaths}) {
      const files = htmlFiles(outDir, baseUrl, routesPaths);
      if (!basic) {
        // http-equiv refresh is the one redirect that needs no script and
        // no server, so it works on Pages and on the box alike. Relative to
        // the locale's base, so a deep link lands on the same page.
        for (const rel of files) {
          const file = path.join(outDir, rel);
          // The search page needs its script and its index, so the basic
          // copy has neither; the Contents page is where it sends a reader.
          const target =
            rel === 'search/index.html'
              ? `${plain}contents`
              : plain + rel.replace(/(^|\/)index\.html$/, '$1');
          const html = await fs.readFile(file, 'utf8');
          await fs.writeFile(
            file,
            html.replace(
              '<head>',
              `<head><noscript><meta http-equiv="refresh" content="0; url=${target}"></noscript>`,
            ),
          );
        }
        return;
      }

      // The pictures are the usual copy's, byte for byte, under the same
      // content-hashed names, and the usual copy is built first into the
      // directory this one sits in (`npm run build`: build/ and build/sk/
      // hold build/basic/ and build/basic/sk/). Pointing the pages at its
      // assets/images/ instead of shipping a second set halves what the box
      // and Pages carry for them. Built alone, with no usual copy beside it,
      // the basic copy keeps its own.
      const usualOut = path.resolve(outDir, '..', ...(locale === '' ? [] : ['..', locale]));
      const images = path.join(outDir, 'assets/images');
      const shared = await sameFiles(images, path.join(usualOut, 'assets/images'));

      for (const rel of files) {
        const file = path.join(outDir, rel);
        let html = await fs.readFile(file, 'utf8');
        if (shared) html = html.replaceAll(`${plain}assets/images/`, `${usual}assets/images/`);
        // The Contents page is the one page the usual copy does not have.
        const own = rel === 'contents/index.html';
        html = withoutScripts(html)
          // The canonical address, its language alternates and og:url name
          // the usual copy: the two copies are one page to a search engine.
          .replace(/<(link|meta)\b[^>]*\b(rel="(canonical|alternate)"|property="og:url")[^>]*>/g, (tag) =>
            own
              ? tag
              : tag.replaceAll(`${context.siteConfig.url}${plain}`, `${context.siteConfig.url}${usual}`),
          );
        if (/<script\b/i.test(html) || /\bsrc="[^"]*\.js"/.test(html)) {
          throw new Error(`basic: ${rel} still loads a script`);
        }
        await fs.writeFile(file, html);
      }
      // What only a script would read: the bundles, the search page and its
      // index (the Contents page stands in for both), and the sitemap, whose
      // addresses are the usual copy's to list.
      await fs.rm(path.join(outDir, 'assets/js'), {recursive: true, force: true});
      if (shared) await fs.rm(images, {recursive: true, force: true});
      await fs.rm(path.join(outDir, 'search'), {recursive: true, force: true});
      await fs.rm(path.join(outDir, 'sitemap.xml'), {force: true});
      for (const name of await fs.readdir(outDir)) {
        if (/^search-index.*\.json$/.test(name)) await fs.rm(path.join(outDir, name));
      }
    },
  };
}
