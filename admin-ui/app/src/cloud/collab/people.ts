/* A person's colour, the same on every page that shows them
 * (docs/profiles.md): the one in their profile when their box can say, else
 * the one their sign-in name gives, worked out the way lososd does, so a
 * person from another box has the colour their own box shows. */

import { ACCENTS, type Accent } from "@/lib/api";

export { ACCENTS, type Accent };

/** The colour a sign-in name gives: FNV-1a over its UTF-8 bytes, as
 *  `Accent::of_name` in backend/src/profiles.rs. */
export function accentFor(name: string): Accent {
  let h = 0x811c9dc5;
  for (const byte of new TextEncoder().encode(name)) {
    h ^= byte;
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return ACCENTS[h % ACCENTS.length] ?? "teal";
}

/** The sign-in name in an address: `eva` in `eva@kitchenbox.local`. */
export function nameOf(address: string): string {
  const at = address.lastIndexOf("@");
  return at < 0 ? address : address.slice(0, at);
}

/** The box in an address: `kitchenbox.local` in `eva@kitchenbox.local`. */
export function boxOf(address: string): string {
  return address.slice(address.lastIndexOf("@") + 1);
}
