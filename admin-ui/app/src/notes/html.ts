/** Text as HTML: the five characters that mean something there escaped. */
export function escapeHtml(text: string): string {
  return text.replace(/[&<>"']/g, (c) => `&${{ "&": "amp", "<": "lt", ">": "gt", '"': "quot", "'": "#39" }[c]};`);
}
