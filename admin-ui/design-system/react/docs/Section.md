---
category: Chrome
---

A show/hide panel (`section.section`) for tabbed settings pages driven by `Sidebar`. Hidden unless `active`; without it the `Section` renders nothing visible. Give it an `id` matching the `SideItem` href.

```tsx
<Section id="general" active>
  <Group>…rows…</Group>
</Section>
```
