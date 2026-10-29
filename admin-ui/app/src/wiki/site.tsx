/* A space of LosOS Wiki as a site of its own, at /wiki/<owner>/<space>/
 * like a project's GitHub Pages: the page tree on the left, the page, and a
 * search of the space. It only reads; whoever may write is sent to the admin
 * UI's /wikis to edit. /wiki/ alone lists every space this reader may open.
 *
 * nginx serves the document to anyone (modules/containers.nix), or behind
 * LosOS Login with `git.site.requireSignIn`; LosOS Git's API decides what
 * each reader sees, so a private space answers 404 to a stranger as if it
 * were not there, and the page offers to sign in. */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { LibraryIcon, PencilEdit02Icon, Search01Icon, SidebarLeftIcon } from "@hugeicons/core-free-icons";
import { Button, buttonVariants } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group";
import { DisplayMenu } from "@/components/preferences-menu";
import { Spinner } from "@/components/ui/spinner";
import { git, GitError, type Wiki, type WikiHit, type WikiPage, type WikiSpace } from "@/git/api";
import { useLocale, useT } from "@/lib/i18n-react";
import { logoFor } from "@/lib/logo";
import { cn } from "@/lib/utils";
import { pageAt, pageHref } from "./links";
import { PageTree, useInPageLinks, WikiArticle } from "./parts";

const ROOT = "/wiki";

/** The space an address names, and its base. */
function spaceAt(pathname: string): { owner: string; repo: string; base: string } | null {
  const [owner, repo] = pathname.slice(ROOT.length + 1).split("/");
  if (owner === undefined || owner === "" || repo === undefined || repo === "") return null;
  const decode = (s: string) => {
    try {
      return decodeURIComponent(s);
    } catch {
      return s;
    }
  };
  return { owner: decode(owner), repo: decode(repo), base: `${ROOT}/${owner}/${repo}` };
}

function useAddress(): [string, (url: string) => void] {
  const [path, setPath] = React.useState(() => window.location.pathname);
  React.useEffect(() => {
    const pop = () => setPath(window.location.pathname);
    window.addEventListener("popstate", pop);
    return () => window.removeEventListener("popstate", pop);
  }, []);
  const go = React.useCallback((url: string) => {
    window.history.pushState(null, "", url);
    setPath(window.location.pathname);
    if (window.location.hash === "") window.scrollTo({ top: 0 });
  }, []);
  return [path, go];
}

function signIn() {
  window.location.assign(`${window.location.origin}/auth/?rd=${encodeURIComponent(window.location.href)}`);
}

