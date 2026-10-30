/* The editor an administrator changes a page or the pages' stylesheet in.
 *
 * It docks beside the page rather than over it, because the page IS the
 * preview: each keystroke is read by the same losos-mdx the box reads a
 * save with (src/mdx/wasm.ts), and the HTML goes into the store as a draft
 * (./store.ts), which the page under it draws in place of what is saved.
 * So what the panel shows is what a save will show, problems included, and
 * the box refuses a save the panel already said does not read.
 *
 * The stylesheet is previewed the same way: a constructed sheet adopted by
 * the document while the panel is open, with the saved one switched off.
 *
 * Opened from the Look pane (screens/settings/pane-look.tsx), by
 * administrators only; lososd refuses everyone else anyway. */

import * as React from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import { Cancel01Icon } from "@hugeicons/core-free-icons";
import { CodeEditor } from "@/components/code-editor";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import { toast } from "@/components/ui/toast";
import {
  getPageSource,
  getPagesStyle,
  putPage,
  putPagesStyle,
  resetPage,
  resetPagesStyle,
  type PageSource,
} from "@/lib/api";
import { useT } from "@/lib/i18n-react";
import type { MessageKey } from "@/lib/i18n";
import { loadMdx, renderPage, type Problem } from "@/mdx/wasm";
import blocks from "./blocks.json";
import type { PageId } from "./page";
import { closePageEditor, setDraft, setStyleDraft, takePages, type EditorTarget } from "./store";

const REGISTRY = JSON.stringify(blocks);

interface PropSpec {
  type: string;
  required?: boolean;
  values?: string[];
}

interface BlockSpec {
  props?: Record<string, PropSpec>;
}

/** The page's name in the language on screen. */
export function pageName(t: ReturnType<typeof useT>, id: PageId): string {
  return t(`pages.page.${id}` as MessageKey);
}

export function PageEditorDock({ target }: { target: EditorTarget }) {
  return target.kind === "page" ? (
    <PageSourceEditor key={target.id} id={target.id as PageId} />
  ) : (
    <StyleEditor />
  );
}

/* The panel both editors are drawn in: a column on the right from md up,
 * a sheet over the lower half of a phone. App.tsx leaves the room for it. */
