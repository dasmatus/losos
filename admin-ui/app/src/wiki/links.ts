/* LosOS Wiki's names and links, shared by the editing screen in the admin
 * UI (src/wikis/) and a space's published site (src/wiki/site.tsx).
 *
 * A space is a LosOS Git repository's wiki (backend/src/forge/wiki.rs): a
 * page is one Markdown file, `Getting-started.md` for "Getting started",
 * and a page below another is a file in a folder, `Guides/Setup.md`. */

/** The name a page gets from its title, as the box makes it (wiki.rs
 *  `name_for`). */
export function wikiName(title: string): string {
  return title
    .trim()
    .replace(/\s+/gu, "-")
    .replace(/[/\\:*?"<>|#%\p{Cc}]/gu, "")
    .slice(0, 120)
    .replace(/^[-.]+|[-.]+$/gu, "");
}

/** "Getting started" from `Guides/Getting-started`. */
export function titleOf(name: string): string {
  return (name.split("/").pop() ?? name).replaceAll("-", " ");
}

/** A page's address below `base` (`/wiki/<owner>/<space>`), parts escaped. */
export function pageHref(base: string, name: string): string {
  return name === "" ? `${base}/` : `${base}/${name.split("/").map(encodeURIComponent).join("/")}`;
}

/** The page an address names below `base`, or "" for the space's home. */
export function pageAt(base: string, pathname: string): string {
  if (!pathname.startsWith(`${base}/`)) return "";
  return pathname
    .slice(base.length + 1)
    .split("/")
    .filter((s) => s !== "")
    .map((s) => {
      try {
        return decodeURIComponent(s);
      } catch {
        return s;
      }
    })
    .join("/");
}

/** `[[Page]]`, `[[Page|label]]` and links to `Page` or `Page.md` lead to
 *  the space's own pages, as on Forgejo. The Markdown renderer follows
 *  absolute links only, so they are made absolute below `base`. A link
 *  inside a page in a folder starts from that folder. */
export function wikiLinks(text: string, base: string, from = "", raw?: (file: string) => string): string {
  const folder = from.includes("/") ? from.slice(0, from.lastIndexOf("/")) : "";
  // A picture kept in the wiki, `![](diagram.png)`, comes from its file.
  const file = (target: string) => {
    const parts: string[] = target.startsWith("/") || folder === "" ? [] : folder.split("/");
    for (const part of target.split("/")) {
      if (part === "" || part === ".") continue;
      if (part === "..") parts.pop();
      else parts.push(part);
    }
    return raw === undefined ? target : raw(parts.join("/"));
  };
  const to = (target: string) => {
    const bare = target.replace(/\.(md|markdown)$/iu, "");
    const parts: string[] = bare.startsWith("/") || folder === "" ? [] : folder.split("/");
    for (const part of bare.split("/")) {
      if (part === "" || part === ".") continue;
      if (part === "..") parts.pop();
      else parts.push(wikiName(part));
    }
    return pageHref(base, parts.join("/"));
  };
  return text
    .replace(/!\[([^\]]*)\]\((?!https?:|data:|#|\/)([^)\s]+)\)/gu, (_, alt: string, target: string) => {
      let decoded = target;
      try {
        decoded = decodeURIComponent(target);
      } catch {
        // as written
      }
      return `![${alt}](${file(decoded)})`;
    })
    .replace(/\[\[([^\]|]+?)(?:\|([^\]]+))?\]\]/gu, (_, target: string, label?: string) => `[${label ?? target}](${to(target.trim())})`)
    .replace(/(?<!!\[[^\]]*)\]\((?!https?:|mailto:|\/|#)([^)\s]+)\)/gu, (_, target: string) => {
      let decoded = target;
      try {
        decoded = decodeURIComponent(target);
      } catch {
        // as written
      }
      return `](${to(decoded)})`;
    });
}

/** One page in the tree, and the pages below it. */
export interface PageNode {
  /** The page's full name, or a folder's that has no page of its own. */
  name: string;
  title: string;
  /** False for a folder that holds pages but is none. */
  page: boolean;
  children: PageNode[];
}

/** The pages as a tree: `Guides/Setup` below `Guides`, which is a page if
 *  `Guides.md` is there and a plain folder otherwise. Home first, then by
 *  title. */
export function pageTree(names: readonly string[]): PageNode[] {
  const top: PageNode[] = [];
  const all = new Map<string, PageNode>();
  const node = (name: string): PageNode => {
    const found = all.get(name.toLowerCase());
    if (found !== undefined) return found;
    const made: PageNode = { name, title: titleOf(name), page: false, children: [] };
    all.set(name.toLowerCase(), made);
    const at = name.lastIndexOf("/");
    (at < 0 ? top : node(name.slice(0, at)).children).push(made);
    return made;
  };
  for (const name of names) node(name).page = true;
  const order = (list: PageNode[]) => {
    list.sort((a, b) =>
      a.name.toLowerCase() === "home" ? -1 : b.name.toLowerCase() === "home" ? 1 : a.title.localeCompare(b.title),
    );
    for (const n of list) order(n.children);
  };
  order(top);
  return top;
}
