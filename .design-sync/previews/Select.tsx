import { Select } from 'losos-ds';

export const Mode = () => (
  <div style={{ width: 240 }}>
    <Select defaultValue="local">
      <option value="local">Local</option>
      <option value="mesh">Mesh</option>
    </Select>
  </div>
);
