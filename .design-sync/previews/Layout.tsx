import { Content, ContentHeader, Desc, Layout, Sidebar, SideItem } from 'losos-ds';

export const SettingsShell = () => (
  <Layout>
    <Sidebar>
      <SideItem href="#general" active>General</SideItem>
      <SideItem href="#nextcloud">Nextcloud</SideItem>
      <SideItem href="#tahoe">Tahoe-LAFS</SideItem>
      <SideItem href="#danger" danger>Factory reset</SideItem>
    </Sidebar>
    <Content>
      <ContentHeader title="General" />
      <Desc>Local ↔ Mesh toggle, hostname, and TPM unlock status for philae.local.</Desc>
    </Content>
  </Layout>
);
