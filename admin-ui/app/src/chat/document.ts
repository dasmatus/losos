/* Whether this document was loaded at /chat/, under chat's own policy
 * (modules/containers.nix), which adds 'wasm-unsafe-eval' for the end-to-end
 * crypto and blob: for decrypted media. Kept apart from the chat code so the
 * shell can ask without loading matrix-js-sdk. */
export function chatDocument(): boolean {
  const entry = performance.getEntriesByType("navigation")[0];
  const url = entry !== undefined ? entry.name : window.location.href;
  return new URL(url, window.location.href).pathname.startsWith("/chat/");
}
