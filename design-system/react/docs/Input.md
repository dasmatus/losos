---
category: Forms
---

Native `<input>` passthrough. The stylesheet styles `input[type="text"]` (180px), `input[type="number"]` (100px), and `input[type="password"]` globally, so there is no wrapper class and every native attribute applies. Set `type` explicitly; only these three types receive the styled border and width.

```tsx
<Input type="password" placeholder="Admin token" autoFocus />
```
