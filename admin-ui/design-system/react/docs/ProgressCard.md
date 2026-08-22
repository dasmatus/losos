---
category: Feedback
---

Rebuild progress card (`div.progress-card`): a `Spinner` (auto-morphing to ✓/× via `kind`), bold `title`, optional monospace `log` line, and the built-in progress bar while `kind="building"`. Palette follows `kind` like `Banner`.

```tsx
<ProgressCard kind="building" title="Switching to Mesh mode" log="activating unit tahoe-shared…" />
<ProgressCard kind="done" title="Settings applied" />
```
