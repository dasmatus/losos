---
category: Feedback
---

State banner (`div.banner`) for long-running system operations. `kind` drives the palette: `"building"` amber (and reveals the built-in indeterminate progress bar), `"done"` green, `"failed"` red. `title` is bold; the optional `message` renders in small monospace (a log line or error).

```tsx
<Banner kind="building" title="Applying settings…" message="nixos-rebuild switch" />
<Banner kind="failed" title="Rebuild failed" message="builder for '/nix/store/…' failed" />
```
