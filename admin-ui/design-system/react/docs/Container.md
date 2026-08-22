---
category: Chrome
---

Centered page shell for dashboard-style pages (`main.container`, max-width 1100px). Put it directly under `Topbar` and fill it with page content such as `HintCard`, `Banner`, and `Grid`.

```tsx
<Topbar brand="losos">…</Topbar>
<Container>
  <HintCard>Services run on this box. Bookmark this page.</HintCard>
  <Grid>…cards…</Grid>
</Container>
```
