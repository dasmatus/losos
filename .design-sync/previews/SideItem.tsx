import { Sidebar, SideItem } from 'losos-ds';

export const States = () => (
  <div style={{ width: 240 }}>
    <Sidebar>
      <SideItem active>General</SideItem>
      <SideItem>Sharing</SideItem>
      <SideItem danger>Factory reset</SideItem>
    </Sidebar>
  </div>
);

export const DangerActive = () => (
  <div style={{ width: 240 }}>
    <Sidebar>
      <SideItem>Services</SideItem>
      <SideItem active danger>Factory reset</SideItem>
    </Sidebar>
  </div>
);
