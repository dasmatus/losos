# losos design system — design spec

Date: 2026-08-21
Status: approved in brainstorming (visual companion session); pending spec review

## Purpose

Extract a design system from the hand-written admin UI so that one visual
vocabulary serves two consumers:

1. **The appliance** — `admin-ui/` keeps its dependency-free, no-build,
   plain-HTML/CSS/JS character, but its styles move to shared, token-driven
   stylesheets.
2. **claude.ai/design** — a thin React component layer over the same CSS,
   built only on the dev machine, so `/design-sync` can upload real losos
   components for the design agent to build with.

Origin: a `/design-sync` run found this repo has no syncable design system
(no components, no tokens, no JS package). This spec creates one.

## Decisions made (with the user, in order)

- **Consumers**: both admin-ui and claude.ai/design ("full pipeline").
- **Unification**: the two page flavors (dashboard "Forgejo", settings
  "macOS") unify to **Forgejo**. Dashboard stays pixel-identical; settings
  re-skins (palette, radius, body font size) while keeping its structure.
- **Structure**: **CSS-first** (approach A). Plain `tokens.css` +
  `losos.css` are the source of visual truth, consumed directly by
  admin-ui with no build step. React wrappers exist only for
  claude.ai/design and never enter the Nix closure.

## 1. Token layer — `design-system/tokens.css`

CSS custom properties on `:root`, prefix `--ls-*`. Values are the existing
literals; Forgejo wins every conflict.

| Token | Value | Absorbs / replaces |
|---|---|---|
| `--ls-bg` | `#f5f7fa` | page background (already shared) |
| `--ls-surface` | `#ffffff` | cards, groups, inputs |
| `--ls-surface-muted` | `#f0f2f5` | chip bg, hover fills |
| `--ls-ink` | `#1c2128` | text and topbar bg; replaces `#1d1d1f` |
| `--ls-ink-muted` | `#57606a` | secondary text; replaces `#6e6e73` |
| `--ls-border` | `#d0d5dd` | already shared |
| `--ls-border-faint` | `#eef1f4` | hairline separators |
| `--ls-accent` | `#1f6feb` | links, focus, primary; replaces `#007aff` |
| `--ls-accent-strong` | `#1a5fd0` | primary hover; replaces `#0a6bdb` |
| `--ls-success` | `#2ecc71` | replaces `#34c759`, `#28a745` |
| `--ls-success-tint` | `#e9f9ef` | done-state backgrounds |
| `--ls-danger` | `#e5534b` | replaces `#ff3b30` |
| `--ls-danger-tint` | `#fdeceb` | error backgrounds |
| `--ls-danger-deep` | `#8a2b25` | error banner text |
| `--ls-warning` | `#d4a012` | building state |
| `--ls-warning-tint` | `#fff8e6` | building backgrounds |
| `--ls-topbar-link` | `#d0d5dd` | idle topnav links |
| `--ls-neutral` | `#8b949e` | status dot "checking"/unknown state |
| `--ls-control-off` | `#e9e9eb` | switch track when off |

Shape, type, depth:

- Radii: `--ls-radius-sm: 6px` (chips, inputs, small controls),
  `--ls-radius-md: 8px` (cards, banners, buttons; settings' 12px groups
  move here), `--ls-radius-lg: 12px` (auth modal only; was 14px),
  `--ls-radius-pill: 999px` (switch track).
- Fonts: `--ls-font: system-ui, -apple-system, "Segoe UI", Roboto,
  sans-serif` (dashboard's stack, used everywhere);
  `--ls-font-mono: ui-monospace, SFMono-Regular, "SF Mono", Menlo,
  Consolas, monospace`.
- Type scale: `--ls-fs-xs: 12px`, `--ls-fs-sm: 13.5px` (settings' 12.5px
  and 13px stragglers collapse here; dashboard's `.chip` keeps its 12.5px
  literal and its mobile topnav keeps its 13px literal to honor pixel-identity, like the 18px glyph below),
  `--ls-fs-md: 14px`, `--ls-fs-base: 15px`
  (body; settings moves 14px → 15px), `--ls-fs-lg: 16px`,
  `--ls-fs-xl: 17px`, `--ls-fs-2xl: 20px`. The 18px `.banner-close`
  dismiss glyph stays a literal in its component rule (glyph sizing, not
  text scale).
- Shadows: `--ls-shadow-sm` = `0 4px 14px rgba(27, 33, 40, 0.10)` (card
  hover), `--ls-shadow-lg` = `0 12px 40px rgba(0, 0, 0, 0.22)` (auth
  modal), `--ls-shadow-knob` = `0 2px 4px rgba(0, 0, 0, 0.22)` (switch
  thumb).
- **No spacing tokens** (deliberate YAGNI): paddings/margins stay literal
  in component rules.

## 2. Component stylesheet — `design-system/losos.css`

All component classes from `admin-ui/common.css`,
`admin-ui/dashboard/style.css`, and `admin-ui/settings/style.css` merge
into one token-driven stylesheet. Those three files are then deleted.
Class names survive unchanged, with two cleanups:

- The two near-duplicate `.error-banner` blocks merge. They differ in
  three properties: border color (→ `--ls-danger`), radius
  (→ `--ls-radius-md`), and an explicit `font-size: 14px` that only the
  dashboard version declares. The merged rule pins
  `font-size: var(--ls-fs-md)` so the settings body-size change (14px →
  15px) cannot enlarge its error banner as a side effect.
- Dashboard's `.open` renames to `.btn-outline` (role-named, matches the
  future `<Button variant="outline">`). Verified JS-safe: no script
  references the class; it appears on exactly 4 anchors in
  `dashboard/index.html`.

