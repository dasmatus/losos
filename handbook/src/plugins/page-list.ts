import type {Plugin} from '@docusaurus/types';
import type {LoadedContent, DocMetadata} from '@docusaurus/plugin-content-docs';

/* Every page of the handbook in sidebar order, grouped by chapter, as global
 * data for the search page (src/theme/SearchPage). The offline search runs
 * in the browser, so without JavaScript that page lists the handbook
 * instead, and the browser's own Find searches the list. Runs at build time
 * in Node.js, once the docs plugin has loaded every locale's pages. */

export type PageListEntry = {title: string; permalink: string; depth: number};
/* A chapter: its heading links to the chapter's own page, and a run of
 * top-level pages outside any chapter is a group with no label. */
export type PageListGroup = {
  label: string | null;
  permalink: string | null;
  pages: PageListEntry[];
};
export type PageList = {groups: PageListGroup[]};

type Item = {
  type: string;
  id?: string;
  label?: string;
  items?: Item[];
  link?: {type: 'doc'; id: string} | {type: 'generated-index'; permalink: string};
};

export default function pageList(): Plugin {
  return {
    name: 'hb-page-list',
    allContentLoaded({allContent, actions}) {
      const docs = allContent['docusaurus-plugin-content-docs']?.['default'] as
        | LoadedContent
        | undefined;
      const version = docs?.loadedVersions[0];
      if (version === undefined) return;
      const byId = new Map<string, DocMetadata>(version.docs.map((doc) => [doc.id, doc]));
      const page = (id: string, label: string | undefined, depth: number): PageListEntry[] => {
        const doc = byId.get(id);
        return doc === undefined
          ? []
          : [{title: label ?? doc.frontMatter.sidebar_label ?? doc.title, permalink: doc.permalink, depth}];
      };
      const link = (item: Item, depth: number): PageListEntry[] =>
        item.link?.type === 'doc'
          ? page(item.link.id, item.label, depth)
          : item.link?.type === 'generated-index'
            ? [{title: item.label ?? '', permalink: item.link.permalink, depth}]
            : [];
      const walk = (items: Item[], depth: number): PageListEntry[] =>
        items.flatMap((item) => {
          if (item.type === 'doc' && item.id !== undefined) return page(item.id, item.label, depth);
          if (item.type !== 'category') return [];
          return [...link(item, depth), ...walk(item.items ?? [], depth + 1)];
        });
      const groups: PageListGroup[] = [];
      const sidebar = (Object.values(version.sidebars)[0] ?? []) as Item[];
      for (const item of sidebar) {
        if (item.type === 'category') {
          groups.push({
            label: item.label ?? null,
            permalink: link(item, 0)[0]?.permalink ?? null,
            pages: walk(item.items ?? [], 0),
          });
          continue;
        }
        const last = groups[groups.length - 1];
        const pages = walk([item], 0);
        if (last !== undefined && last.label === null) last.pages.push(...pages);
        else groups.push({label: null, permalink: null, pages});
      }
      actions.setGlobalData({groups} satisfies PageList);
    },
  };
}
