import { ErrorBanner } from 'losos-ds';

export const WithDismiss = () => (
  <div style={{ width: 360 }}>
    <ErrorBanner onClose={() => {}}>
      Wrong admin token — check the sticker on the box.
    </ErrorBanner>
  </div>
);

export const WithoutDismiss = () => (
  <div style={{ width: 360 }}>
    <ErrorBanner>
      Could not reach lososd on 127.0.0.1:8082 — is the daemon running?
    </ErrorBanner>
  </div>
);
