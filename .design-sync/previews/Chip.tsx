import { Chip, ChipRow } from 'losos-ds';

export const Addresses = () => (
  <div style={{ width: 300 }}>
    <ChipRow>
      <Chip>philae.local/nextcloud</Chip>
      <Chip>philae.local/forgejo</Chip>
      <Chip>philae.local:3456</Chip>
    </ChipRow>
  </div>
);

export const SingleChip = () => (
  <div style={{ width: 220 }}>
    <Chip>philae.local/nextcloud</Chip>
  </div>
);
