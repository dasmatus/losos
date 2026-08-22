import { Button } from 'losos-ds';

export const Primary = () => <Button>Apply</Button>;

export const Danger = () => <Button variant="danger">Factory reset…</Button>;

export const OutlineLink = () => (
  <div style={{ width: 240 }}>
    <Button variant="outline" href="#open">Open</Button>
  </div>
);

export const Disabled = () => <Button disabled>Apply</Button>;

export const Block = () => (
  <div style={{ width: 300 }}>
    <Button block>Unlock</Button>
  </div>
);
