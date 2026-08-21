# losos Design System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract a token-driven design system (`design-system/tokens.css` + `losos.css`) that admin-ui consumes with zero build step, plus a dev-machine-only React wrapper package for claude.ai/design.

**Architecture:** CSS-first. Two plain stylesheets become the single source of visual truth, unified to the dashboard's Forgejo flavor; the three existing per-page stylesheets are deleted and both pages link the shared ones via a new nginx `/ds/` alias. A thin React package (`design-system/react`) renders the exact same class names for the design agent; it never enters the Nix closure.

**Tech Stack:** Plain CSS (custom properties, `color-mix()`), NixOS modules/nginx, TypeScript + React 19 + esbuild (dev machine only).

**Spec:** `docs/superpowers/specs/2026-08-21-design-system-design.md`

## Global Constraints

- The appliance stays dependency-free: no node/npm anywhere in the Nix closure; `design-system/react` is built only by hand on a dev machine.
- Class names are preserved exactly, with ONE rename: `.open` → `.btn-outline` (4 anchors in `dashboard/index.html`; verified referenced by no JS).
- JS-coupled names that must survive verbatim: `.side-item`, `.is-active`, `.section`, `.content`, `data-state` (`up|down|checking`), `data-kind` (`building|done|failed`), `data-section`.
- Dashboard renders pixel-identical after migration (its values ARE the tokens); settings re-skins per spec. Dashboard-only literals that stay: `.chip` 12.5px, `.banner-close` 18px, focus-visible radius 4px.
- All color literals live in `tokens.css` only; `losos.css` contains **zero** hex colors (rgba()/color-mix() allowed). Alpha tints use `color-mix(in srgb, var(--ls-*) N%, transparent)` with the original percentages (12/10/8/60/45) — fine for any 2023+ browser.
- Token prefix is `--ls-*`. No spacing tokens (deliberate YAGNI).
- `system.stateVersion` and everything unrelated (backend, daemon, page JS behavior) untouched. Note: six files are already staged in the index with in-progress installer work (`backend/src/Installer.hs`, `backend/test/Spec.hs`, `flake.nix`, `modules/{boot,disko,installer}.nix`) — never `git add -A`; stage files explicitly, and don't touch those six.
- Every commit message ends with:
  `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>` and
  `Claude-Session: https://claude.ai/code/session_01XL7m53JPidukfkj43LEKrv`

---

### Task 1: Token stylesheet

**Files:**
- Create: `design-system/tokens.css`

**Interfaces:**
- Produces: the `--ls-*` custom properties every later rule uses.

- [ ] **Step 1: Write the file** — exact content:

```css
/* losos design tokens — the single source of the visual vocabulary.
 * Every color literal in the design system lives here; losos.css and
 * consumers reference only var(--ls-*). Forgejo-flavored (see
 * docs/superpowers/specs/2026-08-21-design-system-design.md). */
:root {
  /* ── Color ─────────────────────────────────────────────────────── */
  --ls-bg: #f5f7fa;              /* page background */
  --ls-surface: #ffffff;         /* cards, groups, inputs */
  --ls-surface-muted: #f0f2f5;   /* chip bg, hover fills */
  --ls-ink: #1c2128;             /* text; also the topbar background */
  --ls-ink-muted: #57606a;       /* secondary text */
  --ls-border: #d0d5dd;
  --ls-border-faint: #eef1f4;    /* hairline row separators */
  --ls-accent: #1f6feb;          /* links, focus, primary actions */
  --ls-accent-strong: #1a5fd0;   /* primary hover */
  --ls-success: #2ecc71;
  --ls-success-tint: #e9f9ef;
  --ls-danger: #e5534b;
  --ls-danger-tint: #fdeceb;
  --ls-danger-deep: #8a2b25;     /* error banner text */
  --ls-warning: #d4a012;         /* building state */
  --ls-warning-tint: #fff8e6;
  --ls-topbar-link: #d0d5dd;     /* idle topnav links */
  --ls-neutral: #8b949e;         /* status dot "checking"/unknown */
  --ls-control-off: #e9e9eb;     /* switch track when off */

  /* ── Shape ─────────────────────────────────────────────────────── */
  --ls-radius-sm: 6px;           /* chips, inputs, small controls */
  --ls-radius-md: 8px;           /* cards, banners, buttons */
  --ls-radius-lg: 12px;          /* auth modal only */
  --ls-radius-pill: 999px;

  /* ── Type ──────────────────────────────────────────────────────── */
  --ls-font: system-ui, -apple-system, "Segoe UI", Roboto, sans-serif;
  --ls-font-mono: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace;
  --ls-fs-xs: 12px;
  --ls-fs-sm: 13.5px;
  --ls-fs-md: 14px;
  --ls-fs-base: 15px;
  --ls-fs-lg: 16px;
  --ls-fs-xl: 17px;
  --ls-fs-2xl: 20px;

  /* ── Depth ─────────────────────────────────────────────────────── */
  --ls-shadow-sm: 0 4px 14px rgba(27, 33, 40, 0.10);
  --ls-shadow-lg: 0 12px 40px rgba(0, 0, 0, 0.22);
  --ls-shadow-knob: 0 2px 4px rgba(0, 0, 0, 0.22);
}
```