JS coupling constraint (verified): `settings/app.js` selects `.side-item`,
`.is-active`, `.section`, `.content`, and the pages rely on
`data-state` (`up|down|checking`), `data-kind`
(`building|done|failed`), and `data-section` attribute hooks. All of
these keep their names.

Visible result: dashboard pixel-identical; settings re-skinned to the
Forgejo palette per the approved before/after mockup.

## 3. Repo layout & admin-ui migration

```
design-system/
├── tokens.css
├── losos.css
└── react/              # claude.ai/design only; never in the Nix closure
    ├── package.json    # losos-ds, private; peer: react, react-dom
    ├── tsconfig.json   # dev deps: typescript, esbuild, @types/react
    ├── src/            # index.ts + components/*.tsx
    └── dist/           # ESM bundle + .d.ts (gitignored)
```

Migration edits, all verified against current code:

- **`admin-ui/dashboard/index.html` + `admin-ui/settings/index.html`**:
  each page's `<link rel="stylesheet" href="/common.css">` +
  `<link rel="stylesheet" href="./style.css">` pair becomes
  `<link rel="stylesheet" href="/ds/tokens.css">` +
  `<link rel="stylesheet" href="/ds/losos.css">` (root-absolute works on
  both pages). Script tags untouched. Dashboard additionally renames its
  4 `class="open"` anchors to `class="btn-outline"`.
- **Delete** `admin-ui/common.css`, `admin-ui/dashboard/style.css`,
  `admin-ui/settings/style.css`.
- **`flake/packages.nix`**: the `losos-admin-ui` runCommand additionally
  creates `$out/ds/` and copies `design-system/tokens.css` and
  `design-system/losos.css` into it by explicit path (never
  `cleanSource` of `design-system/`, so `react/` and `node_modules` can
  never leak into the closure).
- **`modules/containers.nix`** (the `losos-front` vhost, :80,
  `root = "${adminUi}/dashboard"`): add
  `"/ds/".alias = "${adminUi}/ds/";` (mirrors the `/settings/` pattern)
  and delete the now-dead `"= /common.css"` alias location.
  `"= /common.js"` stays — only CSS moves.
- **`flake/devshell.nix`**: add `pkgs.nodejs` to `nativeBuildInputs`
  (verified: no node anywhere in the flake today; the react build and
  `/design-sync` need it on the dev machine).
- **`.gitignore`**: add `design-system/react/node_modules` and
  `design-system/react/dist` (`.superpowers/` was already added during
  the design session, in the same commit as this spec).
- **Docs**: CLAUDE.md's "admin SPA on :8081" is stale (the SPA rides the
  :80 front vhost); correct it while documenting `design-system/`.

## 4. React layer — `design-system/react`

Thin functional wrappers rendering the exact markup + class names the
production pages already use. No styles of their own; controlled props
only. `import` of the package CSS is the entire styling story.

Components (~20): `Topbar` (brand + nav links), `Container` (dashboard's
page shell), `Grid`, `Layout` (settings' two-pane shell, `.layout`),
`Content` (`.content` pane) + `ContentHeader` (h1 + action slot),
`Section` (`active?` → `.section.is-active` tab panel),
`Card` (title, status slot, children), `StatusDot`
(`state: 'up' | 'down' | 'checking'`), `Chip`, `Button`
(`variant: 'primary' | 'danger' | 'outline'`, `block?`), `Banner`
(`kind: 'building' | 'done' | 'failed'`, embeds the indeterminate bar),
`ErrorBanner` (message, `onClose`), `HintCard`, `SysList`/`SysRow`,
`Sidebar`/`SideItem` (`active?`, `danger?`), `Group`/`Row` (`danger?`,
plus `GroupHint`), `Switch` (`checked`, `onChange`, label), `Input`
(text/number/password) + `Select`, `ProgressCard` (`kind`, log line,
spinner ✓/×), `AuthOverlay`/`AuthCard`.

Build: `esbuild` → ESM bundle with react external, plus
`tsc --emitDeclarationOnly` for the `.d.ts` prop contracts `/design-sync`
consumes. No Storybook — design-sync's package shape with authored
previews. Node exists only on the dev machine; the flake never builds
this package.

## 5. Testing & verification

- `nix build .#losos-admin-ui` — then assert `$out/ds/tokens.css` and
  `$out/ds/losos.css` exist.
- `nix build .#nixosConfigurations.install.config.system.build.toplevel`
  — evaluates the changed nginx vhost (front-door coverage is eval-only
  by design today).
- `nix build .#checks.x86_64-linux.losos-admin-daemon` — must stay green;
  verified it boots only daemon + D-Bus + API :8082, no nginx/SPA, so it
  is unaffected by this change.
- Visual pass: serve both pages locally; dashboard must be
  pixel-identical, settings must match the approved Forgejo re-skin.
- React: `npm run build` (tsc type-check + esbuild) on the dev machine.
  The real proof is the later `/design-sync` run, which renders and
  grades every component preview.

## Out of scope (explicitly)

- A front-door nginx VM test (flagged as optional follow-up; today's
  eval-only coverage is unchanged).
- Dark mode / theming beyond the single Forgejo palette.
- The `/design-sync` run itself — it happens after this is implemented,
  with `.design-sync/config.json` pointing at `design-system/react` as a
  package-shape source.
- Any change to the Haskell backend, daemon API, or page JS behavior.
