import { Content, ContentHeader, Desc, Group, Row, Switch, StatusDot } from 'losos-ds';

export const SettingsColumn = () => (
  <div style={{ width: 560 }}>
    <Content>
      <ContentHeader title="Services">
        <StatusDot state="up" label="lososd running" />
      </ContentHeader>
      <Desc>Nextcloud, Forgejo, and Tahoe-LAFS run as containers on philae.local.</Desc>
      <Group>
        <Row><Switch checked label="Nextcloud" /></Row>
        <Row><Switch checked label="Forgejo" /></Row>
      </Group>
    </Content>
  </div>
);

export const Narrow = () => (
  <div style={{ width: 320 }}>
    <Content>
      <ContentHeader title="General" />
      <Desc>Host name: philae.local</Desc>
    </Content>
  </div>
);