- [ ] **Step 2: Verify** — Run: `grep -c -- '--ls-' design-system/tokens.css` → Expected: `35` (19 color + 4 radius + 9 type + 3 shadow definition lines; if it differs, recount by eye against the spec's token table).

- [ ] **Step 3: Commit**

```bash
git add design-system/tokens.css
git commit -m "feat(design-system): add token stylesheet"
```

### Task 2: Unified component stylesheet

**Files:**
- Create: `design-system/losos.css`

**Interfaces:**
- Consumes: `tokens.css` variables (loaded by pages as a separate `<link>`, so no `@import` here).
- Produces: every component class both pages and the React layer use.

- [ ] **Step 1: Write the file** — exact content (merged from `admin-ui/common.css`, `admin-ui/dashboard/style.css`, `admin-ui/settings/style.css`; Forgejo wins all conflicts; `.open` renamed `.btn-outline`; the two `.error-banner` blocks merged with pinned font-size):

```css
/* losos design system — every component class for the admin UI pages
 * and the React wrapper layer. Colors only via var(--ls-*) from
 * tokens.css (link tokens.css before this file). No hex literals here. */

/* ── Base ─────────────────────────────────────────────────────────── */
* { box-sizing: border-box; }

html { -webkit-text-size-adjust: 100%; }

body {
  margin: 0;
  background: var(--ls-bg);
  color: var(--ls-ink);
  font-family: var(--ls-font);
  font-size: var(--ls-fs-base);
  line-height: 1.45;
}

a { color: var(--ls-accent); text-decoration: none; }
a:hover { text-decoration: underline; }

code {
  font-family: var(--ls-font-mono);
  font-size: 0.92em;
}

:focus-visible {
  outline: 2px solid var(--ls-accent);
  outline-offset: 2px;
  border-radius: 4px;
}

/* ── Top navigation bar ───────────────────────────────────────────── */
.topbar {
  display: flex;
  align-items: center;
  gap: 16px;
  height: 48px;
  padding: 0 20px;
  background: var(--ls-ink);
  color: var(--ls-surface);
}
.topbar .brand {
  color: var(--ls-surface);
  font-weight: 700;
  font-size: var(--ls-fs-xl);
  letter-spacing: 0.2px;
  text-decoration: none;
}
.topnav {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 4px;
}
.topnav a {
  color: var(--ls-topbar-link);
  text-decoration: none;
  font-size: var(--ls-fs-md);
  padding: 6px 10px;
  border-radius: var(--ls-radius-sm);
}
.topnav a:hover { color: var(--ls-surface); background: rgba(255, 255, 255, 0.08); text-decoration: none; }
.topbar a:focus-visible { outline-color: var(--ls-surface); }

/* ── Indeterminate progress bar (visible while building) ──────────── */
.indbar {
  margin-top: 10px;
  height: 4px;
  border-radius: 2px;
  background: rgba(0, 0, 0, 0.08);
  overflow: hidden;
  display: none;
}
.banner[data-kind="building"] .indbar, .progress-card[data-kind="building"] .indbar { display: block; }
.indbar span {
  display: block;
  width: 30%;
  height: 100%;
  border-radius: 2px;
  background: var(--ls-warning);
  animation: indslide 1.2s ease-in-out infinite;
}
@keyframes indslide {
  0% { transform: translateX(-100%); }
  50% { transform: translateX(180%); }
  100% { transform: translateX(360%); }
}

/* ── Page shells ──────────────────────────────────────────────────── */
.container {
  max-width: 1100px;
  margin: 0 auto;
  padding: 20px 16px 48px;
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: 16px;
  margin-top: 16px;
}

.layout {
  display: flex;
  align-items: flex-start;
  gap: 24px;
  max-width: 880px;
  margin: 0 auto;
  padding: 24px 16px 56px;
}

.sidebar {
  flex: none;
  width: 240px;
  background: var(--ls-surface);
  border: 1px solid var(--ls-border);
  border-radius: var(--ls-radius-md);
  overflow: hidden;
}
.side-list { display: flex; flex-direction: column; }
.side-item {
  display: block;
  padding: 9px 14px;
  color: var(--ls-ink);
  text-decoration: none;
  border-top: 0.5px solid var(--ls-border);
  font-size: var(--ls-fs-md);
}
.side-item:first-child { border-top: 0; }
.side-item:hover { background: var(--ls-surface-muted); text-decoration: none; }
.side-item.is-active {
  background: color-mix(in srgb, var(--ls-accent) 12%, transparent);
  color: var(--ls-accent);
  font-weight: 600;
}
.side-item--danger { color: var(--ls-danger); }
.side-item--danger.is-active {
  background: color-mix(in srgb, var(--ls-danger) 10%, transparent);
  color: var(--ls-danger);
}

.content {
  flex: 1;
  min-width: 0;
  max-width: 560px;
}

.content-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 12px;
}
.content-header h1 {
  margin: 0;
  font-size: var(--ls-fs-2xl);
  font-weight: 700;
}

.section { display: none; }
.section.is-active { display: block; }

/* ── Cards ────────────────────────────────────────────────────────── */
.card {
  background: var(--ls-surface);
  border: 1px solid var(--ls-border);
  border-radius: var(--ls-radius-md);
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  transition: box-shadow 0.15s ease;
}
.card:hover { box-shadow: var(--ls-shadow-sm); }

.card-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.card-head h2 {
  margin: 0;
  font-size: var(--ls-fs-lg);
  font-weight: 600;
}

.status {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: var(--ls-fs-xs);
  color: var(--ls-ink-muted);
}
.dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: var(--ls-neutral); /* checking */
  flex: none;
}
.dot[data-state="up"] { background: var(--ls-success); }
.dot[data-state="down"] { background: var(--ls-danger); }
.dot[data-state="checking"] { background: var(--ls-neutral); }

.desc {
  margin: 0;
  color: var(--ls-ink-muted);
  font-size: var(--ls-fs-sm);
  flex: 1;
}

.chip-row { margin: 0; }
.chip {
  display: inline-block;
  font-family: var(--ls-font-mono);
  font-size: 12.5px; /* dashboard pixel-identity; mono sizing, not on the scale */
  background: var(--ls-surface-muted);
  border: 1px solid var(--ls-border);
  border-radius: var(--ls-radius-sm);
  padding: 2px 8px;
  color: var(--ls-ink);
}

.hint-card {
  background: var(--ls-surface);
  border: 1px solid var(--ls-border);
  border-radius: var(--ls-radius-md);
  padding: 10px 16px;
  margin-bottom: 4px;
  color: var(--ls-ink-muted);
  font-size: var(--ls-fs-sm);
}
.hint-card p { margin: 0; }

.syslist { margin: 0; }
.sysrow {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  padding: 6px 0;
  border-top: 1px solid var(--ls-border-faint);
}
.sysrow:first-child { border-top: 0; }
.sysrow dt {
  color: var(--ls-ink-muted);
  font-size: var(--ls-fs-sm);
}
.sysrow dd { margin: 0; }

/* ── Grouped settings rows ────────────────────────────────────────── */
.group {
  background: var(--ls-surface);
  border: 1px solid var(--ls-border);
  border-radius: var(--ls-radius-md);
  overflow: hidden;
}
.group--danger { border-color: color-mix(in srgb, var(--ls-danger) 45%, transparent); }

.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 11px 14px;
  border-top: 0.5px solid var(--ls-border);
  min-height: 44px;
}
.row:first-child { border-top: 0; }

.row-label { font-size: var(--ls-fs-md); color: var(--ls-ink); }

.switch-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  width: 100%;
  cursor: pointer;
}

.group-hint {
  margin: 8px 4px 0;
  color: var(--ls-ink-muted);
  font-size: var(--ls-fs-sm);
}

.danger-text { display: flex; flex-direction: column; gap: 2px; }
.danger-sub { color: var(--ls-ink-muted); font-size: var(--ls-fs-sm); }

/* ── Toggle switch (pure CSS) ─────────────────────────────────────── */
.switch {
  position: relative;
  display: inline-block;
  width: 51px;
  height: 31px;
  flex: none;
}
.switch input {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  margin: 0;
  opacity: 0;
  cursor: pointer;
}
.switch-track {
  position: absolute;
  inset: 0;
  border-radius: var(--ls-radius-pill);
  background: var(--ls-control-off);
  transition: background 0.2s ease;
  pointer-events: none;
}
.switch-track::after {
  content: "";
  position: absolute;
  top: 2px;
  left: 2px;
  width: 27px;
  height: 27px;
  border-radius: 50%;
  background: var(--ls-surface);
  box-shadow: var(--ls-shadow-knob);
  transition: transform 0.2s ease;
}
.switch input:checked + .switch-track { background: var(--ls-success); }
.switch input:checked + .switch-track::after { transform: translateX(20px); }
.switch input:focus-visible + .switch-track {
  outline: 2px solid var(--ls-accent);
  outline-offset: 2px;
}

/* ── Inputs ───────────────────────────────────────────────────────── */
input[type="text"],
input[type="number"],
input[type="password"],
select {
  font: inherit;
  color: inherit;
  background: var(--ls-surface);
  border: 1px solid var(--ls-border);
  border-radius: var(--ls-radius-sm);
  padding: 6px 10px;
  min-width: 0;
}
input[type="text"] { width: 180px; }
input[type="number"] { width: 100px; }
input:focus, select:focus {
  outline: 2px solid var(--ls-accent);
  outline-offset: 0;
  border-color: var(--ls-accent);
}

select {
  appearance: none;
  -webkit-appearance: none;
  padding-right: 28px;
  background-image: url("data:image/svg+xml;charset=utf-8,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='6'%3E%3Cpath d='M1 1l4 4 4-4' fill='none' stroke='%2357606a' stroke-width='1.5'/%3E%3C/svg%3E");
  background-repeat: no-repeat;
  background-position: right 9px center;
}

/* ── Buttons ──────────────────────────────────────────────────────── */
.btn-primary {
  font: inherit;
  font-weight: 600;
  color: var(--ls-surface);
  background: var(--ls-accent);
  border: 0;
  border-radius: var(--ls-radius-md);
  padding: 7px 16px;
  cursor: pointer;
}
.btn-primary:hover:not(:disabled) { background: var(--ls-accent-strong); }
.btn-primary:disabled { opacity: 0.45; cursor: default; }
.btn-block { display: block; width: 100%; margin-top: 12px; }

.btn-danger {
  font: inherit;
  font-weight: 600;
  color: var(--ls-danger);
  background: var(--ls-surface);
  border: 1px solid color-mix(in srgb, var(--ls-danger) 60%, transparent);
  border-radius: var(--ls-radius-md);
  padding: 7px 14px;
  cursor: pointer;
  white-space: nowrap;
}
.btn-danger:hover:not(:disabled) { background: color-mix(in srgb, var(--ls-danger) 8%, transparent); }
.btn-danger:disabled { opacity: 0.45; cursor: default; }

.btn-outline {
  display: block;
  text-align: center;
  margin-top: 4px;
  padding: 9px 14px;
  border: 1px solid var(--ls-ink);
  border-radius: var(--ls-radius-sm);
  color: var(--ls-ink);
  font-weight: 600;
  font-size: var(--ls-fs-md);
  text-decoration: none;
  background: var(--ls-surface);
  transition: background 0.12s ease, color 0.12s ease;
}
.btn-outline:hover { background: var(--ls-ink); color: var(--ls-surface); text-decoration: none; }

/* ── Banners ──────────────────────────────────────────────────────── */
.banner {
  background: var(--ls-surface);
  border: 1px solid var(--ls-border);
  border-radius: var(--ls-radius-md);
  padding: 12px 16px;
  margin-bottom: 4px;
}
.banner[data-kind="building"] { border-color: var(--ls-warning); background: var(--ls-warning-tint); }
.banner[data-kind="done"] { border-color: var(--ls-success); background: var(--ls-success-tint); }
.banner[data-kind="failed"] { border-color: var(--ls-danger); background: var(--ls-danger-tint); }

.banner-row {
  display: flex;
  align-items: baseline;
  gap: 10px;
  flex-wrap: wrap;
}
.banner-msg {
  font-family: var(--ls-font-mono);
  font-size: var(--ls-fs-xs);
  color: var(--ls-ink-muted);
  overflow-wrap: anywhere;
}

.error-banner {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
  background: var(--ls-danger-tint);
  border: 1px solid var(--ls-danger);
  color: var(--ls-danger-deep);
  border-radius: var(--ls-radius-md);
  padding: 10px 14px;
  margin-bottom: 12px;
  font-size: var(--ls-fs-md); /* pinned: keeps it stable across body-size differences */
}
.banner-close {
  background: none;
  border: 0;
  color: inherit;
  font-size: 18px; /* dismiss glyph, not on the type scale */
  line-height: 1;
  cursor: pointer;
  padding: 0 2px;
}

/* ── Rebuild progress card ────────────────────────────────────────── */
.progress-card {
  background: var(--ls-surface);
  border: 1px solid var(--ls-border);
  border-radius: var(--ls-radius-md);
  padding: 12px 14px;
  margin-bottom: 16px;
}
.progress-card[data-kind="building"] { border-color: var(--ls-warning); background: var(--ls-warning-tint); }
.progress-card[data-kind="done"] { border-color: var(--ls-success); background: var(--ls-success-tint); }
.progress-card[data-kind="failed"] { border-color: var(--ls-danger); background: var(--ls-danger-tint); }

.progress-head {
  display: flex;
  align-items: center;
  gap: 10px;
}

.spinner {
  width: 14px;
  height: 14px;
  flex: none;
  border-radius: 50%;
  border: 2px solid rgba(0, 0, 0, 0.15);
  border-top-color: var(--ls-accent);
  animation: spin 0.8s linear infinite;
}
@keyframes spin { to { transform: rotate(360deg); } }

.progress-card[data-kind="done"] .spinner {
  animation: none;
  border: 0;
  width: auto;
  height: auto;
  color: var(--ls-success);
  font-weight: 700;
}
.progress-card[data-kind="done"] .spinner::before { content: "\2713 "; }  /* U+2713 check mark */
.progress-card[data-kind="failed"] .spinner {
  animation: none;
  border: 0;
  width: auto;
  height: auto;
  color: var(--ls-danger);
  font-weight: 700;
}
.progress-card[data-kind="failed"] .spinner::before { content: "\00d7 "; }  /* U+00D7 multiplication sign */

.progress-log {
  display: block;
  margin-top: 8px;
  font-size: var(--ls-fs-xs);
  color: var(--ls-ink-muted);
  overflow-wrap: anywhere;
  min-height: 1em;
}

/* ── Token unlock overlay ─────────────────────────────────────────── */
.auth-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.32);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
  z-index: 10;
}
.auth-card {
  background: var(--ls-surface);
  border: 1px solid var(--ls-border);
  border-radius: var(--ls-radius-lg);
  padding: 22px;
  width: 340px;
  max-width: 100%;
  box-shadow: var(--ls-shadow-lg);
}
.auth-card h2 { margin: 0 0 6px; font-size: var(--ls-fs-xl); }
.auth-hint { margin: 0 0 14px; color: var(--ls-ink-muted); font-size: var(--ls-fs-sm); }
.auth-card label {
  display: block;
  font-size: var(--ls-fs-sm);
  font-weight: 600;
  margin-bottom: 4px;
}
.auth-card input { width: 100%; }
.auth-error { color: var(--ls-danger); font-size: var(--ls-fs-sm); margin: 8px 0 0; }

/* ── Narrow screens ───────────────────────────────────────────────── */
@media (max-width: 720px) {
  .layout { flex-direction: column; gap: 14px; padding: 14px 10px 48px; }
  .sidebar { width: 100%; overflow-x: auto; }
  .side-list { flex-direction: row; }
  .side-item {
    border-top: 0;
    border-left: 0.5px solid var(--ls-border);
    white-space: nowrap;
    padding: 10px 14px;
  }
  .side-item:first-child { border-left: 0; }
  .content { max-width: none; width: 100%; }
  input[type="text"] { width: 140px; }
}
@media (max-width: 560px) {
  .topbar { padding: 0 12px; gap: 8px; }
  .topnav a { padding: 6px 7px; font-size: var(--ls-fs-sm); }
  .container { padding: 14px 10px 40px; }
  .grid { grid-template-columns: 1fr 1fr; gap: 10px; }
}
@media (max-width: 380px) {
  .grid { grid-template-columns: 1fr; }
}
```

- [ ] **Step 2: Verify no hex literals leaked** — Run: `grep -cE '#[0-9a-fA-F]{3}' design-system/losos.css` → Expected: `0` (the select arrow uses `%23`-encoded color inside a data URI, which this grep does not match).

- [ ] **Step 3: Verify all class names present** — Run:

```bash
for c in topbar brand topnav indbar container grid layout sidebar side-list side-item side-item--danger content content-header section card card-head status dot desc chip-row chip hint-card syslist sysrow group group--danger row row-label switch-row group-hint danger-text danger-sub switch switch-track btn-primary btn-block btn-danger btn-outline banner banner-row banner-msg error-banner banner-close progress-card progress-head spinner progress-log auth-overlay auth-card auth-hint auth-error is-active; do grep -q "\.$c" design-system/losos.css || echo "MISSING: $c"; done
```

Expected: no output. Also confirm the rename is total: `grep -c '\.open' design-system/losos.css` → `0`.

- [ ] **Step 4: Commit**

```bash
git add design-system/losos.css
git commit -m "feat(design-system): add unified component stylesheet"
```

### Task 3: Ship the stylesheets in the losos-admin-ui package

**Files:**
- Modify: `flake/packages.nix` (the `losos-admin-ui` runCommand, currently `cp -rT ${lib.cleanSource ./../admin-ui} $out` + `chmod -R u+rwX $out`)

**Interfaces:**
- Produces: `$out/ds/tokens.css`, `$out/ds/losos.css` in the package for Task 4's nginx alias.

- [ ] **Step 1: Extend the derivation** — replace the runCommand body so it reads:

```nix
losos-admin-ui = pkgs.runCommand "losos-admin-ui-0.1.0" { } ''
  cp -rT ${lib.cleanSource ./../admin-ui} $out
  chmod -R u+rwX $out
  # Design-system stylesheets, copied by explicit path — never
  # cleanSource of design-system/ (react/ must not enter the closure).
  mkdir -p $out/ds
  cp ${./../design-system/tokens.css} $out/ds/tokens.css
  cp ${./../design-system/losos.css} $out/ds/losos.css
'';
```

- [ ] **Step 2: Build and verify**

Run: `nix build .#losos-admin-ui && test -s result/ds/tokens.css && test -s result/ds/losos.css && echo OK`
Expected: `OK`

- [ ] **Step 3: Commit**

```bash
git add flake/packages.nix
git commit -m "feat(design-system): ship ds stylesheets in losos-admin-ui"
```

### Task 4: Serve /ds/ from the front vhost

**Files:**
- Modify: `modules/containers.nix` — the `virtualHosts."losos-front"` locations block (near the existing `"/settings/"` and `"= /common.css"` aliases, around lines 155-179)

**Interfaces:**
- Consumes: `$out/ds/` from Task 3 (`adminUi` in scope as `config.losos.admin.ui`).
- Produces: `/ds/tokens.css` and `/ds/losos.css` URLs both pages link in Tasks 5-6.

- [ ] **Step 1: Add the location** — next to the existing `"/settings/"` alias location, add (do NOT remove `"= /common.css"` yet — settings still links it until Task 6):

```nix
"/ds/" = {
  alias = "${adminUi}/ds/";
};
```

- [ ] **Step 2: Verify the system evaluates**

Run: `nix build .#nixosConfigurations.install.config.system.build.toplevel --dry-run 2>&1 | tail -3`
Expected: a will-be-built report, no evaluation errors.

- [ ] **Step 3: Commit**

```bash
git add modules/containers.nix
git commit -m "feat(design-system): serve /ds/ stylesheets from the front vhost"
```

### Task 5: Migrate the dashboard page

**Files:**
- Modify: `admin-ui/dashboard/index.html` (two link tags; four `class="open"` anchors)
- Delete: `admin-ui/dashboard/style.css`

- [ ] **Step 1: Swap the stylesheet links** — replace exactly:

```html
<link rel="stylesheet" href="/common.css">
<link rel="stylesheet" href="./style.css">
```

with:

```html
<link rel="stylesheet" href="/ds/tokens.css">
<link rel="stylesheet" href="/ds/losos.css">
```

- [ ] **Step 2: Rename the four action links** — `class="open"` → `class="btn-outline"` on all four anchors (Nextcloud `href="/nextcloud"`, Forgejo `href="/forgejo/"`, Settings `href="/settings"`, Tahoe `id="link-tahoe"`). Verify: `grep -c 'class="open"' admin-ui/dashboard/index.html` → `0`; `grep -c 'btn-outline' admin-ui/dashboard/index.html` → `4`.

- [ ] **Step 3: Delete the page stylesheet**

```bash
git rm admin-ui/dashboard/style.css
```

- [ ] **Step 4: Build and verify the package**

Run: `nix build .#losos-admin-ui && grep -c '/ds/' result/dashboard/index.html && test ! -e result/dashboard/style.css && echo OK`
Expected: `2` then `OK`.

- [ ] **Step 5: Commit**

```bash
git add admin-ui/dashboard/index.html
git commit -m "feat(design-system): dashboard consumes ds stylesheets"
```

(The `git rm` from Step 3 is already staged.)

### Task 6: Migrate the settings page and retire the old CSS

**Files:**
- Modify: `admin-ui/settings/index.html` (two link tags)
- Delete: `admin-ui/settings/style.css`, `admin-ui/common.css`
- Modify: `modules/containers.nix` (remove the `"= /common.css"` alias location; keep `"= /common.js"`)

- [ ] **Step 1: Swap the stylesheet links** — same replacement as Task 5 Step 1, in `admin-ui/settings/index.html`.

- [ ] **Step 2: Delete the retired stylesheets**

```bash
git rm admin-ui/settings/style.css admin-ui/common.css
```

- [ ] **Step 3: Drop the dead nginx alias** — in `modules/containers.nix`, delete the whole `"= /common.css".alias = "${adminUi}/common.css";` location. `"= /common.js"` stays.

- [ ] **Step 4: Verify** — Run:

```bash
nix build .#losos-admin-ui && grep -c '/ds/' result/settings/index.html && test ! -e result/common.css && nix build .#nixosConfigurations.install.config.system.build.toplevel --dry-run 2>&1 | tail -1 && echo OK
```

Expected: `2`, then a clean dry-run line, then `OK`.

- [ ] **Step 5: Commit**

```bash
git add admin-ui/settings/index.html modules/containers.nix
git commit -m "feat(design-system): settings consumes ds stylesheets; retire page CSS"
```

### Task 7: Dev tooling and docs

**Files:**
- Modify: `flake/devshell.nix` (add `pkgs.nodejs` to `nativeBuildInputs`)
- Modify: `CLAUDE.md` (stale `:8081`; document `design-system/`)
- Modify: `.gitignore`

- [ ] **Step 1: Add node to the devshell** — in `flake/devshell.nix` `nativeBuildInputs`, after `pkgs.haskellPackages.hpack`, add a line: `pkgs.nodejs`.

- [ ] **Step 2: Verify** — Run: `nix develop .# -c node --version` → Expected: a `v2x.y.z` version string.

- [ ] **Step 3: Fix CLAUDE.md** — in the "Containers and the single front door" paragraph, replace `the admin SPA on ` `` `:8081` `` with `the admin SPA on the same ` `` `:80` `` ` vhost` (the `:8081` claim is stale — the SPA rides the default `losos-front` vhost). In the same paragraph, after the sentence about path-based routing, add: `Shared stylesheets live in ` `` `design-system/` `` ` (` `` `tokens.css` `` ` + ` `` `losos.css` `` `, served at ` `` `/ds/` `` `); ` `` `design-system/react` `` ` is a dev-machine-only React wrapper package for claude.ai/design — never part of the Nix closure.`

- [ ] **Step 4: gitignore the react build artifacts** — append two lines to `.gitignore`:

```
design-system/react/node_modules
design-system/react/dist
```

- [ ] **Step 5: Commit**

```bash
git add flake/devshell.nix CLAUDE.md .gitignore
git commit -m "chore(design-system): devshell node, docs, gitignore"
```

### Task 8: Visual verification checkpoint

**Files:** none (verification only)

- [ ] **Step 1: Serve the built package** — Run: `nix build .#losos-admin-ui && nix run nixpkgs#python3 -- -m http.server 8090 --directory result`
  Note: this serves the package root, so `/ds/*.css`, `/common.js` resolve; open `http://localhost:8090/dashboard/` and `http://localhost:8090/settings/`. Page JS may 404 (`/app.js` is root-relative behind nginx) — irrelevant for a styling check; the API-dependent widgets just stay in their initial state.

- [ ] **Step 2: Eyeball against the approved design** — Dashboard: must look exactly as before (Forgejo flavor IS the token set; check cards, dots, chips, the four outline buttons). Settings: Forgejo blue links/focus/active-sidebar/primary-button, `#2ecc71` toggle-on, `#e5534b` danger, 8px group corners, 15px body. This is the human gate from the spec — pause for the user's confirmation before continuing to Phase B.

- [ ] **Step 3: Regression check** — Run: `nix build .#checks.x86_64-linux.losos-admin-daemon` → Expected: builds green (it boots no nginx/SPA; this is the canary that nothing unrelated broke).

---

### Task 9: React package scaffold

**Files:**
- Create: `design-system/react/package.json`, `design-system/react/tsconfig.json`, `design-system/react/src/index.ts` (placeholder export list, filled by Tasks 10-13), `design-system/react/tests/render.test.tsx` (assert helper + first trivial assert)

**Interfaces:**
- Produces: `npm run typecheck` / `npm run test` / `npm run build` used by every later task; the `h()`-free TSX toolchain (jsx automatic).

- [ ] **Step 1: package.json**

```json
{
  "name": "losos-ds",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "exports": {
    ".": { "types": "./dist/index.d.ts", "import": "./dist/index.js" },
    "./tokens.css": "./dist/tokens.css",
    "./losos.css": "./dist/losos.css"
  },
  "scripts": {
    "typecheck": "tsc --noEmit",
    "test": "esbuild tests/render.test.tsx --bundle --format=esm --platform=node --jsx=automatic --outfile=dist/.render-test.mjs && node dist/.render-test.mjs",
    "build": "esbuild src/index.ts --bundle --format=esm --jsx=automatic --external:react --external:react-dom --outfile=dist/index.js && tsc --emitDeclarationOnly && cp ../tokens.css ../losos.css dist/"
  },
  "peerDependencies": {
    "react": "^19.0.0",
    "react-dom": "^19.0.0"
  },
  "devDependencies": {
    "@types/react": "^19.0.0",
    "esbuild": "^0.25.0",
    "react": "^19.0.0",
    "react-dom": "^19.0.0",
    "typescript": "^5.7.0"
  }
}
```

- [ ] **Step 2: tsconfig.json**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "jsx": "react-jsx",
    "strict": true,
    "declaration": true,
    "outDir": "dist",
    "skipLibCheck": true
  },
  "include": ["src"]
}
```

- [ ] **Step 3: src/index.ts** — starts empty of components; Tasks 10-13 append exports. Initial content:

```ts
// losos-ds — thin React wrappers over design-system/losos.css class names.
// Styling comes entirely from the CSS; components render markup only.
export {};
```

- [ ] **Step 4: tests/render.test.tsx** — the assert helper every later task extends:

```tsx
import { renderToStaticMarkup } from 'react-dom/server';
import assert from 'node:assert';
import type { ReactElement } from 'react';

