import { AuthOverlay, AuthCard, Input, Button } from 'losos-ds';

// The capture harness wraps every cell in a transformed div, which per CSS
// becomes the containing block for position:fixed. Giving the preview its own
// sized transformed wrapper makes the overlay fill THIS box instead — a
// truthful miniature viewport (in the real app the overlay fills the screen).
export const Locked = () => (
  <div style={{ position: 'relative', width: 512, height: 372, transform: 'translateZ(0)', overflow: 'hidden' }}>
    <AuthOverlay>
      <AuthCard title="Admin access" hint="Enter the token printed on the sticker inside the case.">
        <label>Admin token</label>
        <Input type="password" placeholder="Admin token" />
        <Button block>Unlock</Button>
      </AuthCard>
    </AuthOverlay>
  </div>
);
