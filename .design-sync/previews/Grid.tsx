import { Button, Card, Chip, ChipRow, Desc, Grid, StatusDot } from 'losos-ds';

export const ServiceCards = () => (
  <Grid>
    <Card title="Nextcloud" status={<StatusDot state="up" label="Up" />}>
      <Desc>Files, calendar, and contacts for this household.</Desc>
      <ChipRow><Chip>philae.local/nextcloud</Chip></ChipRow>
      <Button variant="outline" href="#nextcloud">Open</Button>
    </Card>
    <Card title="Forgejo" status={<StatusDot state="down" label="Down" />}>
      <Desc>Git hosting for the projects that stay home.</Desc>
      <ChipRow><Chip>philae.local/forgejo</Chip></ChipRow>
      <Button variant="outline" href="#forgejo">Open</Button>
    </Card>
    <Card title="Tahoe-LAFS" status={<StatusDot state="checking" label="Checking…" />}>
      <Desc>Neighborhood mesh storage.</Desc>
      <ChipRow><Chip>:3456</Chip></ChipRow>
    </Card>
  </Grid>
);
