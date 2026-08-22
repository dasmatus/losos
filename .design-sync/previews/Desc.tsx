import { Desc } from 'losos-ds';

export const ServiceDescription = () => (
  <div style={{ width: 280 }}>
    <Desc>Files, calendar, and contacts for this household.</Desc>
  </div>
);

export const LongerDescription = () => (
  <div style={{ width: 280 }}>
    <Desc>
      Neighborhood mesh storage backed by Tahoe-LAFS — files are erasure-coded
      across the mesh, so no single box holds a full copy.
    </Desc>
  </div>
);
