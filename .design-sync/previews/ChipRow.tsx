import { Chip, ChipRow } from 'losos-ds';

export const ServiceAddresses = () => (
  <div style={{ width: 300 }}>
    <ChipRow>
      <Chip>philae.local/nextcloud</Chip>
      <Chip>philae.local/forgejo</Chip>
    </ChipRow>
  </div>
);

export const SingleAddress = () => (
  <div style={{ width: 260 }}>
    <ChipRow>
      <Chip>philae.local:3456</Chip>
    </ChipRow>
  </div>
);
