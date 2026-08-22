---
category: Chrome
---

One navigation link inside `Sidebar` (`a.side-item`). `active` highlights it with the accent tint. `danger` renders it in the danger color for destructive destinations like factory reset, and combines with `active`. Accepts `href` and `onClick`.

```tsx
<SideItem active href="#general">General</SideItem>
<SideItem danger href="#reset">Factory reset</SideItem>
```
