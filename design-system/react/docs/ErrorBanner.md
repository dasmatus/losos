---
category: Feedback
---

Dismissible error strip (`div.error-banner`) in the danger palette for request-level failures ("Wrong token", "lososd unreachable"). Children are the message; pass `onClose` to show the × dismiss button.

```tsx
<ErrorBanner onClose={() => {}}>Wrong admin token — check the sticker on the box.</ErrorBanner>
```
