import { ProgressCard } from 'losos-ds';

export const Building = () => (
  <div style={{ width: 380 }}>
    <ProgressCard
      kind="building"
      title="Applying settings…"
      log="activating unit container@nextcloud…"
    />
  </div>
);

export const Done = () => (
  <div style={{ width: 380 }}>
    <ProgressCard
      kind="done"
      title="Rebuild complete"
      log="activated 4 units, 0 failed"
    />
  </div>
);

export const Failed = () => (
  <div style={{ width: 380 }}>
    <ProgressCard
      kind="failed"
      title="Rebuild failed"
      log="builder for '/nix/store/…-tahoe-lafs.drv' failed"
    />
  </div>
);
