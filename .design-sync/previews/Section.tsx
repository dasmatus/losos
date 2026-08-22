import { Section, ContentHeader, Desc, Group, GroupHint, Row, Switch, Button } from 'losos-ds';

export const SharingActive = () => (
  <div style={{ width: 560 }}>
    <ContentHeader title="Sharing" />
    <Section id="sharing" active>
      <Desc>Share this household's storage with the neighborhood mesh over Tahoe-LAFS.</Desc>
      <Group>
        <Row><Switch checked label="Share my storage" /></Row>
      </Group>
      <GroupHint>Uses spare disk space; does not expose your files.</GroupHint>
    </Section>
  </div>
);

export const DangerZoneActive = () => (
  <div style={{ width: 560 }}>
    <ContentHeader title="Factory reset" />
    <Section id="danger" active>
      <Group danger>
        <Row>
          <Desc>Wipes /persist and reboots to a clean install. This cannot be undone.</Desc>
          <Button variant="danger">Factory reset…</Button>
        </Row>
      </Group>
    </Section>
  </div>
);
