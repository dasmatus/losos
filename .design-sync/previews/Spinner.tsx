import { Spinner } from 'losos-ds';

// Standalone the Spinner is a 14px ring — a near-blank screenshot fails the
// render check, so every cell composes it with visible inline text.
export const CheckingServiceHealth = () => (
  <div style={{ width: 260 }}>
    <p style={{ display: 'flex', alignItems: 'center', gap: 8, margin: 0 }}>
      <Spinner /> Checking service health…
    </p>
  </div>
);

export const WaitingOnIntroducer = () => (
  <div style={{ width: 260 }}>
    <p style={{ display: 'flex', alignItems: 'center', gap: 8, margin: 0 }}>
      <Spinner /> Waiting for Tahoe-LAFS introducer…
    </p>
  </div>
);
