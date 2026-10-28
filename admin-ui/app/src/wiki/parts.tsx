/* The pieces of LosOS Wiki both its screens draw: the page tree, a page
 * as it reads, and the editor. The admin UI's /wikis edits a space; the
 * published site at /wiki/<owner>/<space>/ (./site.tsx) only reads it. */

import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { ArrowRight01Icon, File01Icon, Folder01Icon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { Dialog, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Field, FieldDescription, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { git, GitError, type WikiPage } from "@/git/api";
import { when, whenFull } from "@/git/format";
import { Markdown } from "@/git/markdown";
import { pageHref, pageTree, wikiLinks, wikiName, type PageNode } from "./links";

/** Clicks on links below `base` move inside the page instead of loading
 *  a new one. */
export function useInPageLinks(base: string, go: (href: string) => void): React.RefCallback<HTMLElement> {
  const goRef = React.useRef(go);
  goRef.current = go;
  return React.useCallback(
    (el: HTMLElement | null) => {
      if (el === null) return;
      el.addEventListener("click", (event) => {
        if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.defaultPrevented) return;
        const a = (event.target as Element | null)?.closest?.("a");
        const href = a?.getAttribute("href") ?? "";
        if (a === null || a === undefined || a.target === "_blank" || !href.startsWith(`${base}/`)) return;
        event.preventDefault();
        goRef.current(href);
      });
    },
    [base],
  );
}

/** The space's pages, nested as their names are. */
export function PageTree({
  names,
  current,
  base,
  onOpen,
  label,
}: {
  names: readonly string[];
  current: string;
  base: string;
  onOpen: (name: string) => void;
  label: string;
}) {
  const tree = React.useMemo(() => pageTree(names), [names]);
  return (
    <ul className="flex flex-col gap-px" aria-label={label} data-page-tree>
      {tree.map((n) => (
        <TreeNode key={n.name} node={n} depth={0} current={current} base={base} onOpen={onOpen} />
      ))}
    </ul>
  );
}

function TreeNode({
  node,
  depth,
  current,
  base,
  onOpen,
}: {
  node: PageNode;
  depth: number;
  current: string;
  base: string;
  onOpen: (name: string) => void;
}) {
  const below = current.toLowerCase().startsWith(`${node.name.toLowerCase()}/`);
  const [open, setOpen] = React.useState(true);
  React.useEffect(() => {
    if (below) setOpen(true);
  }, [below]);
  const isCurrent = current.toLowerCase() === node.name.toLowerCase();
  const pad = { paddingInlineStart: `${0.25 + depth * 0.875}rem` };
  return (
    <li>
      <div className="flex items-center">
        {node.children.length > 0 ? (
          <button
            type="button"
            onClick={() => setOpen(!open)}
            aria-expanded={open}
            aria-label={node.title}
            className="grid size-6 shrink-0 place-items-center rounded-control text-faint hover:text-ink"
            // A CSSOM write, which the admin CSP allows.
            style={{ marginInlineStart: `${depth * 0.875}rem` }}
          >
            <HugeiconsIcon
              icon={ArrowRight01Icon}
              size={13}
              strokeWidth={1.5}
              color="currentColor"
              aria-hidden="true"
              className={cn("transition-transform duration-150 motion-reduce:transition-none", open && "rotate-90")}
            />
          </button>
        ) : null}
        {node.page ? (
          <a
            href={pageHref(base, node.name)}
            aria-current={isCurrent ? "page" : undefined}
            onClick={(event) => {
              if (event.button !== 0 || event.metaKey || event.ctrlKey) return;
              event.preventDefault();
              onOpen(node.name);
            }}
            style={node.children.length > 0 ? undefined : { ...pad, paddingInlineStart: `${1.75 + depth * 0.875}rem` }}
            className={cn(
              "flex min-w-0 flex-1 items-center gap-2 rounded-control py-1.5 pr-2 text-left text-[13px] outline-none transition-colors duration-150",
              "focus-visible:ring-2 focus-visible:ring-accent/40",
              node.children.length > 0 && "pl-1",
              isCurrent ? "bg-accent-wash font-medium text-ink" : "text-muted hover:bg-sunk hover:text-ink",
            )}
            data-page={node.name}
          >
            <HugeiconsIcon icon={File01Icon} size={14} strokeWidth={1.5} color="currentColor" aria-hidden="true" className="shrink-0" />
            <span className="truncate">{node.title}</span>
          </a>
        ) : (
          <span className="flex min-w-0 flex-1 items-center gap-2 py-1.5 pr-2 pl-1 text-[13px] text-muted">
            <HugeiconsIcon icon={Folder01Icon} size={14} strokeWidth={1.5} color="currentColor" aria-hidden="true" className="shrink-0" />
            <span className="truncate">{node.title}</span>
          </span>
        )}
      </div>
      {open && node.children.length > 0 && (
        <ul className="flex flex-col gap-px">
          {node.children.map((c) => (
            <TreeNode key={c.name} node={c} depth={depth + 1} current={current} base={base} onOpen={onOpen} />
          ))}
        </ul>
      )}
    </li>
  );
}

/** A page as it reads: its title, who changed it last, the text, and the
 *  space's sidebar and footer pages around it. */
export function WikiArticle({
  owner,
  repo,
  page,
  base,
  go,
  actions,
  history = false,
}: {
  owner: string;
  repo: string;
  page: WikiPage;
  base: string;
  go: (href: string) => void;
  actions?: React.ReactNode;
  /** List the page's changes below it. */
  history?: boolean;
}) {
  const t = useT();
  const links = useInPageLinks(base, go);
  const md = (text: string, from = page.name) => wikiLinks(text, base, from, (file) => git.wikiFile(owner, repo, file));
  return (
    <div ref={links} className="@container flex min-w-0 flex-col" data-wiki-page={page.name}>
      <header className="flex flex-wrap items-center gap-x-3 gap-y-1 border-b border-hair px-5 py-3">
        <h1 className="min-w-0 flex-1 text-[19px] font-semibold tracking-tight break-words">{page.title}</h1>
        {actions}
        {page.updated !== null && (
          <p className="w-full text-[12.5px] text-muted" title={whenFull(page.updated.date)}>
            {t("wiki.page.updated", { who: page.updated.author, when: when(page.updated.date) })}
          </p>
        )}
      </header>
      {/* The sidebar beside the text where the article is wide enough,
          below it where it is not: the article's own width decides, so the
          three columns of /wikis and the wide site both read well. */}
      <div className="flex gap-6 px-5 py-4 @max-2xl:flex-col">
        <Markdown source={md(page.text)} className="min-w-0 flex-1" />
        {page.sidebar !== null && (
          <aside className="w-52 shrink-0 self-start rounded-control border border-line bg-sunk px-3 py-2 @max-2xl:w-full" data-wiki-sidebar>
            <Markdown source={md(page.sidebar, page.sidebar_page ?? "")} className="text-[13px]" />
          </aside>
        )}
      </div>
      {page.footer !== null && (
        <footer className="border-t border-hair px-5 py-3" data-wiki-footer>
          <Markdown source={md(page.footer, page.footer_page ?? "")} className="text-[12.5px] text-muted" />
        </footer>
      )}
      {history && page.history.length > 0 && (
        <section className="border-t border-hair px-5 py-3" aria-label={t("wiki.page.history")} data-wiki-history>
          <h2 className="pb-2 text-[13px] font-semibold text-muted">{t("wiki.page.history")}</h2>
          <ol className="flex flex-col gap-1">
            {page.history.map((c) => (
              <li key={c.sha} className="flex flex-wrap items-baseline gap-x-2 text-[12.5px]">
                <span className="text-ink">{c.subject}</span>
                <span className="text-muted" title={whenFull(c.date)}>
                  {t("wiki.page.change", { who: c.author, when: when(c.date) })}
                </span>
              </li>
            ))}
          </ol>
        </section>
      )}
    </div>
  );
}

/** Write a page: a new one under `parent` (its title makes its name), or
 *  `page` as it was opened. */
export function WikiEditor({
  owner,
  repo,
  page,
  parent = "",
  initialTitle = "",
  onSaved,
  onCancel,
}: {
  owner: string;
  repo: string;
  page: WikiPage | null;
  parent?: string;
  initialTitle?: string;
  onSaved: (page: WikiPage) => void;
  onCancel: () => void;
}) {
  const t = useT();
  const id = React.useId();
  const [title, setTitle] = React.useState(initialTitle);
  const [text, setText] = React.useState(page?.text ?? "");
  const [message, setMessage] = React.useState("");
  const [tab, setTab] = React.useState("write");
  const [error, setError] = React.useState<string | null>(null);
  const [busy, setBusy] = React.useState(false);
  const creating = page === null;
  const own = wikiName(title);
  const target = creating ? (own === "" ? "" : parent === "" ? own : `${parent}/${own}`) : page.name;

  const save = (event: React.FormEvent) => {
    event.preventDefault();
    if (target === "") return;
    setBusy(true);
    setError(null);
    git.saveWikiPage(owner, repo, target, text, page?.sha ?? null, message.trim()).then(onSaved, (err: unknown) => {
      setBusy(false);
      setError(
        err instanceof GitError && err.status === 409
          ? t("wiki.edit.conflict")
          : err instanceof GitError && err.status === 403
            ? t("wiki.edit.forbidden")
            : t("wiki.edit.failed", { detail: err instanceof Error ? err.message : String(err) }),
      );
    });
  };

  return (
    <form onSubmit={save} className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-5 py-4" data-wiki-editor>
      <h1 className="text-[17px] font-semibold tracking-tight">
        {creating ? t("wiki.edit.newTitle") : t("wiki.edit.title", { title: page.title })}
      </h1>
      {creating && (
        <Field>
          <FieldLabel htmlFor={`${id}-title`}>{t("wiki.edit.pageTitle")}</FieldLabel>
          <Input id={`${id}-title`} value={title} onChange={(e) => setTitle(e.target.value)} required autoFocus aria-describedby={`${id}-file`} />
          <FieldDescription id={`${id}-file`}>
            {target === "" ? t("wiki.edit.titleHint") : t("wiki.edit.file", { file: `${target}.md` })}
          </FieldDescription>
        </Field>
      )}
      <Tabs value={tab} onValueChange={setTab} className="flex flex-col gap-3">
        <TabsList className="w-fit">
          <TabsTrigger value="write">{t("wiki.edit.write")}</TabsTrigger>
          <TabsTrigger value="preview">{t("wiki.edit.preview")}</TabsTrigger>
        </TabsList>
        <TabsContent value="write">
          <FieldLabel htmlFor={`${id}-text`} className="sr-only">
            {t("wiki.edit.text")}
          </FieldLabel>
          <Textarea
            id={`${id}-text`}
            value={text}
            onChange={(e) => setText(e.target.value)}
            className="min-h-72 font-mono text-[13px]"
            placeholder={t("wiki.edit.placeholder")}
          />
        </TabsContent>
        <TabsContent value="preview" className="min-h-72 rounded-control border border-line px-4 py-3">
          {text.trim() === "" ? (
            <p className="text-[13px] text-muted">{t("wiki.edit.nothing")}</p>
          ) : (
            <Markdown source={wikiLinks(text, `/wikis/${encodeURIComponent(owner)}/${encodeURIComponent(repo)}`, target, (file) => git.wikiFile(owner, repo, file))} />
          )}
        </TabsContent>
      </Tabs>
      <Field>
        <FieldLabel htmlFor={`${id}-message`}>{t("wiki.edit.message")}</FieldLabel>
        <Input id={`${id}-message`} value={message} onChange={(e) => setMessage(e.target.value)} placeholder={t("wiki.edit.messageHint")} />
      </Field>
      {error !== null && (
        <p role="alert" className="text-[13px] text-crit">
          {error}
        </p>
      )}
      <div className="flex justify-end gap-2">
        <Button type="button" variant="secondary" onClick={onCancel}>
          {t("wiki.cancel")}
        </Button>
        <Button type="submit" disabled={busy || target === ""}>
          {t("wiki.edit.save")}
        </Button>
      </div>
    </form>
  );
}

export function DeletePageDialog({ title, onClose, onDelete }: { title: string; onClose: () => void; onDelete: () => Promise<void> }) {
  const t = useT();
  const id = React.useId();
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  return (
    <Dialog open onOpenChange={(open) => !open && onClose()} labelledBy={`${id}-t`} dialogClassName="w-[min(28rem,calc(100vw-2rem))]">
      <DialogHeader>
        <DialogTitle id={`${id}-t`}>{t("wiki.delete.title", { title })}</DialogTitle>
        <DialogDescription>{t("wiki.delete.body")}</DialogDescription>
      </DialogHeader>
      {error !== null && (
        <p role="alert" className="px-6 text-[13px] text-crit">
          {error}
        </p>
      )}
      <DialogFooter>
        <Button variant="secondary" onClick={onClose}>
          {t("wiki.cancel")}
        </Button>
        <Button
          variant="destructive"
          disabled={busy}
          onClick={() => {
            setBusy(true);
            onDelete().catch((err: unknown) => {
              setBusy(false);
              setError(err instanceof Error ? err.message : String(err));
            });
          }}
        >
          {t("wiki.delete.confirm")}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}
