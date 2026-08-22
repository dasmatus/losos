import { Banner } from 'losos-ds';

export const Building = () => (
  <div style={{ width: 420 }}>
    <Banner kind="building" title="Applying settings…" message="nixos-rebuild switch (job 14)" />
  </div>
);

export const Done = () => (
  <div style={{ width: 420 }}>
    <Banner kind="done" title="Settings applied" />
  </div>
);

export const Failed = () => (
  <div style={{ width: 420 }}>
    <Banner
      kind="failed"
      title="Rebuild failed"
      message="builder for '/nix/store/…-nextcloud.drv' failed with exit code 1"
    />
  </div>
);
