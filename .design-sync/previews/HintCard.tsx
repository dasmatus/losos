import { HintCard } from 'losos-ds';

export const LocalOnly = () => (
  <div style={{ width: 420 }}>
    <HintCard>
      Reachable only on the local network at philae.local — no port forwarding needed.
    </HintCard>
  </div>
);

export const Bookmark = () => (
  <div style={{ width: 420 }}>
    <HintCard>This box answers at philae.local — bookmark this page.</HintCard>
  </div>
);