export function expectMarkup(el: ReactElement, ...needles: string[]) {
  const html = renderToStaticMarkup(el);
  for (const n of needles) {
    assert.ok(html.includes(n), `expected ${JSON.stringify(n)} in:\n${html}`);
  }
  return html;
}

// Tasks 10-13 append component assertions below this line.
console.log('render tests: OK');
```

- [ ] **Step 5: Install and verify the pipeline**

Run (inside `nix develop .#` for node): `cd design-system/react && npm install && npm run typecheck && npm run test`
Expected: install completes, typecheck silent, `render tests: OK`.

- [ ] **Step 6: Commit**

```bash
git add design-system/react/package.json design-system/react/tsconfig.json design-system/react/src/index.ts design-system/react/tests/render.test.tsx design-system/react/package-lock.json
git commit -m "feat(design-system): react package scaffold"
```

### Task 10: Chrome & shell components

**Files:**
- Create: `design-system/react/src/components/chrome.tsx`
- Modify: `design-system/react/src/index.ts`, `design-system/react/tests/render.test.tsx`

**Interfaces:**
- Produces: `Topbar {brand?, href?, children?}`, `Indbar {}`, `Container {children}`, `Grid {children}`, `Layout {children}`, `Sidebar {children}`, `SideItem {active?, danger?, href?, onClick?, children}`, `Content {children}`, `ContentHeader {title, children?}`, `Section {active?, id?, children}`.

