---
category: Forms
---

Native `<select>` passthrough with the design system's chevron and border applied via the global `select` selector. All native attributes pass through; children are plain `<option>` elements.

```tsx
<Select defaultValue="local">
  <option value="local">Local</option>
  <option value="mesh">Mesh</option>
</Select>
```
