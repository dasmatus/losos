---
category: Chrome
---

Two-column settings shell (`div.layout`, max-width 880px): a fixed-width `Sidebar` on the left and flexible `Content` on the right. Stacks vertically on narrow screens. Use it directly under `Topbar` for settings-style pages.

```tsx
<Layout>
  <Sidebar>
    <SideItem active>General</SideItem>
    <SideItem>Sharing</SideItem>
  </Sidebar>
  <Content>…sections…</Content>
</Layout>
```
