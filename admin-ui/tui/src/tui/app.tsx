/* The full-screen view: the web UI's shell in a terminal.
 *
 *   header   the salmon, LosOS, the host's name and how it is doing
 *   sidebar  the panes, in the web UI's order; `/` searches them the way the
 *            web UI's sidebar search does (paneMatches)
 *   pane     the open pane's groups and rows; the cursor moves over the rows
 *   bar      the web UI's Apply bar: pending changes, or the rebuild running
 *   footer   the keys
 *
 * Settings changes stay a draft until Apply, as in the browser, and Apply asks
 * before it starts a rebuild. */

import * as React from "react";
import { Box, Text, useApp, useInput, type Key } from "ink";
import type { Transport } from "../api";
import { describeError } from "../api";
import { fold, t, type Locale } from "../lang";
import type { Theme } from "../theme";
import type { HostRecord } from "../vault";
import { BOX_PANES, runSearch, type BoxPane, type PaneCtx, type PaneDef, type SearchState } from "./box-panes";
import { EDGE_PANES, type EdgeCtx, type EdgePane } from "./edge-panes";
import { useSettingsForm, type Notice, type SettingsForm } from "./form";
import { Logo } from "./Logo";
import { layout, rowsOf, wrap, type ConfirmSpec, type Line, type PaneModel, type Row, type Span } from "./model";
import { useResources } from "./resources";
import { LineView, splitKeys, TextField, ThemeContext, useTheme } from "./ui";

export const SIDEBAR = 26;
export const HEADER = 5;

/** The web UI's paneMatches: every term must match the pane's name, summary
 *  or keywords in the language on screen or in English, accents folded. */
export function paneMatches(pane: PaneDef, search: string): boolean {
  const terms = fold(search)
    .split(/\s+/)
    .filter((term) => term.length > 0);
  if (terms.length === 0) return true;
  const haystack = fold(
    [pane.label(), pane.summary(), pane.keywords(), pane.englishLabel(), pane.englishSummary(), pane.englishKeywords.join(" ")].join(" "),
  );
  return terms.every((term) => haystack.includes(term));
}

/* State the key handler reads through a ref. Ink calls the handler once per
 * key, and keys typed or pasted faster than a render would otherwise all see
 * the state of the last render: "j" then Enter would open the pane the
 * cursor was on before the "j". */
type SetState<T> = (next: T | ((current: T) => T)) => void;

function useSyncState<T>(initial: T): [T, SetState<T>, React.MutableRefObject<T>] {
  const [value, setValue] = React.useState<T>(initial);
  const ref = React.useRef<T>(value);
  const set = React.useCallback<SetState<T>>((next) => {
    const resolved = typeof next === "function" ? (next as (current: T) => T)(ref.current) : next;
    ref.current = resolved;
    setValue(resolved);
  }, []);
  return [value, set, ref];
}

type Modal =
  | { kind: "confirm"; spec: ConfirmSpec; onConfirm: () => void }
  | { kind: "edit"; prompt: string; masked: boolean; initial: string; onSubmit: (value: string) => void }
  | { kind: "help" }
  | { kind: "busy"; text: string };

interface ShellProps {
  host: HostRecord;
  cols: number;
  rows: number;
  title: string;
  status: { text: string; tone: "ok" | "crit" | "muted" | "accent" };
  panes: readonly PaneDef[];
  build: (id: string, cols: number) => PaneModel;
  open: (id: string) => void;
  refresh: (id: string) => void;
  form?: SettingsForm;
  notice: Notice | null;
  setNotice: (notice: Notice | null) => void;
  initialPane?: string;
}

