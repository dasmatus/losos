/* A person's colour and initials, the same on every page that shows them.
 *
 * Six colours (tokens.css, `--person-<accent>`). Until someone picks one,
 * their name picks it, so the same person has the same colour in a row of
 * faces, beside their cursor and on every box that shows them. */

export const ACCENTS = ["teal", "blue", "violet", "rose", "amber", "green"] as const;
export type Accent = (typeof ACCENTS)[number];

/** The colour a name gives: FNV-1a over its UTF-16 units. */
export function accentFor(name: string): Accent {
  let h = 0x811c9dc5;
  for (let i = 0; i < name.length; i++) {
    h ^= name.charCodeAt(i);
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return ACCENTS[h % ACCENTS.length] ?? "teal";
}

/** The first letter of the first and last word. */
export function initials(name: string): string {
  const words = name.trim().split(/\s+/).filter(Boolean);
  const first = words[0]?.[0] ?? "?";
  const last = words.length > 1 ? (words[words.length - 1]?.[0] ?? "") : "";
  return (first + last).toUpperCase();
}

/* Static class names, so Tailwind finds every one. */
export const STRONG: Record<Accent, string> = {
  teal: "text-[var(--person-teal)]",
  blue: "text-[var(--person-blue)]",
  violet: "text-[var(--person-violet)]",
  rose: "text-[var(--person-rose)]",
  amber: "text-[var(--person-amber)]",
  green: "text-[var(--person-green)]",
};

export const WASH: Record<Accent, string> = {
  teal: "bg-[var(--person-teal-wash)]",
  blue: "bg-[var(--person-blue-wash)]",
  violet: "bg-[var(--person-violet-wash)]",
  rose: "bg-[var(--person-rose-wash)]",
  amber: "bg-[var(--person-amber-wash)]",
  green: "bg-[var(--person-green-wash)]",
};

export const RING: Record<Accent, string> = {
  teal: "ring-[var(--person-teal)]",
  blue: "ring-[var(--person-blue)]",
  violet: "ring-[var(--person-violet)]",
  rose: "ring-[var(--person-rose)]",
  amber: "ring-[var(--person-amber)]",
  green: "ring-[var(--person-green)]",
};
