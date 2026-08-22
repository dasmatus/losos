---
category: Display
---

Dashboard service card (`article.card`, `role="listitem"`): a head row with the `title` (h2) and an optional `status` node (usually `StatusDot`), then free-form children, typically a `Desc`, a `ChipRow`, and an outline `Button` linking to the service.

```tsx
<Card title="Nextcloud" status={<StatusDot state="up" label="Up" />}>
  <Desc>Files, calendar, and contacts for this household.</Desc>
  <ChipRow><Chip>philae.local/nextcloud</Chip></ChipRow>
  <Button variant="outline" href="/nextcloud">Open</Button>
</Card>
```