- [ ] **Step 1: Write the failing test** — append to `tests/render.test.tsx` (above the final `console.log`):

```tsx
import {
  Topbar, Indbar, Container, Grid, Layout, Sidebar, SideItem,
  Content, ContentHeader, Section,
} from '../src/index';

expectMarkup(<Topbar brand="losos"><a href="/settings">Settings</a></Topbar>,
  'class="topbar"', 'class="brand"', 'class="topnav"', '>losos<');
expectMarkup(<Indbar />, 'class="indbar"', 'role="progressbar"');
expectMarkup(<Container>x</Container>, 'class="container"');
expectMarkup(<Grid>x</Grid>, 'class="grid"', 'role="list"');
expectMarkup(<Layout>x</Layout>, 'class="layout"');
expectMarkup(<Sidebar><SideItem active>General</SideItem><SideItem danger>Danger</SideItem></Sidebar>,
  'class="sidebar"', 'class="side-list"', 'side-item is-active', 'side-item side-item--danger');
expectMarkup(<Content><ContentHeader title="Sharing" /></Content>,
  'class="content"', 'class="content-header"', '<h1>Sharing</h1>');
expectMarkup(<Section active>panel</Section>, 'section is-active');
```

- [ ] **Step 2: Run to verify it fails** — `npm run test` → Expected: FAIL (exports missing).

