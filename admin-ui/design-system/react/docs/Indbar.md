---
category: Feedback
---

Indeterminate progress bar (`div.indbar`). It is `display: none` on its own. The stylesheet only reveals it inside `Banner` or `ProgressCard` when their `kind` is `"building"`, and both of those render it for you. You almost never place `Indbar` yourself; reach for `Banner` or `ProgressCard` instead.
