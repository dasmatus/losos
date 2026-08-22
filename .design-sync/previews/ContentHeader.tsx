import { ContentHeader, StatusDot } from 'losos-ds';

export const WithStatus = () => (
  <div style={{ width: 560 }}>
    <ContentHeader title="Services">
      <StatusDot state="up" label="lososd running" />
    </ContentHeader>
  </div>
);

export const TitleOnly = () => (
  <div style={{ width: 420 }}>
    <ContentHeader title="General" />
  </div>
);
