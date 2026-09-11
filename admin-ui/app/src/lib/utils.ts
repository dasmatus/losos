import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

/** The ShadCN class merger: conditional classes in, last-wins Tailwind out. */
export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}

/* Set a CSS custom property on an element through the CSSOM.
 *
 * The escape hatch for a value that is genuinely dynamic — a progress
 * percentage, a measured width. `style={{ ... }}` is banned in this app by
 * house rule under the appliance's `style-src 'self'` CSP; this goes through
 * the CSSOM instead of an inline style attribute, and the stylesheet still
 * owns what the property MEANS. Use it from a ref callback or an effect, and
 * only when a class cannot express the value. */
export function setCssVar(element: HTMLElement | null, name: string, value: string): void {
  element?.style.setProperty(name, value);
}
