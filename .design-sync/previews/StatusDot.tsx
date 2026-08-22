import { StatusDot } from 'losos-ds';

export const AllStates = () => (
  <div style={{ width: 220, display: 'flex', flexDirection: 'column', gap: 8 }}>
    <StatusDot state="up" label="Up" />
    <StatusDot state="down" label="Down" />
    <StatusDot state="checking" label="Checking…" />
  </div>
);

export const ServiceRow = () => (
  <div style={{ width: 260, display: 'flex', flexDirection: 'column', gap: 6 }}>
    <div style={{ display: 'flex', justifyContent: 'space-between' }}>
      <span>Nextcloud</span>
      <StatusDot state="up" label="Up" />
    </div>
    <div style={{ display: 'flex', justifyContent: 'space-between' }}>
      <span>Forgejo</span>
      <StatusDot state="down" label="Down" />
    </div>
    <div style={{ display: 'flex', justifyContent: 'space-between' }}>
      <span>Tahoe-LAFS</span>
      <StatusDot state="checking" label="Checking…" />
    </div>
  </div>
);