- [ ] **Step 3: Implement** — `src/components/chrome.tsx`:

```tsx
import type { ReactNode } from 'react';

export function Topbar({ brand = 'losos', href = '/', children }:
  { brand?: string; href?: string; children?: ReactNode }) {
  return (
    <header className="topbar">
      <a className="brand" href={href}>{brand}</a>
      <nav className="topnav">{children}</nav>
    </header>
  );
}

export function Indbar() {
  return <div className="indbar" role="progressbar"><span /></div>;
}

export function Container({ children }: { children: ReactNode }) {
  return <main className="container">{children}</main>;
}

export function Grid({ children }: { children: ReactNode }) {
  return <div className="grid" role="list">{children}</div>;
}

export function Layout({ children }: { children: ReactNode }) {
  return <div className="layout">{children}</div>;
}

export function Sidebar({ children }: { children: ReactNode }) {
  return <aside className="sidebar"><nav className="side-list">{children}</nav></aside>;
}

export function SideItem({ active, danger, href = '#', onClick, children }:
  { active?: boolean; danger?: boolean; href?: string; onClick?: () => void; children: ReactNode }) {
  const cls = ['side-item', danger && 'side-item--danger', active && 'is-active']
    .filter(Boolean).join(' ');
  return <a className={cls} href={href} onClick={onClick}>{children}</a>;
}

export function Content({ children }: { children: ReactNode }) {
  return <main className="content">{children}</main>;
}

export function ContentHeader({ title, children }:
  { title: string; children?: ReactNode }) {
  return <div className="content-header"><h1>{title}</h1>{children}</div>;
}

export function Section({ active, id, children }:
  { active?: boolean; id?: string; children: ReactNode }) {
  return <section id={id} className={active ? 'section is-active' : 'section'}>{children}</section>;
}
```

