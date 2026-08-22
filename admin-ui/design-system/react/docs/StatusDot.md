---
category: Display
---

Small service-health indicator (`span.status`): a 10px dot colored by `state`, green for `"up"`, red for `"down"`, neutral gray for `"checking"`, with an optional muted `label`. Used in `Card` status slots and `ContentHeader`.

```tsx
<StatusDot state="up" label="Up" />
<StatusDot state="checking" label="Checking…" />
```
