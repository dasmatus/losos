/** Links other screens make into chat. Contacts, Mail and Cloud send a
 *  person as `/chat/?with=<name>`, either a whole Matrix ID or a box
 *  sign-in name, since a page outside chat does not know the server name.
 *  LosOS Cloud's share sheet sends `?share=<path>…&text=…` (chat/cloud.tsx). */

/** The characters a Matrix user ID's localpart may hold. */
const LOCALPART = /^[a-z0-9._=\-/+]+$/;

/** The Matrix ID `raw` names on `server`, or null when it names nobody. */
export function withUserId(raw: string | null, server: string): string | null {
  if (raw === null) return null;
  const value = raw.trim();
  if (value.startsWith("@")) {
    const colon = value.indexOf(":");
    if (colon < 2 || colon === value.length - 1) return null;
    return LOCALPART.test(value.slice(1, colon)) && !/\s/.test(value.slice(colon + 1)) ? value : null;
  }
  return LOCALPART.test(value) ? `@${value}:${server}` : null;
}

/** `href` without the parameters chat acts on (`with`, `share`, `text`),
 *  so a reload does not do it again. The others (a call's `call`) stay
 *  for whoever reads them next. */
export function withoutLink(href: string): string {
  const url = new URL(href);
  for (const key of ["with", "share", "text"]) url.searchParams.delete(key);
  return `${url.pathname}${url.search}${url.hash}`;
}
