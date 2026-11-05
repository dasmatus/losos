/* A video call with an assistant, end to end; see fixtures/voice/assistant-call.mjs.
 * Skips itself without LOSOS_VOICE_DIR (nix/voice.nix's output). */

if (!Bun.env.LOSOS_VOICE_DIR) {
  console.log('voice assistant: skipped (LOSOS_VOICE_DIR is not set)');
} else {
  await import('./fixtures/voice/assistant-call.mjs');
}