Replace `src/index.ts` content with:

```ts
// losos-ds — thin React wrappers over design-system/losos.css class names.
// Styling comes entirely from the CSS; components render markup only.
export * from './components/chrome';
```

- [ ] **Step 4: Run tests** — `npm run typecheck && npm run test` → Expected: PASS, `render tests: OK`.

- [ ] **Step 5: Commit**

```bash
git add design-system/react/src design-system/react/tests
git commit -m "feat(design-system): react chrome and shell components"
```

### Task 11: Display components

**Files:**
- Create: `design-system/react/src/components/display.tsx`
- Modify: `design-system/react/src/index.ts`, `design-system/react/tests/render.test.tsx`

**Interfaces:**
- Produces: `Card {title, status?, children?}`, `StatusDot {state, label?}`, `Chip {children}`, `ChipRow {children}`, `Desc {children}`, `HintCard {children}`, `SysList {children}`, `SysRow {label, children}`.

- [ ] **Step 1: Write the failing test** — append:

```tsx
import {
  Card, StatusDot, Chip, ChipRow, Desc, HintCard, SysList, SysRow,
} from '../src/index';

expectMarkup(
  <Card title="Nextcloud" status={<StatusDot state="up" label="running" />}>
    <Desc>Files</Desc>
    <ChipRow><Chip>/nextcloud</Chip></ChipRow>
  </Card>,
  'class="card"', 'role="listitem"', 'class="card-head"', '<h2>Nextcloud</h2>',
  'class="status"', 'data-state="up"', 'class="desc"', 'class="chip-row"', 'class="chip"');
expectMarkup(<HintCard>Sign in first.</HintCard>, 'class="hint-card"');
expectMarkup(<SysList><SysRow label="Hostname"><Chip>losos</Chip></SysRow></SysList>,
  'class="syslist"', 'class="sysrow"', '<dt>Hostname</dt>', '<dd>');
```

