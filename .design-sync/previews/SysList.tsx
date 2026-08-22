import { Chip, SysList, SysRow } from 'losos-ds';

export const SystemFacts = () => (
  <div style={{ width: 340 }}>
    <SysList>
      <SysRow label="Hostname">philae.local</SysRow>
      <SysRow label="Disk used">142 GB / 512 GB</SysRow>
      <SysRow label="Uptime">6 days, 4 hours</SysRow>
      <SysRow label="NixOS generation">
        <Chip>247</Chip>
      </SysRow>
    </SysList>
  </div>
);
