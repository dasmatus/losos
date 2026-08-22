import { Group, Row, Select, GroupHint } from 'losos-ds';

export const MeshHint = () => (
  <div style={{ width: 420 }}>
    <Group>
      <Row>
        <span className="row-label">Mode</span>
        <Select defaultValue="mesh">
          <option value="local">Local</option>
          <option value="mesh">Mesh</option>
        </Select>
      </Row>
    </Group>
    <GroupHint>Mesh mode announces this box to the Tahoe introducer.</GroupHint>
  </div>
);
