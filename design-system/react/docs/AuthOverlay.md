---
category: Forms
---

Full-screen dimmed layer (`div.auth-overlay`, `position: fixed`) that centers its child, always an `AuthCard`. Render it conditionally at the app root while the admin session is locked.

```tsx
{!unlocked && (
  <AuthOverlay>
    <AuthCard title="Admin access" hint="Enter the token from the sticker.">…</AuthCard>
  </AuthOverlay>
)}
```
