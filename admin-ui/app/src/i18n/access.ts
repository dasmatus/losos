import { defineMessages } from "./define";

/* The Access tokens pane (screens/settings/pane-access.tsx): tokens for the
 * `losos` terminal client, each tied to a GPG key the installer seeded. The
 * pane's name, summary and search words live under settings.panes.access.*
 * with the other panes'. */

export default defineMessages({
  "access.loading": {
    en: "Loading access tokens…",
    sk: "Načítavam prístupové tokeny…",
    de: "Lade Zugriffstokens…",
  },
  "access.unavailable": {
    en: "This box does not offer terminal access yet.",
    sk: "Toto zariadenie zatiaľ neponúka prístup z terminálu.",
    de: "Diese Box bietet noch keinen Zugriff per Terminal.",
  },
  "access.failed": {
    en: "Could not load the access tokens.",
    sk: "Prístupové tokeny sa nepodarilo načítať.",
    de: "Die Zugriffstokens konnten nicht geladen werden.",
  },
  "access.retry": { en: "Try again", sk: "Skúsiť znova", de: "Erneut versuchen" },

  // ── This box's key ──────────────────────────────────────────────────────
  "access.host.title": { en: "This box's key", sk: "Kľúč tohto zariadenia", de: "Schlüssel dieser Box" },
  "access.host.fingerprint": { en: "Fingerprint", sk: "Odtlačok", de: "Fingerabdruck" },
  "access.host.caption": {
    en: "The first time the terminal client connects, check that it shows this same fingerprint.",
    sk: "Keď sa terminálový klient pripojí prvýkrát, skontrolujte, že ukazuje rovnaký odtlačok.",
    de: "Wenn sich der Terminal-Client zum ersten Mal verbindet, prüf, dass er denselben Fingerabdruck zeigt.",
  },

  // ── Seeded keys ─────────────────────────────────────────────────────────
  "access.keys.title": { en: "Seeded GPG keys", sk: "Vložené kľúče GPG", de: "Hinterlegte GPG-Schlüssel" },
  "access.keys.caption": {
    en: "The installer set these keys, and nothing on this page can change them.",
    sk: "Tieto kľúče nastavil inštalátor a nič na tejto stránke ich nezmení.",
    de: "Diese Schlüssel hat das Installationsprogramm gesetzt, und auf dieser Seite lässt sich nichts daran ändern.",
  },
  "access.keys.noUserId": { en: "No user ID", sk: "Bez ID používateľa", de: "Keine Benutzerkennung" },
  "access.keys.created": { en: "Created {date}", sk: "Vytvorený {date}", de: "Erstellt am {date}" },
  "access.keys.emptyTitle": {
    en: "No key was seeded at install",
    sk: "Pri inštalácii nebol vložený žiadny kľúč",
    de: "Bei der Installation wurde kein Schlüssel hinterlegt",
  },
  "access.keys.emptyBody": {
    en: "Every token is tied to a GPG key the installer seeds. This box has none, so it cannot make tokens.",
    sk: "Každý token je viazaný na kľúč GPG, ktorý vkladá inštalátor. Toto zariadenie žiadny nemá, takže tokeny vytvoriť nemôže.",
    de: "Jedes Token ist an einen GPG-Schlüssel gebunden, den das Installationsprogramm hinterlegt. Diese Box hat keinen und kann daher keine Tokens erstellen.",
  },
  "access.keys.emptyLink": {
    en: "How to seed a key",
    sk: "Ako vložiť kľúč",
    de: "Wie man einen Schlüssel hinterlegt",
  },

  // ── Tokens ──────────────────────────────────────────────────────────────
  "access.scope.read": { en: "Read only", sk: "Iba čítanie", de: "Nur lesen" },
  "access.scope.full": { en: "Full access", sk: "Plný prístup", de: "Voller Zugriff" },
  "access.scope.agent": { en: "AI agents", sk: "Agenti AI", de: "KI-Agenten" },
  "access.tokens.title": { en: "Tokens", sk: "Tokeny", de: "Tokens" },
  "access.tokens.none": { en: "No tokens yet.", sk: "Zatiaľ žiadne tokeny.", de: "Noch keine Tokens." },
  "access.tokens.key": { en: "Key", sk: "Kľúč", de: "Schlüssel" },
  "access.tokens.created": { en: "Created", sk: "Vytvorený", de: "Erstellt" },
  "access.tokens.expires": { en: "Expires", sk: "Platí do", de: "Läuft ab" },
  "access.tokens.lastUsed": { en: "Last used", sk: "Naposledy použitý", de: "Zuletzt benutzt" },
  "access.tokens.never": { en: "Never", sk: "Nikdy", de: "Nie" },
  "access.tokens.neverUsed": { en: "Never used", sk: "Ešte nepoužitý", de: "Nie benutzt" },
  "access.tokens.expired": { en: "Expired", sk: "Vypršal", de: "Abgelaufen" },
  "access.tokens.revoke": { en: "Revoke", sk: "Zrušiť", de: "Widerrufen" },
  "access.tokens.caption": {
    en: "A token works only on requests signed by the key it is tied to. Revoke any token you no longer use.",
    sk: "Token funguje iba pri požiadavkách podpísaných kľúčom, na ktorý je viazaný. Zrušte každý token, ktorý už nepoužívate.",
    de: "Ein Token gilt nur für Anfragen, die mit seinem Schlüssel signiert sind. Widerruf jedes Token, das du nicht mehr benutzt.",
  },
  "access.tokens.dialogTitle": {
    en: "Revoke {name}?",
    sk: "Zrušiť token {name}?",
    de: "{name} widerrufen?",
  },
  "access.tokens.dialogBody": {
    en: "Whatever uses this token loses access at once. You cannot undo this.",
    sk: "Čokoľvek tento token používa, okamžite stratí prístup. Nedá sa to vrátiť.",
    de: "Was dieses Token benutzt, verliert sofort den Zugriff. Das lässt sich nicht rückgängig machen.",
  },
  "access.tokens.keep": { en: "Keep it", sk: "Ponechať", de: "Behalten" },
  "access.tokens.confirm": { en: "Revoke", sk: "Zrušiť", de: "Widerrufen" },
  "access.tokens.revoked": {
    en: "Revoked {name}",
    sk: "Token {name} je zrušený",
    de: "{name} widerrufen",
  },
  "access.tokens.revokeFailed": {
    en: "The token was not revoked",
    sk: "Token sa nepodarilo zrušiť",
    de: "Das Token wurde nicht widerrufen",
  },

  // ── New token ───────────────────────────────────────────────────────────
  "access.form.title": { en: "New token", sk: "Nový token", de: "Neues Token" },
  "access.form.name": { en: "Name", sk: "Názov", de: "Name" },
  "access.form.namePlaceholder": { en: "laptop", sk: "notebook", de: "Laptop" },
  "access.form.key": { en: "Key", sk: "Kľúč", de: "Schlüssel" },
  "access.form.access": { en: "Access", sk: "Prístup", de: "Zugriff" },
  "access.form.readDetail": {
    en: "Can read, but cannot change anything.",
    sk: "Môže čítať, ale nič nezmení.",
    de: "Kann lesen, aber nichts ändern.",
  },
  "access.form.agentDetail": {
    en: "For the LosOS companion in your AI tools. Reads files in LosOS Cloud and repositories in LosOS Git, and nothing else.",
    sk: "Pre pomocníka LosOS vo vašich nástrojoch AI. Číta súbory v LosOS Cloud a repozitáre v LosOS Git a nič iné.",
    de: "Für den LosOS-Begleiter in deinen KI-Werkzeugen. Liest Dateien in LosOS Cloud und Repositories in LosOS Git, sonst nichts.",
  },
  "access.form.fullDetail": {
    en: "Can change what this page can change, except tokens.",
    sk: "Môže meniť to, čo táto stránka, okrem tokenov.",
    de: "Kann ändern, was diese Seite ändern kann, außer Tokens.",
  },
  "access.form.expires": { en: "Expires", sk: "Platnosť", de: "Läuft ab" },
  "access.form.days30": { en: "In 30 days", sk: "O 30 dní", de: "In 30 Tagen" },
  "access.form.days90": { en: "In 90 days", sk: "O 90 dní", de: "In 90 Tagen" },
  "access.form.days365": { en: "In 1 year", sk: "O 1 rok", de: "In 1 Jahr" },
  "access.form.never": { en: "Never", sk: "Nikdy", de: "Nie" },
  "access.form.create": { en: "Create token", sk: "Vytvoriť token", de: "Token erstellen" },
  "access.form.needsKey": {
    en: "Needs a seeded key.",
    sk: "Potrebuje vložený kľúč.",
    de: "Braucht einen hinterlegten Schlüssel.",
  },
  "access.form.full": {
    en: "This box keeps at most {count} tokens. Revoke one to make another.",
    sk: "Toto zariadenie uchová najviac {count} tokenov. Zrušte jeden, aby ste mohli vytvoriť ďalší.",
    de: "Diese Box behält höchstens {count} Tokens. Widerruf eines, um ein neues zu erstellen.",
  },
  "access.form.caption": {
    en: "This page shows a new token once. The box keeps only its SHA-256 hash.",
    sk: "Nový token táto stránka ukáže iba raz. Zariadenie si uchová iba jeho hash SHA-256.",
    de: "Diese Seite zeigt ein neues Token nur einmal. Die Box behält nur seinen SHA-256-Hash.",
  },
  "access.form.failed": {
    en: "The box did not make the token.",
    sk: "Zariadenie token nevytvorilo.",
    de: "Die Box hat das Token nicht erstellt.",
  },

  // ── The new token, once ─────────────────────────────────────────────────
  "access.created.title": { en: "Token created", sk: "Token je vytvorený", de: "Token erstellt" },
  "access.created.once": {
    en: "Copy it now. This page will not show it again.",
    sk: "Skopírujte ho teraz. Táto stránka ho už znova neukáže.",
    de: "Kopier es jetzt. Diese Seite zeigt es nicht noch einmal.",
  },
  "access.created.copy": { en: "Copy", sk: "Kopírovať", de: "Kopieren" },
  "access.created.copied": { en: "Copied", sk: "Skopírované", de: "Kopiert" },
  "access.created.copyFailed": {
    en: "Could not copy. Select the token and copy it by hand.",
    sk: "Kopírovanie zlyhalo. Označte token a skopírujte ho ručne.",
    de: "Kopieren ging nicht. Markier das Token und kopier es von Hand.",
  },
  "access.created.use": {
    en: "To use it, run this on your computer:",
    sk: "Ak ho chcete použiť, spustite na svojom počítači:",
    de: "Um es zu benutzen, führ das auf deinem Computer aus:",
  },
  "access.created.useAgent": {
    en: "To put the LosOS companion in your AI tools, run this on your computer once losos is signed in to this box, and paste the token when it asks:",
    sk: "Ak chcete pridať pomocníka LosOS do svojich nástrojov AI, spustite na svojom počítači tento príkaz, keď je losos prihlásený k tomuto zariadeniu, a keď si ho vypýta, vložte token:",
    de: "Um den LosOS-Begleiter in deine KI-Werkzeuge einzubauen, führ das auf deinem Computer aus, sobald losos bei dieser Box angemeldet ist, und füg das Token ein, wenn es danach fragt:",
  },
  "access.created.done": { en: "Done", sk: "Hotovo", de: "Fertig" },
});
