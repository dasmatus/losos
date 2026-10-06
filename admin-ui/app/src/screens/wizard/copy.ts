/* Copy, by whichever route this browser allows.
 *
 * navigator.clipboard exists only in a secure context, and step 1 of this very
 * wizard is what makes the connection secure — so on the first pass through,
 * over http, it is routinely absent. The fallback selects the element that is
 * already on screen and asks the document to copy the selection: no synthetic
 * textarea to position (which would need a style attribute the CSP refuses)
 * and the owner can see what was taken.
 *
 * Shared by the admin key (step 2) and the recovery code (its step is hidden,
 * see ./steps.ts), the things the wizard asks the owner to take off the
 * screen. */
export async function copyText(text: string, element: HTMLElement | null): Promise<boolean> {
  if (window.isSecureContext && navigator.clipboard !== undefined) {
    try {
      await navigator.clipboard.writeText(text);
      return true;
    } catch {
      /* Permission refused, or no focus. Fall through. */
    }
  }
  return selectAndCopy(element);
}

function selectAndCopy(element: HTMLElement | null): boolean {
  if (element === null) return false;
  const selection = window.getSelection();
  if (selection === null) return false;
  const range = document.createRange();
  range.selectNodeContents(element);
  selection.removeAllRanges();
  selection.addRange(range);
  try {
    // Deprecated, and the only copy this browser has on a plain http page.
    return document.execCommand("copy");
  } catch {
    return false;
  }
}
