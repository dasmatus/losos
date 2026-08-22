---
category: Forms
---

Token-unlock modal card (`form.auth-card`, 340px, large radius + shadow): `title` heading, muted `hint`, children (a `<label>` + `Input`), an `error` line when the token was wrong, and a submit `Button`. `onSubmit` fires on form submit with `preventDefault` already handled. Compose inside `AuthOverlay`.

```tsx
<AuthCard title="Admin access" hint="Paper sticker, underside of the box." error="Wrong token." onSubmit={unlock}>
  <label>Admin token</label>
  <Input type="password" autoFocus />
  <Button block type="submit">Unlock</Button>
</AuthCard>
```
