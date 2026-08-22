import { Button, Card, Chip, ChipRow, Desc, StatusDot } from 'losos-ds';

export const ServiceUp = () => (
  <div style={{ width: 280 }}>
    <Card title="Nextcloud" status={<StatusDot state="up" label="Up" />}>
      <Desc>Files, calendar, and contacts for this household.</Desc>
      <ChipRow><Chip>philae.local/nextcloud</Chip></ChipRow>
      <Button variant="outline" href="#nextcloud">Open</Button>
    </Card>
  </div>
);

export const ServiceDown = () => (
  <div style={{ width: 280 }}>
    <Card title="Forgejo" status={<StatusDot state="down" label="Down" />}>
      <Desc>Git hosting for the projects that stay home.</Desc>
      <ChipRow><Chip>philae.local/forgejo</Chip></ChipRow>
      <Button variant="outline" href="#forgejo">Open</Button>
    </Card>
  </div>
);

export const Minimal = () => (
  <div style={{ width: 280 }}>
    <Card title="Tahoe-LAFS" status={<StatusDot state="checking" label="Checking…" />}>
      <Desc>Neighborhood mesh storage.</Desc>
    </Card>
  </div>
);