function Dock({
  title,
  children,
  footer,
}: {
  title: string;
  children: React.ReactNode;
  footer: React.ReactNode;
}) {
  const t = useT();
  return (
    <aside
      aria-label={title}
      data-testid="page-editor"
      className={
        "fixed inset-x-0 bottom-0 z-40 flex h-[55dvh] flex-col border-t border-line bg-surface " +
        "shadow-[0_-8px_32px_rgb(0_0_0/0.12)] md:inset-y-0 md:left-auto md:h-auto md:w-[30rem] " +
        "md:border-t-0 md:border-l md:shadow-[-8px_0_32px_rgb(0_0_0/0.10)]"
      }
    >
      <header className="flex items-center gap-2 border-b border-line px-4 py-3">
        <h2 className="min-w-0 flex-1 truncate text-[15px] font-semibold">{title}</h2>
        <Button
          variant="ghost"
          size="icon"
          className="size-8"
          aria-label={t("pages.editor.close")}
          title={t("pages.editor.close")}
          onClick={() => closePageEditor()}
        >
          <HugeiconsIcon icon={Cancel01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
        </Button>
      </header>
      <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-4 py-3">{children}</div>
      <footer className="flex flex-wrap items-center justify-end gap-2 border-t border-line px-4 py-3">
        {footer}
      </footer>
    </aside>
  );
}

function Problems({ problems }: { problems: readonly Problem[] }) {
  const t = useT();
  if (problems.length === 0) return null;
  return (
    <Alert variant="crit" data-testid="page-problems">
      <AlertTitle>{t("pages.editor.problems")}</AlertTitle>
      <AlertDescription>
        <ul className="list-none space-y-1 p-0">
          {problems.map((p, i) => (
            <li key={i}>
              {p.place === null
                ? p.message
                : t("pages.editor.problem", { line: p.place.line, column: p.place.column, message: p.message })}
            </li>
          ))}
        </ul>
      </AlertDescription>
    </Alert>
  );
}

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

// ── A page ────────────────────────────────────────────────────────────────

function PageSourceEditor({ id }: { id: PageId }) {
  const t = useT();
  const name = pageName(t, id);
  const spec = blocks.pages[id];
  const navigate = useNavigate();
  const location = useLocation();

  // The page itself is the preview, so the editor opens over it.
  const path = spec.path;
  const navigated = React.useRef(false);
  React.useEffect(() => {
    if (navigated.current) return;
    navigated.current = true;
    if (location.pathname !== path) navigate(path);
  }, [location.pathname, navigate, path]);

  const [source, setSource] = React.useState<PageSource | null>(null);
  const [failed, setFailed] = React.useState<string | null>(null);
  const [text, setText] = React.useState("");
  const [problems, setProblems] = React.useState<Problem[]>([]);
  const [busy, setBusy] = React.useState(false);
  const [parser, setParser] = React.useState(false);

  const load = React.useCallback(async (): Promise<void> => {
    try {
      const answer = await getPageSource(id);
      setSource(answer);
      setText(answer.source);
      setFailed(null);
    } catch (error) {
      setFailed(errorText(error));
    }
  }, [id]);

  React.useEffect(() => {
    void load();
    void loadMdx().then(() => setParser(true));
  }, [load]);

  // Each change is read as the box will read it, a moment after typing stops.
  React.useEffect(() => {
    if (!parser || source === null) return;
    const timer = setTimeout(() => {
      try {
        const rendered = renderPage(text, REGISTRY, id);
        setProblems(rendered.problems);
        setDraft(id, rendered.html);
      } catch (error) {
        setProblems([{ place: null, message: errorText(error) }]);
      }
    }, 120);
    return () => clearTimeout(timer);
  }, [parser, source, text, id]);

  const save = async (): Promise<void> => {
    setBusy(true);
    try {
      takePages(await putPage(id, text));
      await load();
      toast.success(t("pages.editor.saved"));
    } catch (error) {
      toast.error(t("pages.editor.notSaved"), errorText(error), { help: "pages" });
    } finally {
      setBusy(false);
    }
  };

  const reset = async (): Promise<void> => {
    setBusy(true);
    try {
      takePages(await resetPage(id));
      setDraft(id, null);
      await load();
      toast.success(t("pages.editor.resetDone"));
    } catch (error) {
      toast.error(t("pages.editor.notSaved"), errorText(error), { help: "pages" });
    } finally {
      setBusy(false);
    }
  };

  const blockList = Object.entries(spec.blocks as Record<string, BlockSpec>);

  return (
    <Dock
      title={t("pages.editor.title", { name })}
      footer={
        <>
          {source?.custom === true && (
            <Button variant="ghost" size="sm" disabled={busy} onClick={() => void reset()}>
              {t("pages.editor.reset")}
            </Button>
          )}
          <Button
            size="sm"
            disabled={busy || source === null || problems.length > 0 || text === source.source}
            onClick={() => void save()}
          >
            {busy ? t("pages.editor.saving") : t("pages.editor.save")}
          </Button>
        </>
      }
    >
      <p className="text-[13px] leading-snug text-muted">{t("pages.editor.preview")}</p>
      {failed !== null && (
        <Alert variant="crit">
          <AlertDescription>{t("pages.editor.notLoaded", { message: failed })}</AlertDescription>
        </Alert>
      )}
      {source?.defaultChanged === true && (
        <Alert variant="warn" data-testid="page-default-changed">
          <AlertTitle>{t("pages.editor.defaultChanged.title")}</AlertTitle>
          <AlertDescription>
            <p>{t("pages.editor.defaultChanged.detail")}</p>
            <Button variant="secondary" size="sm" className="mt-2" onClick={() => setText(source.default)}>
              {t("pages.editor.defaultChanged.open")}
            </Button>
          </AlertDescription>
        </Alert>
      )}
      {source === null && failed === null ? (
        <Spinner label={t("pages.editor.loading")} className="text-muted" />
      ) : (
        source !== null && (
          <CodeEditor
            value={text}
            onChange={setText}
            language="mdx"
            label={t("pages.editor.label", { name })}
            invalid={problems.length > 0}
            className="h-64 shrink-0 rounded-md border border-line md:h-80"
          />
        )
      )}
      <Problems problems={problems} />
      <details className="rounded-md border border-line px-3 py-2" open>
        <summary className="cursor-pointer text-[13px] font-medium">{t("pages.editor.blocks")}</summary>
        <ul className="mt-2 list-none space-y-2 p-0 text-[13px]">
          {blockList.map(([block, about]) => (
            <li key={block}>
              <code className="font-mono text-ink">{`<${block} />`}</code>
              <span className="text-muted"> {t(`pages.block.${block}` as MessageKey)}</span>
              {Object.entries(about.props ?? {}).map(([prop, p]) => (
                <div key={prop} className="ml-3 text-muted">
                  <code className="font-mono text-ink">{prop}</code>
                  {p.type === "flag" ? ` (${t("pages.editor.flag")})` : ""}{" "}
                  {t(`pages.block.${block}.${prop}` as MessageKey)}
                </div>
              ))}
            </li>
          ))}
        </ul>
        <p className="mt-2 text-[12px] leading-snug text-muted">{t("pages.editor.blocksCaption")}</p>
      </details>
      {source !== null && (
        <details className="rounded-md border border-line px-3 py-2">
          <summary className="cursor-pointer text-[13px] font-medium">{t("pages.editor.history")}</summary>
          {source.history.length === 0 ? (
            <p className="mt-2 text-[13px] text-muted">{t("pages.editor.historyEmpty")}</p>
          ) : (
            <ol className="mt-2 list-none space-y-1.5 p-0 text-[13px]" data-testid="page-history">
              {source.history.map((c) => (
                <li key={c.commit}>
                  <span className="text-ink">{c.message}</span>
                  <span className="block text-muted">
                    {t("pages.editor.historyBy", { by: c.by, at: new Date(c.at).toLocaleString() })}
                  </span>
                </li>
              ))}
            </ol>
          )}
        </details>
      )}
    </Dock>
  );
}

// ── The stylesheet ────────────────────────────────────────────────────────

function StyleEditor() {
  const t = useT();
  const [saved, setSaved] = React.useState<string | null>(null);
  const [failed, setFailed] = React.useState<string | null>(null);
  const [text, setText] = React.useState("");
  const [busy, setBusy] = React.useState(false);

  const load = React.useCallback(async (): Promise<void> => {
    try {
      const css = await getPagesStyle();
      setSaved(css);
      setText(css);
      setFailed(null);
    } catch (error) {
      setFailed(errorText(error));
    }
  }, []);

  React.useEffect(() => {
    void load();
  }, [load]);

  React.useEffect(() => {
    if (saved === null) return;
    const timer = setTimeout(() => setStyleDraft(text), 120);
    return () => clearTimeout(timer);
  }, [saved, text]);

  const save = async (): Promise<void> => {
    setBusy(true);
    try {
      takePages(await putPagesStyle(text));
      await load();
      toast.success(t("pages.editor.styleSaved"));
    } catch (error) {
      toast.error(t("pages.editor.notSaved"), errorText(error), { help: "pages" });
    } finally {
      setBusy(false);
    }
  };

  const reset = async (): Promise<void> => {
    setBusy(true);
    try {
      takePages(await resetPagesStyle());
      await load();
      toast.success(t("pages.editor.styleResetDone"));
    } catch (error) {
      toast.error(t("pages.editor.notSaved"), errorText(error), { help: "pages" });
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dock
      title={t("pages.editor.styleTitle")}
      footer={
        <>
          {saved !== null && saved !== "" && (
            <Button variant="ghost" size="sm" disabled={busy} onClick={() => void reset()}>
              {t("pages.editor.styleReset")}
            </Button>
          )}
          <Button size="sm" disabled={busy || saved === null || text === saved} onClick={() => void save()}>
            {busy ? t("pages.editor.saving") : t("pages.editor.save")}
          </Button>
        </>
      }
    >
      <p className="text-[13px] leading-snug text-muted">{t("pages.editor.stylePreview")}</p>
      {failed !== null && (
        <Alert variant="crit">
          <AlertDescription>{t("pages.editor.notLoaded", { message: failed })}</AlertDescription>
        </Alert>
      )}
      {saved === null && failed === null ? (
        <Spinner label={t("pages.editor.loading")} className="text-muted" />
      ) : (
        saved !== null && (
          <CodeEditor
            value={text}
            onChange={setText}
            language="css"
            label={t("pages.editor.styleLabel")}
            className="min-h-64 flex-1 rounded-md border border-line"
          />
        )
      )}
    </Dock>
  );
}
