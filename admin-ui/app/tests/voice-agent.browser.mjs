/* LosOS in a call, end to end, in Chromium against the real SFU: see
 * tests/fixtures/voice/agent-call.mjs.
 *
 * It needs the speech models, which are 150 MB of nix/voice.nix's output,
 * so it runs when LOSOS_VOICE_DIR names them (the nix check does) and says
 * it skipped when it does not. */

if (Bun.env.LOSOS_VOICE_DIR) await import('./fixtures/voice/agent-call.mjs');
else console.log('voice agent: skipped, LOSOS_VOICE_DIR is not set (nix build .#losos-voice)');
