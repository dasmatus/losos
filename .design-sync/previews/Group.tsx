import { Group, Row, Select, Switch, Button } from 'losos-ds';

export const Settings = () => (
  <div style={{ width: 420 }}>
    <Group>
      <Row><Switch checked label="Share my storage" /></Row>
      <Row>
        <span className="row-label">Mode</span>
        <Select defaultValue="local">
          <option value="local">Local</option>
          <option value="mesh">Mesh</option>
        </Select>
      </Row>
      <Row>
        <span className="row-label">Admin token</span>
        <Button variant="outline">Reveal</Button>
      </Row>
    </Group>
  </div>
);

export const Danger = () => (
  <div style={{ width: 420 }}>
    <Group danger>
      <Row>
        <span className="row-label">Factory reset</span>
        <Button variant="danger">Reset…</Button>
      </Row>
    </Group>
  </div>
);
