import { Group, Row, Select, Input } from 'losos-ds';

export const InGroup = () => (
  <div style={{ width: 420 }}>
    <Group>
      <Row>
        <span className="row-label">Mode</span>
        <Select defaultValue="local">
          <option value="local">Local</option>
          <option value="mesh">Mesh</option>
        </Select>
      </Row>
      <Row>
        <span className="row-label">Admin token</span>
        <Input type="password" placeholder="Admin token" />
      </Row>
    </Group>
  </div>
);
