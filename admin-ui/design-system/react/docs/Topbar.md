---
category: Chrome
---

The dark application bar across the very top of every page. Renders a `header.topbar` with the brand link on the left and a `nav.topnav` slot on the right; children should be plain `<a>` elements (the topnav styles them as muted links that brighten on hover).

```tsx
<Topbar brand="losos">
  <a href="/nextcloud">Nextcloud</a>
  <a href="/forgejo">Forgejo</a>
  <a href="/settings">Settings</a>
</Topbar>
```