export function Shell(props: ShellProps) {
  const { panes, cols, rows, form, notice, setNotice } = props;
  const theme = useTheme();
  const app = useApp();
  const [paneId, setPaneId, paneRef] = useSyncState(props.initialPane ?? panes[0]?.id ?? "");
  const [focus, setFocus, focusRef] = useSyncState<"sidebar" | "pane">(props.initialPane === undefined ? "sidebar" : "pane");
  const [sideCursor, setSideCursor, sideRef] = useSyncState(Math.max(0, panes.findIndex((p) => p.id === paneId)));
  const [cursor, setCursor, cursorRef] = useSyncState(0);
  const [scroll, setScroll] = React.useState(0);
  const [search, setSearch, searchRef] = useSyncState<string | null>(null);
  const [modal, setModal, modalRef] = useSyncState<Modal | null>(null);

  const filtered = search === null ? panes : panes.filter((p) => paneMatches(p, search));

  React.useEffect(() => {
    props.open(paneId);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [paneId]);

  const barRows = form?.rebuild !== null && form?.rebuild !== undefined ? 2 : form?.dirty === true ? 2 : 0;
  const noticeRows = notice === null ? 0 : 1;
  const bodyRows = Math.max(6, rows - HEADER - 1 - 1 - barRows - noticeRows);
  const paneCols = Math.max(30, cols - SIDEBAR - 1);
  const innerCols = paneCols - 2;

  const model = props.build(paneId, innerCols);
  const paneRows = rowsOf(model);
  const words = { on: t("advanced.on"), off: t("advanced.off") };
  const laid = layout(model, innerCols, words, focus === "pane" ? cursor : null);

  // Keep the cursor's row in view.
  React.useEffect(() => {
    if (cursor >= laid.rowCount && laid.rowCount > 0) setCursor(laid.rowCount - 1);
    const first = laid.lines.findIndex((l) => l.row === cursor);
    let last = first;
    for (let i = laid.lines.length - 1; i >= 0; i -= 1) {
      if (laid.lines[i]?.row === cursor) {
        last = i;
        break;
      }
    }
    if (first < 0) return;
    if (first < scroll) setScroll(first === laid.lines.findIndex((l) => l.row !== undefined) ? 0 : first);
    else if (last >= scroll + bodyRows) setScroll(Math.min(first, last - bodyRows + 1));
  }, [cursor, laid, bodyRows, scroll]);

  const openPane = (id: string): void => {
    setPaneId(id);
    setCursor(0);
    setScroll(0);
  };

  const act = (row: Row | undefined): void => {
    const action = row?.action;
    if (row === undefined || action === undefined) return;
    if ("disabled" in action && action.disabled === true) return;
    switch (action.kind) {
      case "toggle":
        action.set(!action.on);
        return;
      case "choice": {
        const i = action.options.findIndex((o) => o.value === action.value);
        const next = action.options[(i + 1) % action.options.length];
        if (next !== undefined) action.set(next.value);
        return;
      }
      case "edit":
        setModal({
          kind: "edit",
          prompt: action.prompt,
          masked: action.masked === true,
          initial: action.masked === true ? "" : action.value,
          onSubmit: (value) => {
            setModal(null);
            void (async () => {
              try {
                const said = await action.submit(value);
                if (typeof said === "string") setNotice({ tone: "ok", title: said, text: "" });
              } catch (error) {
                setNotice({ tone: "crit", title: t("tui.failed"), text: describeError(error) });
              }
            })();
          },
        });
        return;
      case "run": {
        const go = (): void => {
          setModal({ kind: "busy", text: t("ui.working") });
          void (async () => {
            try {
              const said = await action.run();
              setModal(null);
              if (typeof said === "string") setNotice({ tone: "ok", title: said, text: "" });
            } catch (error) {
              setModal(null);
              setNotice({ tone: "crit", title: t("tui.failed"), text: describeError(error) });
            }
          })();
        };
        const confirm = action.confirm;
        if (confirm === undefined) {
          go();
          return;
        }
        if (typeof confirm === "function") {
          setModal({ kind: "busy", text: t("ui.working") });
          void (async () => {
            try {
              const spec = await confirm();
              setModal(spec === null ? null : { kind: "confirm", spec, onConfirm: go });
            } catch (error) {
              setModal(null);
              setNotice({ tone: "crit", title: t("tui.failed"), text: describeError(error) });
            }
          })();
          return;
        }
        setModal({ kind: "confirm", spec: confirm, onConfirm: go });
        return;
      }
    }
  };

  const askApply = (): void => {
    if (form === undefined || !form.dirty || form.rebuild !== null) return;
    if (!form.valid) {
      setNotice({ tone: "crit", title: t("settings.applyBar.invalid"), text: "" });
      return;
    }
    setModal({
      kind: "confirm",
      spec: {
        title: t("tui.apply.title"),
        body: [t("tui.apply.body", { count: form.changeCount }), t("settings.applyBar.valid")],
        confirm: t("settings.applyBar.apply"),
        cancel: t("tui.cancel"),
      },
      onConfirm: () => {
        setModal(null);
        void form.apply();
      },
    });
  };

  const onKey = (input: string, key: Key): void => {
      const modal = modalRef.current;
      const search = searchRef.current;
      const focus = focusRef.current;
      const sideCursor = sideRef.current;
      const cursor = cursorRef.current;
      const paneId = paneRef.current;
      const filtered = search === null ? panes : panes.filter((p) => paneMatches(p, search));
      if (notice !== null && !key.return) setNotice(null);
      if (modal !== null) {
        if (modal.kind === "edit" || modal.kind === "busy") return;
        if (modal.kind === "help") {
          setModal(null);
          return;
        }
        if (key.escape || input === "n") setModal(null);
        else if (key.return || input === "y") {
          const run = modal.onConfirm;
          setModal(null);
          run();
        }
        return;
      }
      if (search !== null) {
        if (key.escape) {
          setSearch(null);
          return;
        }
        if (key.return) {
          const target = filtered[Math.min(sideCursor, filtered.length - 1)];
          setSearch(null);
          if (target !== undefined) {
            setSideCursor(panes.indexOf(target));
            openPane(target.id);
            setFocus("pane");
          }
          return;
        }
        if (key.upArrow) setSideCursor((c) => Math.max(0, c - 1));
        else if (key.downArrow) setSideCursor((c) => Math.min(filtered.length - 1, c + 1));
        else if (key.backspace || key.delete) setSearch((s) => (s === null ? s : [...s].slice(0, -1).join("")));
        else if (!key.ctrl && !key.meta && input.length > 0 && !key.tab) {
          setSearch((s) => (s ?? "") + input);
          setSideCursor(0);
        }
        return;
      }
      if (input === "q") {
        app.exit();
        return;
      }
      if (input === "?") {
        setModal({ kind: "help" });
        return;
      }
      if (input === "/") {
        setSearch("");
        setSideCursor(0);
        setFocus("sidebar");
        return;
      }
      if (input === "r") {
        props.refresh(paneId);
        void form?.reload();
        setNotice({ tone: "accent", title: t("tui.refreshed"), text: "" });
        return;
      }
      if (input === "a") {
        askApply();
        return;
      }
      if (input === "d" && form !== undefined && form.dirty && form.rebuild === null) {
        form.discard();
        return;
      }
      if (key.tab) {
        setFocus((f) => (f === "sidebar" ? "pane" : "sidebar"));
        return;
      }
      const up = key.upArrow || input === "k";
      const down = key.downArrow || input === "j";
      if (focus === "sidebar") {
        if (up) setSideCursor((c) => Math.max(0, c - 1));
        else if (down) setSideCursor((c) => Math.min(panes.length - 1, c + 1));
        else if (key.return || key.rightArrow || input === "l" || input === " ") {
          const target = panes[sideCursor];
          if (target !== undefined) {
            openPane(target.id);
            setFocus("pane");
          }
        }
        return;
      }
      if (up) setCursor((c) => Math.max(0, c - 1));
      else if (down && cursor >= laid.rowCount - 1) {
        // Past the last row: scroll on to the captions and tables below it.
        setScroll((s) => Math.min(s + 1, Math.max(0, laid.lines.length - bodyRows)));
      } else if (down) setCursor((c) => Math.min(Math.max(0, laid.rowCount - 1), c + 1));
      else if (key.pageUp) setCursor((c) => Math.max(0, c - 8));
      else if (key.pageDown) setCursor((c) => Math.min(Math.max(0, laid.rowCount - 1), c + 8));
      else if (key.leftArrow || input === "h" || key.escape) setFocus("sidebar");
      else if (key.return || input === " ") act(paneRows[cursor]);
  };
  useInput(
    (input, key) => {
      for (const [one, k] of splitKeys(input, key)) {
        // An edit field opened by an earlier key in the same chunk takes the rest.
        if (modalRef.current?.kind === "edit") return;
        onKey(one, k);
      }
    },
    { isActive: modal?.kind !== "edit" },
  );

  // ── Drawing ──
  const sideLines: Line[] = [];
  if (search !== null) {
    sideLines.push({ spans: [{ text: " / ", color: "accent", bold: true }, { text: search, color: "ink" }, { text: "▏", color: "accent" }], bg: "sunk" });
  } else {
    sideLines.push({ spans: [{ text: ` / ${t("tui.searchPanes")}`, color: "faint" }], bg: "ground" });
  }
  sideLines.push({ spans: [], bg: "ground" });
  const shown = search === null ? panes : filtered;
  shown.forEach((pane, i) => {
    const current = pane.id === paneId;
    const pointed = (focus === "sidebar" || search !== null) && i === sideCursor;
    sideLines.push({
      spans: [
        { text: current ? " ▌" : "  ", color: "accent" },
        { text: pane.label(), color: current ? "accent" : pointed ? "ink" : "muted", bold: current || pointed },
      ],
      bg: pointed ? "accentWash" : current ? "sunk" : "ground",
    });
  });
  if (search !== null && filtered.length === 0) sideLines.push({ spans: [{ text: `  ${t("tui.noPaneMatches")}`, color: "faint" }], bg: "ground" });

  const visible = laid.lines.slice(scroll, scroll + bodyRows);
  const more = laid.lines.length > scroll + bodyRows;

  return (
    <Box flexDirection="column" width={cols}>
      <Header host={props.host} title={props.title} status={props.status} cols={cols} />
      <LineView line={{ spans: [{ text: "─".repeat(cols), color: "line" }], bg: "ground" }} cols={cols} />
      <Box flexDirection="row" height={bodyRows}>
        <Box flexDirection="column" width={SIDEBAR}>
          {Array.from({ length: bodyRows }, (_, i) => (
            <LineView key={i} line={sideLines[i] ?? { spans: [], bg: "ground" }} cols={SIDEBAR} />
          ))}
        </Box>
        <Box flexDirection="column" width={1}>
          {Array.from({ length: bodyRows }, (_, i) => (
            <Text key={i} color={theme.c.line} backgroundColor={theme.c.ground}>
              │
            </Text>
          ))}
        </Box>
        <Box flexDirection="column" width={paneCols}>
          {modal === null ? (
            Array.from({ length: bodyRows }, (_, i) => {
              const line = visible[i] ?? { spans: [], bg: "ground" as const };
              const scrollMark = i === bodyRows - 1 && more ? "↓" : " ";
              return (
                <Box key={i} flexDirection="row">
                  <LineView line={{ ...line, spans: [{ text: " ", bg: "ground" }, ...line.spans] }} cols={paneCols - 1} />
                  <Text color={theme.c.faint} backgroundColor={theme.c.ground}>
                    {scrollMark}
                  </Text>
                </Box>
              );
            })
          ) : (
            <ModalView modal={modal} cols={paneCols} rows={bodyRows} onCancel={() => setModal(null)} />
          )}
        </Box>
      </Box>
      {notice !== null && <NoticeLine notice={notice} cols={cols} />}
      {form !== undefined && barRows > 0 && <ApplyBar form={form} cols={cols} />}
      <Footer cols={cols} focus={focus} searching={search !== null} dirty={form?.dirty === true && form.rebuild === null} />
    </Box>
  );
}

function Header({ host, title, status, cols }: { host: HostRecord; title: string; status: ShellProps["status"]; cols: number }) {
  const theme = useTheme();
  const textCols = cols - 18;
  const lines: Span[][] = [
    [{ text: "LosOS", color: "ink", bold: true }, { text: `  ${t(host.kind === "edge" ? "tui.kind.edge" : "tui.kind.box")}`, color: "faint" }],
    [{ text: title, color: "ink", bold: true }],
    [{ text: host.url, color: "muted" }],
    [{ text: "● ", color: status.tone }, { text: status.text, color: status.tone === "muted" ? "muted" : "ink" }],
    [],
  ];
  return (
    <Box flexDirection="row">
      <Box width={1} flexDirection="column">
        {lines.map((_, i) => (
          <Text key={i} backgroundColor={theme.c.ground}>
            {" "}
          </Text>
        ))}
      </Box>
      <Logo size="small" theme={theme} />
      <Box flexDirection="column" width={textCols + 1}>
        {lines.map((spans, i) => (
          <LineView key={i} line={{ spans: [{ text: "  " }, ...spans], bg: "ground" }} cols={textCols + 1} />
        ))}
      </Box>
    </Box>
  );
}

function NoticeLine({ notice, cols }: { notice: Notice; cols: number }) {
  const tone = notice.tone === "ok" ? "ok" : notice.tone === "crit" ? "crit" : "accent";
  const text = notice.text.length > 0 ? `${notice.title}: ${notice.text}` : notice.title;
  return <LineView line={{ spans: [{ text: " ● ", color: tone }, { text, color: "ink" }], bg: "sunk" }} cols={cols} />;
}

function ApplyBar({ form, cols }: { form: SettingsForm; cols: number }) {
  if (form.rebuild !== null) {
    return (
      <Box flexDirection="column">
        <LineView line={{ spans: [{ text: " ◐ ", color: "accent", bold: true }, { text: form.rebuild.title, color: "ink", bold: true }], bg: "surface" }} cols={cols} />
        <LineView line={{ spans: [{ text: `   ${form.rebuild.message}`, color: "muted" }], bg: "surface" }} cols={cols} />
      </Box>
    );
  }
  return (
    <Box flexDirection="column">
      <LineView
        line={{
          spans: [
            { text: " ● ", color: "accent" },
            { text: t("settings.applyBar.pending", { count: form.changeCount }), color: "ink", bold: true },
          ],
          bg: "surface",
        }}
        cols={cols}
      />
      <LineView
        line={{
          spans: [
            { text: `   ${form.valid ? t("settings.applyBar.valid") : t("settings.applyBar.invalid")}`, color: form.valid ? "muted" : "crit" },
          ],
          bg: "surface",
        }}
        cols={cols}
      />
    </Box>
  );
}

function Footer({ cols, focus, searching, dirty }: { cols: number; focus: "sidebar" | "pane"; searching: boolean; dirty: boolean }) {
  const keys: [string, string][] = searching
    ? [
        ["↑↓", t("tui.keys.move")],
        ["Enter", t("tui.keys.open")],
        ["Esc", t("tui.keys.clear")],
      ]
    : [
        ["↑↓ j k", t("tui.keys.move")],
        ["Enter", focus === "sidebar" ? t("tui.keys.open") : t("tui.keys.change")],
        ["Tab", t("tui.keys.switch")],
        ["/", t("tui.keys.search")],
        ...(dirty
          ? ([
              ["a", t("settings.applyBar.apply")],
              ["d", t("settings.applyBar.discard")],
            ] as [string, string][])
          : []),
        ["r", t("tui.keys.refresh")],
        ["?", t("tui.keys.help")],
        ["q", t("tui.keys.quit")],
      ];
  const spans: Span[] = [{ text: " " }];
  for (const [k, what] of keys) spans.push({ text: ` ${k} `, color: "ink", bg: "sunk", bold: true }, { text: ` ${what}  `, color: "muted" });
  return <LineView line={{ spans, bg: "ground" }} cols={cols} />;
}

function ModalView({ modal, cols, rows, onCancel }: { modal: Modal; cols: number; rows: number; onCancel: () => void }) {
  const width = Math.min(cols - 4, 70);
  const pad = Math.max(0, Math.floor((cols - width) / 2));
  const card = (spans: Span[], bg: "surface" | "sunk" = "surface"): Line => ({ spans: [{ text: "  " }, ...spans], bg });
  const lines: Line[] = [];
  let field: React.ReactNode = null;
  if (modal.kind === "help") {
    lines.push(card([{ text: t("tui.help.title"), color: "ink", bold: true }]));
    lines.push(card([]));
    const keys: [string, string][] = [
      ["↑ ↓  j k", t("tui.help.move")],
      ["Enter  Space", t("tui.help.enter")],
      ["Tab", t("tui.help.tab")],
      ["← h  Esc", t("tui.help.back")],
      ["/", t("tui.help.search")],
      ["a  d", t("tui.help.apply")],
      ["r", t("tui.help.refresh")],
      ["PgUp PgDn", t("tui.help.page")],
      ["q  Ctrl-C", t("tui.help.quit")],
    ];
    for (const [k, what] of keys) lines.push(card([{ text: k.padEnd(14), color: "accent", bold: true }, { text: what, color: "muted" }]));
    lines.push(card([]));
    lines.push(card([{ text: t("tui.help.close"), color: "faint" }]));
  } else if (modal.kind === "busy") {
    lines.push(card([{ text: `◐ ${modal.text}`, color: "accent" }]));
  } else if (modal.kind === "confirm") {
    lines.push(card([{ text: modal.spec.title, color: "ink", bold: true }]));
    lines.push(card([]));
    for (const paragraph of modal.spec.body) {
      for (const text of wrap(paragraph, width - 4)) lines.push(card([{ text, color: "muted" }]));
      lines.push(card([]));
    }
    lines.push(
      card([
        { text: ` Enter `, color: "ground", bg: "accent", bold: true },
        { text: ` ${modal.spec.confirm}    `, color: "ink" },
        { text: ` Esc `, color: "ink", bg: "sunk", bold: true },
        { text: ` ${modal.spec.cancel}`, color: "muted" },
      ]),
    );
  } else {
    for (const text of wrap(modal.prompt, width - 4)) lines.push(card([{ text, color: "ink", bold: true }]));
    lines.push(card([]));
    field = (
      <Box flexDirection="row">
        <Text>{" ".repeat(pad)}</Text>
        <LineView line={card([])} cols={2} />
        <TextField masked={modal.masked} initial={modal.initial} onSubmit={modal.onSubmit} onCancel={onCancel} />
      </Box>
    );
  }
  const after: Line[] =
    modal.kind === "edit"
      ? [card([]), card([{ text: t(modal.masked ? "tui.edit.hintMasked" : "tui.edit.hint"), color: "faint" }])]
      : [];
  const top = 1;
  const used = top + lines.length + (field === null ? 0 : 1) + after.length;
  return (
    <ThemeContext.Consumer>
      {(theme) => (
        <Box flexDirection="column">
          <Text backgroundColor={theme?.c.ground}>{" ".repeat(cols)}</Text>
          {lines.map((line, i) => (
            <Box key={i} flexDirection="row">
              <Text backgroundColor={theme?.c.ground}>{" ".repeat(pad)}</Text>
              <LineView line={line} cols={width} />
              <Text backgroundColor={theme?.c.ground}>{" ".repeat(Math.max(0, cols - pad - width))}</Text>
            </Box>
          ))}
          {field}
          {after.map((line, i) => (
            <Box key={`a${i}`} flexDirection="row">
              <Text backgroundColor={theme?.c.ground}>{" ".repeat(pad)}</Text>
              <LineView line={line} cols={width} />
              <Text backgroundColor={theme?.c.ground}>{" ".repeat(Math.max(0, cols - pad - width))}</Text>
            </Box>
          ))}
          {Array.from({ length: Math.max(0, rows - used) }, (_, i) => (
            <Text key={`f${i}`} backgroundColor={theme?.c.ground}>
              {" ".repeat(cols)}
            </Text>
          ))}
        </Box>
      )}
    </ThemeContext.Consumer>
  );
}

// ── The two apps ─────────────────────────────────────────────────────────

export interface AppProps {
  host: HostRecord;
  transport: Transport;
  theme: Theme;
  locale: Locale;
  cols: number;
  rows: number;
  initialPane?: string;
  /** For screenshots and tests: start with this search state. */
  initialSearch?: SearchState;
}

function useNotice(): [Notice | null, (n: Notice | null) => void] {
  const [notice, setNotice] = React.useState<Notice | null>(null);
  const timer = React.useRef<ReturnType<typeof setTimeout> | null>(null);
  const set = React.useCallback((n: Notice | null) => {
    if (timer.current !== null) clearTimeout(timer.current);
    setNotice(n);
    if (n !== null) {
      timer.current = setTimeout(() => setNotice(null), 8000);
      timer.current.unref?.();
    }
  }, []);
  React.useEffect(() => () => {
    if (timer.current !== null) clearTimeout(timer.current);
  }, []);
  return [notice, set];
}

export function BoxApp(props: AppProps) {
  const [notice, setNotice] = useNotice();
  const form = useSettingsForm(props.transport, setNotice);
  const res = useResources(props.transport);
  const [search, setSearch] = React.useState<SearchState>(props.initialSearch ?? { kind: "idle" });
  const ctx: PaneCtx = { host: props.host, transport: props.transport, form, res, search, setSearch, say: setNotice };
  React.useEffect(() => {
    res.want(["/api/health", "/api/status"]);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  const health = res.get<{ ok: boolean }>("/api/health");
  const status =
    form.rebuild !== null
      ? { text: form.rebuild.title, tone: "accent" as const }
      : health.state === "ok" && health.data.ok
        ? { text: t("home.header.answering"), tone: "ok" as const }
        : health.state === "loading"
          ? { text: t("home.header.checking"), tone: "muted" as const }
          : { text: t("home.header.notAnswering"), tone: "crit" as const };
  const title = form.saved?.hostName ?? new URL(props.host.url).hostname;
  const byId = (id: string): BoxPane => BOX_PANES.find((p) => p.id === id) ?? BOX_PANES[0]!;
  return (
    <ThemeContext.Provider value={props.theme}>
      <Shell
        host={props.host}
        cols={props.cols}
        rows={props.rows}
        title={title}
        status={status}
        panes={BOX_PANES}
        build={(id, cols) => byId(id).buildAt(ctx, cols)}
        open={(id) => res.want(byId(id).needs)}
        refresh={(id) => res.reload(["/api/health", "/api/status", ...byId(id).needs])}
        form={form}
        notice={notice}
        setNotice={setNotice}
        {...(props.initialPane === undefined ? {} : { initialPane: props.initialPane })}
      />
    </ThemeContext.Provider>
  );
}

export function EdgeApp(props: AppProps) {
  const [notice, setNotice] = useNotice();
  const res = useResources(props.transport);
  const ctx: EdgeCtx = { host: props.host, transport: props.transport, res };
  // The header's status comes from /overview, whichever pane opens first.
  React.useEffect(() => {
    res.want(["/overview"]);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  const overview = res.get<unknown>("/overview");
  const status =
    overview.state === "ok"
      ? { text: t("home.header.answering"), tone: "ok" as const }
      : overview.state === "loading"
        ? { text: t("home.header.checking"), tone: "muted" as const }
        : { text: t("home.header.notAnswering"), tone: "crit" as const };
  const byId = (id: string): EdgePane => EDGE_PANES.find((p) => p.id === id) ?? EDGE_PANES[0]!;
  return (
    <ThemeContext.Provider value={props.theme}>
      <Shell
        host={props.host}
        cols={props.cols}
        rows={props.rows}
        title={new URL(props.host.url).hostname}
        status={status}
        panes={EDGE_PANES}
        build={(id, cols) => byId(id).buildEdge(ctx, cols)}
        open={(id) => res.want(byId(id).needs)}
        refresh={(id) => res.reload(byId(id).needs)}
        notice={notice}
        setNotice={setNotice}
        {...(props.initialPane === undefined ? {} : { initialPane: props.initialPane })}
      />
    </ThemeContext.Provider>
  );
}

export { runSearch };
