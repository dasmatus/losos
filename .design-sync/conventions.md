# Building with losos-ds

losos is a NixOS home-server appliance and this design system is its admin
UI: one dark topbar, light content on `--ls-bg`, bordered white cards,
Forgejo-flavored. Components are thin React wrappers that render class-named
markup only. **All styling comes from `styles.css`** (design tokens plus
component classes). There is no provider, no theme object, no CSS-in-JS.
Render components directly on a light page and the stylesheet does the rest.
The stylesheet styles bare `input[type="text"|"number"|"password"]` and
`select` globally, so use the `Input`/`Select` wrappers and always set an
input `type`.

## Styling your own layout glue

Don't invent CSS classes; the vocabulary is closed. For spacing and layout
around components use inline styles or your own CSS built from the tokens,
all defined at the top of `styles.css` with the `--ls-*` prefix:

- Color: `--ls-bg` `--ls-surface` `--ls-surface-muted` `--ls-ink`
  `--ls-ink-muted` `--ls-border` `--ls-border-faint` `--ls-accent`
  `--ls-accent-strong` `--ls-success` `--ls-success-tint` `--ls-danger`
  `--ls-danger-tint` `--ls-danger-deep` `--ls-warning` `--ls-warning-tint`
  `--ls-neutral`
- Shape: `--ls-radius-sm` (6px, inputs and chips), `--ls-radius-md` (8px,
  cards and buttons), `--ls-radius-lg` (12px, the auth modal),
  `--ls-radius-pill`
- Type: `--ls-font` and `--ls-font-mono`; sizes `--ls-fs-xs` up to
  `--ls-fs-2xl` (12 to 20px; body is `--ls-fs-base`, 15px)
- Depth: `--ls-shadow-sm` `--ls-shadow-lg`

A few content classes exist for composition inside components:
`row-label` (the left label in a `Row`), `danger-text` and `danger-sub`
(two-line danger row label). `Topbar` nav links are plain `<a>` children.

## Composition rules that bite

- Page shells. A dashboard page stacks `Topbar`, then `Container` holding a
  `HintCard` or `Banner` and a `Grid` of `Card`s. A settings page stacks
  `Topbar`, then `Layout` holding a `Sidebar` of `SideItem`s and a `Content`
  with a `ContentHeader` followed by `Section`s.
- `Section` is invisible unless `active`. Keep exactly one active per view.
- `Indbar` is hidden except inside a `Banner` or `ProgressCard` with
  `kind="building"`; never place it bare. `kind` (`building|done|failed`)
  drives the palette and the `ProgressCard` spinner morph (the ring becomes
  a check mark or a cross).
- `Switch` with `label` fills a whole `Row`. `Button variant="outline"` with
  `href` is the card-footer "Open" link; `variant="danger"` is for
  destructive actions.
- `AuthCard` goes inside `AuthOverlay`, the fixed full-screen dim layer.

## Where the truth lives

Before styling, read `styles.css` (tokens first, then every component class),
each component's `.prompt.md` for usage, and its `.d.ts` for props.

## Idiomatic page

```tsx
<Topbar brand="losos">
  <a href="/nextcloud">Nextcloud</a>
  <a href="/settings">Settings</a>
</Topbar>
<Container>
  <HintCard>This box answers at philae.local — bookmark this page.</HintCard>
  <Grid>
    <Card title="Nextcloud" status={<StatusDot state="up" label="Up" />}>
      <Desc>Files, calendar, and contacts.</Desc>
      <ChipRow><Chip>philae.local/nextcloud</Chip></ChipRow>
      <Button variant="outline" href="/nextcloud">Open</Button>
    </Card>
  </Grid>
</Container>
```
