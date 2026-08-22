---
category: Forms
---

Bordered container (`div.group`) stacking settings `Row`s separated by hairlines. `danger` tints the border red for destructive groups (factory reset). Follow it with a `GroupHint` for explanatory text.

```tsx
<Group>
  <Row><Switch label="Share my storage" checked /></Row>
  <Row><span className="row-label">Mode</span><Select>…</Select></Row>
</Group>
<GroupHint>Sharing donates spare disk to the neighborhood mesh.</GroupHint>
```
