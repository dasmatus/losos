import { Sidebar, SideItem } from 'losos-ds';

export const SettingsNav = () => (
  <div style={{ width: 240 }}>
    <Sidebar>
      <SideItem active>General</SideItem>
      <SideItem>Sharing</SideItem>
      <SideItem>Services</SideItem>
      <SideItem danger>Factory reset</SideItem>
    </Sidebar>
  </div>
);

export const ServicesActive = () => (
  <div style={{ width: 240 }}>
    <Sidebar>
      <SideItem>General</SideItem>
      <SideItem>Sharing</SideItem>
      <SideItem active>Services</SideItem>
      <SideItem danger>Factory reset</SideItem>
    </Sidebar>
  </div>
);
