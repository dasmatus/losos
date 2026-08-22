import { Chip, SysList, SysRow } from 'losos-ds';

export const TextValue = () => (
  <div style={{ width: 340 }}>
    <SysList>
      <SysRow label="Hostname">philae.local</SysRow>
    </SysList>
  </div>
);

export const ChipValue = () => (
  <div style={{ width: 340 }}>
    <SysList>
      <SysRow label="NixOS generation">
        <Chip>247</Chip>
      </SysRow>
    </SysList>
  </div>
);
