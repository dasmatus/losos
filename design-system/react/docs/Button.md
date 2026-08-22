---
category: Forms
---

The action button. `variant` picks the class: `"primary"` solid accent (default), `"danger"` outlined red for destructive actions, `"outline"` full-width ink outline used as the "Open" link on dashboard cards. `block` stretches it full-width. With `href` it renders an `<a>` styled identically; otherwise a `<button>` that passes through native button attributes (`disabled`, `onClick`, …). It defaults to `type="button"`, so inside a form (like `AuthCard`) pass `type="submit"` or clicks won't submit.

```tsx
<Button onClick={apply}>Apply</Button>
<Button variant="danger">Factory reset…</Button>
<Button variant="outline" href="/nextcloud">Open</Button>
```