function useLoaded<T>(load: (signal: AbortSignal) => Promise<T>, deps: React.DependencyList) {
  const [state, setState] = React.useState<{ data: T | null; error: unknown }>({ data: null, error: null });
  React.useEffect(() => {
    const controller = new AbortController();
    setState({ data: null, error: null });
    load(controller.signal).then(
      (data) => !controller.signal.aborted && setState({ data, error: null }),
      (error: unknown) => !controller.signal.aborted && setState({ data: null, error }),
    );
    return () => controller.abort();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
  return state;
}

export function WikiSite() {
  useLocale();
  const [path, go] = useAddress();
  const space = spaceAt(path);
  const wiki = useLoaded((signal) => (space === null ? Promise.resolve(null) : git.wiki(space.owner, space.repo, signal)), [space?.base]);
  const me = useLoaded((signal) => git.me(signal).catch(() => null), []);

  React.useEffect(() => {
    document.title = space === null ? "LosOS Wiki" : `${space.repo} · LosOS Wiki`;
  }, [space]);

  return (
    <div className="losos-shell min-h-dvh text-ink" data-wiki-site-root>
      <Header title={space?.repo ?? ""} home={space?.base ?? ROOT} go={go} signedIn={me.data !== null} />
      {space === null ? (
        <Directory go={go} />
      ) : wiki.error !== null ? (
        <Missing error={wiki.error} signedIn={me.data !== null} />
      ) : wiki.data === null ? (
        <Loading />
      ) : (
        <Space owner={space.owner} repo={space.repo} base={space.base} wiki={wiki.data} path={path} go={go} />
      )}
    </div>
  );
}

function Header({ title, home, go, signedIn }: { title: string; home: string; go: (url: string) => void; signedIn: boolean }) {
  const t = useT();
  const logo = logoFor();
  return (
    <header className="sticky top-0 z-40 border-b border-line bg-surface/85 backdrop-blur-sm">
      <div className="mx-auto flex w-full max-w-[96rem] items-center gap-3 px-4 py-3 sm:px-6">
        <a
          href={`${home}/`}
          onClick={(event) => {
            if (event.button !== 0 || event.metaKey || event.ctrlKey) return;
            event.preventDefault();
            go(`${home}/`);
          }}
          className="flex min-w-0 items-center gap-2.5 rounded-control focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
        >
          <img src={logo.url} alt="" width={32} height={32} className="size-8" />
          <span className="truncate text-[15px] font-semibold tracking-tight" data-wiki-site-title>
            {title || t("wiki.site.title")}
          </span>
        </a>
        <div className="flex-1" />
        {!signedIn && (
          <Button variant="ghost" size="sm" onClick={signIn} data-wiki-sign-in>
            {t("wiki.site.signIn")}
          </Button>
        )}
        <DisplayMenu />
      </div>
    </header>
  );
}

function Loading() {
  const t = useT();
  return (
    <div className="flex min-h-72 items-center justify-center gap-2 text-[13px] text-muted" role="status">
      <Spinner size={16} />
      {t("wiki.site.loading")}
    </div>
  );
}

function Missing({ error, signedIn }: { error: unknown; signedIn: boolean }) {
  const t = useT();
  const gone = error instanceof GitError && (error.status === 404 || error.status === 401 || error.status === 403);
  return (
    <Empty className="mx-auto mt-10 min-h-72 max-w-xl" data-wiki-missing>
      <EmptyHeader>
        <EmptyTitle>{t(gone ? "wiki.site.notFound.title" : "wiki.site.offline.title")}</EmptyTitle>
        <EmptyDescription>
          {gone ? t(signedIn ? "wiki.site.notFound.text" : "wiki.site.notFound.signIn") : t("wiki.site.offline.text")}
        </EmptyDescription>
      </EmptyHeader>
      {gone && !signedIn && <Button onClick={signIn}>{t("wiki.site.signIn")}</Button>}
    </Empty>
  );
}

// ── /wiki/: every space this reader may open ──────────────────────────────

function Directory({ go }: { go: (url: string) => void }) {
  const t = useT();
  const list = useLoaded((signal) => git.wikis("", signal), []);
  const spaces: WikiSpace[] = (list.data?.wikis ?? []).filter((s) => s.pages > 0);
  return (
    <main className="mx-auto flex w-full max-w-3xl flex-col gap-4 px-4 py-8 sm:px-6" data-wiki-directory>
      <div className="flex flex-col gap-1">
        <h1 className="text-[22px] font-semibold tracking-tight">{t("wiki.site.title")}</h1>
        <p className="text-[13.5px] text-muted">{t("wiki.site.summary")}</p>
      </div>
      {list.error !== null ? (
        <Missing error={list.error} signedIn />
      ) : list.data === null ? (
        <Loading />
      ) : spaces.length === 0 ? (
        <p className="text-[13.5px] text-muted">{t("wiki.site.none")}</p>
      ) : (
        <ul className="grid gap-2 sm:grid-cols-2">
          {spaces.map((s) => (
            <li key={s.full_name}>
              <a
                href={`${s.html_url}/`}
                onClick={(event) => {
                  if (event.button !== 0 || event.metaKey || event.ctrlKey) return;
                  event.preventDefault();
                  go(`${s.html_url}/`);
                }}
                className="flex h-full flex-col gap-1 rounded-card border border-line bg-surface px-4 py-3 transition-colors hover:border-accent/50 focus-visible:outline-2 focus-visible:outline-accent"
                data-wiki-space={s.full_name}
              >
                <span className="flex items-center gap-2 text-[14px] font-medium">
                  <HugeiconsIcon icon={LibraryIcon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                  {s.repo}
                </span>
                {s.description !== "" && <span className="line-clamp-2 text-[13px] text-muted">{s.description}</span>}
                <span className="text-[12px] text-faint">
                  {s.owner} · {t("wiki.pages", { count: s.pages })}
                </span>
              </a>
            </li>
          ))}
        </ul>
      )}
    </main>
  );
}

// ── /wiki/<owner>/<space>/: one space ──────────────────────────────────────

function Space({ owner, repo, base, wiki, path, go }: { owner: string; repo: string; base: string; wiki: Wiki; path: string; go: (url: string) => void }) {
  const t = useT();
  const asked = pageAt(base, path);
  const name = asked !== "" ? asked : (wiki.home ?? "");
  const page = useLoaded((signal) => (name === "" ? Promise.resolve(null) : git.wikiPage(owner, repo, name, signal)), [owner, repo, name]);
  const [tree, setTree] = React.useState(false);
  const [query, setQuery] = React.useState("");
  const [search, setSearch] = React.useState("");
  const names = React.useMemo(() => wiki.pages.map((p) => p.name), [wiki]);
  const open = (n: string) => {
    setTree(false);
    setSearch("");
    go(pageHref(base, n));
  };

  return (
    <div className="mx-auto flex w-full max-w-[96rem] gap-6 px-4 py-6 sm:px-6">
      <aside
        className={cn(
          "w-64 shrink-0 flex-col gap-3 max-lg:fixed max-lg:inset-y-0 max-lg:left-0 max-lg:z-50 max-lg:w-72 max-lg:overflow-y-auto max-lg:border-r max-lg:border-line max-lg:bg-surface max-lg:p-4 lg:sticky lg:top-20 lg:flex lg:max-h-[calc(100dvh-6rem)] lg:self-start lg:overflow-y-auto",
          tree ? "flex" : "hidden",
        )}
        aria-label={t("wiki.pagesOf", { space: repo })}
      >
        <form
          role="search"
          onSubmit={(event) => {
            event.preventDefault();
            setSearch(query.trim());
            setTree(false);
          }}
        >
          <InputGroup>
            <InputGroupAddon>
              <HugeiconsIcon icon={Search01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            </InputGroupAddon>
            <InputGroupInput
              type="search"
              value={query}
              onChange={(event) => {
                setQuery(event.target.value);
                if (event.target.value === "") setSearch("");
              }}
              placeholder={t("wiki.site.search")}
              aria-label={t("wiki.site.search")}
              data-wiki-site-search
            />
          </InputGroup>
        </form>
        <PageTree names={names} current={page.data?.name ?? name} base={base} onOpen={open} label={t("wiki.pagesOf", { space: repo })} />
        <p className="mt-auto pt-3 text-[12px] text-faint">{owner}</p>
      </aside>
      {tree && <button type="button" aria-hidden="true" tabIndex={-1} className="fixed inset-0 z-40 bg-ink/20 lg:hidden" onClick={() => setTree(false)} />}

      <main className="min-w-0 flex-1">
        <div className="mb-3 flex items-center gap-2 lg:hidden">
          <Button variant="secondary" size="sm" onClick={() => setTree(true)} aria-expanded={tree} data-wiki-site-pages>
            <HugeiconsIcon icon={SidebarLeftIcon} size={15} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            {t("wiki.site.pages")}
          </Button>
        </div>
        <div className="rounded-card border border-line bg-surface">
          {search !== "" ? (
            <Results owner={owner} repo={repo} q={search} base={base} go={go} onOpen={open} />
          ) : !wiki.exists || name === "" ? (
            <Empty className="m-4 border-0">
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <HugeiconsIcon icon={LibraryIcon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
                </EmptyMedia>
                <EmptyTitle>{t("wiki.empty.title")}</EmptyTitle>
                <EmptyDescription>{t(wiki.writable ? "wiki.site.emptyWriter" : "wiki.empty.reader")}</EmptyDescription>
              </EmptyHeader>
              {wiki.writable && (
                <a
                  href={`/wikis/${encodeURIComponent(owner)}/${encodeURIComponent(repo)}/`}
                  className={buttonVariants({ variant: "default" })}
                  data-wiki-site-edit
                >
                  {t("wiki.empty.first")}
                </a>
              )}
            </Empty>
          ) : page.error !== null ? (
            <Empty className="m-4 border-0">
              <EmptyHeader>
                <EmptyTitle>{t("wiki.page.missing")}</EmptyTitle>
                <EmptyDescription>{t("wiki.page.missingText")}</EmptyDescription>
              </EmptyHeader>
            </Empty>
          ) : page.data === null ? (
            <Loading />
          ) : (
            <WikiArticle
              owner={owner}
              repo={repo}
              page={page.data}
              base={base}
              go={go}
              actions={wiki.writable ? <EditLink owner={owner} repo={repo} page={page.data} /> : undefined}
            />
          )}
        </div>
      </main>
    </div>
  );
}

function EditLink({ owner, repo, page }: { owner: string; repo: string; page: WikiPage }) {
  const t = useT();
  return (
    <a
      href={pageHref(`/wikis/${encodeURIComponent(owner)}/${encodeURIComponent(repo)}`, page.name)}
      className={buttonVariants({ variant: "secondary", size: "sm" })}
      data-wiki-site-edit
    >
      <HugeiconsIcon icon={PencilEdit02Icon} size={15} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
      {t("wiki.site.edit")}
    </a>
  );
}

function Results({
  owner,
  repo,
  q,
  base,
  go,
  onOpen,
}: {
  owner: string;
  repo: string;
  q: string;
  base: string;
  go: (url: string) => void;
  onOpen: (name: string) => void;
}) {
  const t = useT();
  const links = useInPageLinks(base, go);
  const hits = useLoaded((signal) => git.wikiSearch(owner, repo, q, signal), [owner, repo, q]);
  const list: WikiHit[] = hits.data ?? [];
  return (
    <section ref={links} className="flex flex-col" aria-label={t("wiki.results", { query: q })} data-wiki-site-results>
      <h1 className="border-b border-hair px-5 py-3 text-[17px] font-semibold tracking-tight">{t("wiki.results", { query: q })}</h1>
      {hits.data === null ? (
        <Loading />
      ) : list.length === 0 ? (
        <p className="px-5 py-4 text-[13.5px] text-muted">{t("wiki.noResults")}</p>
      ) : (
        <ul className="flex flex-col p-2">
          {list.map((h, i) => (
            <li key={`${h.page}-${i}`}>
              <button
                type="button"
                onClick={() => onOpen(h.page)}
                className="block w-full rounded-control px-3 py-2 text-left hover:bg-sunk focus-visible:ring-2 focus-visible:ring-accent/40 focus-visible:outline-none"
                data-hit={h.page}
              >
                <span className="block text-[14px] font-medium text-ink">{h.title}</span>
                {h.text !== null && <span className="line-clamp-2 text-[13px] text-muted">{h.text}</span>}
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
