---
category: Forms
---

iOS-style toggle (`span.switch`, 51×31, green when on). Controlled: pass `checked` and `onChange`. With `label` it wraps itself in a full-width `label.switch-row` (label left, control right) sized to sit alone inside a `Row`; without `label` it renders just the control.

```tsx
<Row><Switch label="Share my storage" checked onChange={setSharing} /></Row>
```
