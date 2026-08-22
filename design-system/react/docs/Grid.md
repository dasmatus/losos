---
category: Chrome
---

Responsive card grid (`div.grid`, `role="list"`): auto-fill columns of at least 240px, collapsing to two then one column on small screens. Children are `Card` components (each renders `role="listitem"`).

```tsx
<Grid>
  <Card title="Nextcloud" status={<StatusDot state="up" label="Up" />}>…</Card>
  <Card title="Forgejo" status={<StatusDot state="down" label="Down" />}>…</Card>
</Grid>
```