- [ ] **Step 2: Run to verify it fails** — `npm run test` → Expected: FAIL.

- [ ] **Step 3: Implement** — `src/components/display.tsx`:

```tsx
import type { ReactNode } from 'react';

export function Card({ title, status, children }:
  { title: string; status?: ReactNode; children?: ReactNode }) {
  return (
    <article className="card" role="listitem">
      <div className="card-head"><h2>{title}</h2>{status}</div>
      {children}
    </article>
  );
}

export function StatusDot({ state, label }:
  { state: 'up' | 'down' | 'checking'; label?: string }) {
  return (
    <span className="status">
      <span className="dot" data-state={state} />
      {label && <span className="status-text">{label}</span>}
    </span>
  );
}

export function Chip({ children }: { children: ReactNode }) {
  return <code className="chip">{children}</code>;
}

export function ChipRow({ children }: { children: ReactNode }) {
  return <p className="chip-row">{children}</p>;
}

export function Desc({ children }: { children: ReactNode }) {
  return <p className="desc">{children}</p>;
}

export function HintCard({ children }: { children: ReactNode }) {
  return <div className="hint-card"><p>{children}</p></div>;
}

export function SysList({ children }: { children: ReactNode }) {
  return <dl className="syslist">{children}</dl>;
}

export function SysRow({ label, children }:
  { label: string; children: ReactNode }) {
  return <div className="sysrow"><dt>{label}</dt><dd>{children}</dd></div>;
}
```

Append to `src/index.ts`: `export * from './components/display';`

- [ ] **Step 4: Run tests** — `npm run typecheck && npm run test` → Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add design-system/react/src design-system/react/tests
git commit -m "feat(design-system): react display components"
```

### Task 12: Feedback components

**Files:**
- Create: `design-system/react/src/components/feedback.tsx`
- Modify: `design-system/react/src/index.ts`, `design-system/react/tests/render.test.tsx`

**Interfaces:**
- Produces: `Banner {kind, title, message?}`, `ErrorBanner {onClose?, children}`, `ProgressCard {kind, title, log?}`, `Spinner {}` (uses Task 10's `Indbar`).

- [ ] **Step 1: Write the failing test** — append:

```tsx
import { Banner, ErrorBanner, ProgressCard, Spinner } from '../src/index';

expectMarkup(<Banner kind="building" title="Rebuilding…" message="step 3/7" />,
  'class="banner"', 'data-kind="building"', 'class="banner-row"',
  'class="banner-msg"', 'class="indbar"');
expectMarkup(<ErrorBanner onClose={() => {}}>Request failed</ErrorBanner>,
  'class="error-banner"', 'class="banner-close"');
expectMarkup(<ProgressCard kind="failed" title="Rebuild failed" log="exit 1" />,
  'class="progress-card"', 'data-kind="failed"', 'class="progress-head"',
  'progress-icon spinner', 'class="progress-log"');
expectMarkup(<Spinner />, 'progress-icon spinner');
```

- [ ] **Step 2: Run to verify it fails** — `npm run test` → Expected: FAIL.

- [ ] **Step 3: Implement** — `src/components/feedback.tsx`:

```tsx
import type { ReactNode } from 'react';
import { Indbar } from './chrome';

export type StateKind = 'building' | 'done' | 'failed';

export function Banner({ kind, title, message }:
  { kind: StateKind; title: string; message?: string }) {
  return (
    <div className="banner" data-kind={kind}>
      <div className="banner-row">
        <strong>{title}</strong>
        {message && <span className="banner-msg">{message}</span>}
      </div>
      <Indbar />
    </div>
  );
}

export function ErrorBanner({ onClose, children }:
  { onClose?: () => void; children: ReactNode }) {
  return (
    <div className="error-banner">
      <span>{children}</span>
      {onClose && (
        <button type="button" className="banner-close" aria-label="Dismiss" onClick={onClose}>
          ×
        </button>
      )}
    </div>
  );
}

export function Spinner() {
  return <span className="progress-icon spinner" />;
}

export function ProgressCard({ kind, title, log }:
  { kind: StateKind; title: string; log?: string }) {
  return (
    <div className="progress-card" data-kind={kind}>
      <div className="progress-head">
        <Spinner />
        <strong>{title}</strong>
      </div>
      {log !== undefined && <code className="progress-log">{log}</code>}
      <Indbar />
    </div>
  );
}
```

Append to `src/index.ts`: `export * from './components/feedback';`

- [ ] **Step 4: Run tests** — `npm run typecheck && npm run test` → Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add design-system/react/src design-system/react/tests
git commit -m "feat(design-system): react feedback components"
```

### Task 13: Form, group & overlay components

