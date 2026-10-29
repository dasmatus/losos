import type { Credential } from "@/lib/api";
import { t, type MessageKey } from "@/lib/i18n";

/* The words for each credential and each app the Credentials pane groups by.
 *
 * lososd names a credential and says in English what it is for; the page
 * says it in the language on screen. A name the catalogue does not know yet
 * (a module added one after this build) falls back to lososd's sentence, so
 * a new credential is never a blank row. */

const KNOWN = {
  "admin-token": "credentials.name.adminToken",
  "authelia-jwt": "credentials.name.autheliaJwt",
  "authelia-session": "credentials.name.autheliaSession",
  "authelia-storage": "credentials.name.autheliaStorage",
  "mail-master": "credentials.name.mailMaster",
  "backup-bucket": "credentials.name.backupBucket",
  "market-escrow-key": "credentials.name.marketEscrowKey",
  "authelia-oidc-hmac": "credentials.name.autheliaOidcHmac",
  "authelia-oidc-jwks": "credentials.name.autheliaOidcJwks",
  "oidc-lososd": "credentials.name.oidcLososd",
} as const satisfies Record<string, string>;

type KnownName = keyof typeof KNOWN;

function known(name: string): name is KnownName {
  return Object.prototype.hasOwnProperty.call(KNOWN, name);
}

/** The credential's name on the page. */
export function credentialTitle(c: Credential): string {
  return known(c.name) ? t(`${KNOWN[c.name]}.title` as MessageKey) : c.name;
}

/** One line on what it is for. */
export function credentialDetail(c: Credential): string {
  return known(c.name) ? t(`${KNOWN[c.name]}.detail` as MessageKey) : c.description;
}

/** What a new value does, said in the confirmation. */
export function credentialRotateEffect(c: Credential): string {
  return known(c.name) ? t(`${KNOWN[c.name]}.rotate` as MessageKey) : t("credentials.rotate.effect");
}

const APPS = {
  admin: "credentials.app.admin",
  login: "credentials.app.login",
  mail: "credentials.app.mail",
  backup: "credentials.app.backup",
  market: "credentials.app.market",
} as const satisfies Record<string, MessageKey>;

/** The order the groups come in: the box's own way in first. */
export const APP_ORDER: readonly string[] = ["admin", "login", "mail", "backup", "market"];

export function appTitle(app: string): string {
  return Object.prototype.hasOwnProperty.call(APPS, app)
    ? t(APPS[app as keyof typeof APPS])
    : app;
}
