import { Button, Card, ChipRow, Chip, Container, Desc, Grid, HintCard, StatusDot } from 'losos-ds';

export const DashboardShell = () => (
  <div style={{ width: 500 }}>
    <Container>
      <HintCard>Reachable only on the local network at philae.local — no port forwarding needed.</HintCard>
      <Grid>
        <Card title="Nextcloud" status={<StatusDot state="up" label="Up" />}>
          <Desc>Files, calendar, and contacts for this household.</Desc>
          <ChipRow><Chip>philae.local/nextcloud</Chip></ChipRow>
          <Button variant="outline" href="#nextcloud">Open</Button>
        </Card>
        <Card title="Tahoe-LAFS" status={<StatusDot state="checking" label="Checking…" />}>
          <Desc>Neighborhood mesh storage.</Desc>
        </Card>
      </Grid>
    </Container>
  </div>
);
