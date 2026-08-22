import { AuthCard, Input, Button } from 'losos-ds';

export const Normal = () => (
  <div style={{ width: 340 }}>
    <AuthCard title="Admin access" hint="Enter the token printed on the sticker inside the case.">
      <label>Admin token</label>
      <Input type="password" placeholder="Admin token" />
      <Button block type="submit">Unlock</Button>
    </AuthCard>
  </div>
);

export const Error = () => (
  <div style={{ width: 340 }}>
    <AuthCard
      title="Admin access"
      hint="Enter the token printed on the sticker inside the case."
      error="Wrong token — check the sticker on the box."
    >
      <label>Admin token</label>
      <Input type="password" placeholder="Admin token" />
      <Button block type="submit">Unlock</Button>
    </AuthCard>
  </div>
);
