// Browser showcase mounted by tests/browser.test.mjs. Renders one instance of
// every visual state the computed-style assertions read, with the same content
// vocabulary as the appliance pages.
import { createRoot } from 'react-dom/client';
import {
  AuthCard, Banner, Button, Card, Chip, ChipRow, Container, Content,
  ContentHeader, Desc, ErrorBanner, Grid, Group, GroupHint, HintCard, Input,
  Layout, ProgressCard, Row, Section, Select, Sidebar, SideItem, StatusDot,
  Switch, SysList, SysRow, Topbar,
} from '../src/index';

function Showcase() {
  return (
    <>
      <Topbar brand="losos">
        <a href="#nextcloud">Nextcloud</a>
        <a href="#forgejo">Forgejo</a>
        <a href="#settings">Settings</a>
      </Topbar>
      <Container>
        <HintCard>This box answers at philae.local — bookmark this page.</HintCard>
        <Banner kind="building" title="Applying settings…" message="nixos-rebuild switch (job 14)" />
        <ErrorBanner onClose={() => {}}>Wrong admin token — check the sticker on the box.</ErrorBanner>
        <ProgressCard kind="done" title="Settings applied" />
        <Grid>
          <Card title="Nextcloud" status={<StatusDot state="up" label="Up" />}>
            <Desc>Files, calendar, and contacts for this household.</Desc>
            <ChipRow><Chip>philae.local/nextcloud</Chip></ChipRow>
            <Button variant="outline" href="#nextcloud">Open</Button>
          </Card>
          <Card title="Forgejo" status={<StatusDot state="down" label="Down" />}>
            <Desc>Git hosting for the projects that stay home.</Desc>
          </Card>
        </Grid>
      </Container>
      <Layout>
        <Sidebar>
          <SideItem active href="#general">General</SideItem>
          <SideItem href="#sharing">Sharing</SideItem>
          <SideItem danger href="#reset">Factory reset</SideItem>
        </Sidebar>
        <Content>
          <ContentHeader title="Settings">
            <StatusDot state="checking" label="Checking…" />
          </ContentHeader>
          <Section id="general" active>
            <Group>
              <Row><Switch label="Share my storage" checked /></Row>
              <Row>
                <span className="row-label">Mode</span>
                <Select defaultValue="local">
                  <option value="local">Local</option>
                  <option value="mesh">Mesh</option>
                </Select>
              </Row>
              <Row>
                <span className="row-label">Tahoe web port</span>
                <Input type="number" defaultValue={3456} />
              </Row>
            </Group>
            <GroupHint>Mesh mode announces this box to the Tahoe introducer.</GroupHint>
            <Button onClick={() => {}}>Apply</Button>
          </Section>
        </Content>
      </Layout>
      <SysList>
        <SysRow label="Hostname"><Chip>philae.local</Chip></SysRow>
        <SysRow label="Disk used">41 GB of 460 GB</SysRow>
      </SysList>
      <AuthCard title="Admin access" hint="Paper sticker, underside of the box.">
        <label>Admin token</label>
        <Input type="password" />
        <Button block>Unlock</Button>
      </AuthCard>
    </>
  );
}

createRoot(document.getElementById('root')!).render(<Showcase />);
