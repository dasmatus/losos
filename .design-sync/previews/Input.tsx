import { Input } from 'losos-ds';

export const Text = () => (
  <div style={{ width: 240 }}>
    <Input type="text" defaultValue="philae" />
  </div>
);

export const Password = () => (
  <div style={{ width: 240 }}>
    <Input type="password" placeholder="Admin token" />
  </div>
);

export const Number = () => (
  <div style={{ width: 240 }}>
    <Input type="number" defaultValue={3456} />
  </div>
);
