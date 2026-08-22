import { Banner } from 'losos-ds';

// Indbar has `display: none` on its own — the stylesheet only reveals it
// inside .banner[data-kind="building"] or .progress-card[data-kind="building"].
// The only truthful preview composes it inside a building-state parent.
export const InsideBuildingBanner = () => (
  <div style={{ width: 420 }}>
    <Banner kind="building" title="Applying settings…" message="nixos-rebuild switch (job 14)" />
  </div>
);
