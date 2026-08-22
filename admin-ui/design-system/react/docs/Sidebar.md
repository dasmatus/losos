---
category: Chrome
---

Settings navigation card (`aside.sidebar` wrapping `nav.side-list`, 240px wide; becomes a horizontal scroller on narrow screens). Children are `SideItem` links.

```tsx
<Sidebar>
  <SideItem active>General</SideItem>
  <SideItem>Sharing</SideItem>
  <SideItem danger>Factory reset</SideItem>
</Sidebar>
```