**Files:**
- Create: `design-system/react/src/components/forms.tsx`
- Modify: `design-system/react/src/index.ts`, `design-system/react/tests/render.test.tsx`

**Interfaces:**
- Produces: `Button {variant?, block?, href?, ...button props}`, `Group {danger?, children}`, `Row {children}`, `GroupHint {children}`, `Switch {checked, onChange?, label?}`, `Input {…input props}`, `Select {…select props}`, `AuthOverlay {children}`, `AuthCard {title, hint?, error?, onSubmit?, children}`.

- [ ] **Step 1: Write the failing test** — append:

```tsx
import {
  Button, Group, Row, GroupHint, Switch, Input, Select, AuthOverlay, AuthCard,
} from '../src/index';

expectMarkup(<Button>Apply</Button>, 'class="btn-primary"');
expectMarkup(<Button variant="danger">Reset…</Button>, 'class="btn-danger"');
expectMarkup(<Button variant="outline" href="/nextcloud">Open</Button>,
  'class="btn-outline"', '<a ');
expectMarkup(<Button block>Unlock</Button>, 'btn-primary btn-block');
expectMarkup(
  <Group danger><Row><Switch checked label="Share my storage" /></Row><GroupHint>hint</GroupHint></Group>,
  'group group--danger', 'class="row"', 'class="switch-row"', 'class="row-label"',
  'class="switch"', 'checked', 'class="switch-track"', 'class="group-hint"');
expectMarkup(<Row><Input type="text" defaultValue="losos" /><Select><option>local</option></Select></Row>,
  '<input type="text"', '<select');
expectMarkup(
  <AuthOverlay><AuthCard title="Unlock" hint="Paste the admin token." error="Wrong token."><Input type="password" /></AuthCard></AuthOverlay>,
  'class="auth-overlay"', 'class="auth-card"', '<h2>Unlock</h2>',
  'class="auth-hint"', 'class="auth-error"');
```

- [ ] **Step 2: Run to verify it fails** — `npm run test` → Expected: FAIL.

- [ ] **Step 3: Implement** — `src/components/forms.tsx`:

```tsx
import type {
  ButtonHTMLAttributes, InputHTMLAttributes, ReactNode, SelectHTMLAttributes,
} from 'react';

type ButtonVariant = 'primary' | 'danger' | 'outline';

export function Button({ variant = 'primary', block, href, children, ...rest }:
  { variant?: ButtonVariant; block?: boolean; href?: string; children: ReactNode }
  & ButtonHTMLAttributes<HTMLButtonElement>) {
  const cls = [`btn-${variant}`, block && 'btn-block'].filter(Boolean).join(' ');
  if (href !== undefined) return <a className={cls} href={href}>{children}</a>;
  return <button type="button" className={cls} {...rest}>{children}</button>;
}

export function Group({ danger, children }:
  { danger?: boolean; children: ReactNode }) {
  return <div className={danger ? 'group group--danger' : 'group'}>{children}</div>;
}

export function Row({ children }: { children: ReactNode }) {
  return <div className="row">{children}</div>;
}

export function GroupHint({ children }: { children: ReactNode }) {
  return <p className="group-hint">{children}</p>;
}

export function Switch({ checked, onChange, label }:
  { checked: boolean; onChange?: (checked: boolean) => void; label?: string }) {
  const control = (
    <span className="switch">
      <input
        type="checkbox"
        checked={checked}
        onChange={e => onChange?.(e.currentTarget.checked)}
        readOnly={onChange === undefined}
      />
      <span className="switch-track" />
    </span>
  );
  if (label === undefined) return control;
  return (
    <label className="switch-row">
      <span className="row-label">{label}</span>
      {control}
    </label>
  );
}

export function Input(props: InputHTMLAttributes<HTMLInputElement>) {
  return <input {...props} />;
}

export function Select(props: SelectHTMLAttributes<HTMLSelectElement>) {
  return <select {...props} />;
}

export function AuthOverlay({ children }: { children: ReactNode }) {
  return <div className="auth-overlay">{children}</div>;
}

export function AuthCard({ title, hint, error, onSubmit, children }:
  { title: string; hint?: string; error?: string; onSubmit?: () => void; children: ReactNode }) {
  return (
    <form className="auth-card" onSubmit={e => { e.preventDefault(); onSubmit?.(); }}>
      <h2>{title}</h2>
      {hint && <p className="auth-hint">{hint}</p>}
      {children}
      {error && <p className="auth-error">{error}</p>}
    </form>
  );
}
```

Append to `src/index.ts`: `export * from './components/forms';`

- [ ] **Step 4: Run tests** — `npm run typecheck && npm run test` → Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add design-system/react/src design-system/react/tests
git commit -m "feat(design-system): react form, group and overlay components"
```

### Task 14: Build the package and close out

**Files:**
- Verify only (build artifacts are gitignored)

- [ ] **Step 1: Full build** — Run: `cd design-system/react && npm run build`
Expected: `dist/index.js`, `dist/index.d.ts` (plus per-module `.d.ts`), `dist/tokens.css`, `dist/losos.css` all exist. Verify: `test -s dist/index.js && test -s dist/index.d.ts && test -s dist/losos.css && echo OK` → `OK`.

- [ ] **Step 2: Verify the ESM surface** — Run: `node -e "import('./design-system/react/dist/index.js').then(m => console.log(Object.keys(m).length))"` from the repo root, inside `nix develop .#` with react resolvable — simpler equivalent from `design-system/react/`: `node -e "import('./dist/index.js').then(m => console.log(Object.keys(m).sort().join(',')))"` → Expected: 31 exported names (AuthCard through Topbar; `StateKind` is a type and does not appear), no error. (react is external in the bundle; node resolves it from `node_modules` — run from `design-system/react/`.)

- [ ] **Step 3: Final regression sweep**

```bash
nix build .#losos-admin-ui && nix build .#checks.x86_64-linux.losos-admin-daemon && nix build .#nixosConfigurations.install.config.system.build.toplevel --dry-run
```

Expected: all green.

- [ ] **Step 4: Commit any stragglers** — only if `git status --short` shows plan-related files unstaged (never the six pre-staged installer files): `git add <them>` and `git commit -m "chore(design-system): close out implementation"`.

---

## Deferred (per spec, not in this plan)

- The `/design-sync` run against `design-system/react` (package shape; `.design-sync/config.json` will point at it).
- A front-door nginx VM test.
- Dark mode / additional themes.
