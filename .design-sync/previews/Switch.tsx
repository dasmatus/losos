import { Group, Row, Switch } from 'losos-ds';

export const On = () => (
  <div style={{ width: 320 }}>
    <Group>
      <Row><Switch checked label="Share my storage" /></Row>
    </Group>
  </div>
);

export const Off = () => (
  <div style={{ width: 320 }}>
    <Group>
      <Row><Switch checked={false} label="Share my storage" /></Row>
    </Group>
  </div>
);

export const Bare = () => (
  <div style={{ width: 60 }}>
    <Switch checked />
  </div>
);
