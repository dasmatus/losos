import { defineMessages } from "./define";

/* The shell (App.tsx) and the shared ui/ primitives. */
export default defineMessages({
  "shell.nav.overview": { en: "Overview", sk: "Prehľad", de: "Übersicht" },
  "shell.nav.apps": { en: "Apps", sk: "Aplikácie", de: "Apps" },
  "shell.nav.storage": { en: "Storage", sk: "Úložisko", de: "Speicher" },
  "shell.nav.mesh": { en: "Mesh", sk: "Mesh", de: "Mesh" },
  "shell.nav.settings": { en: "Settings", sk: "Nastavenia", de: "Einstellungen" },
  "shell.nav.label": { en: "Sections", sk: "Sekcie", de: "Bereiche" },
  "shell.asking": {
    en: "Asking this box whether it has been set up.",
    sk: "Zisťujeme, či je toto zariadenie už nastavené.",
    de: "Frage die Box, ob sie schon eingerichtet ist.",
  },
  "shell.thisBox": { en: "this box", sk: "toto zariadenie", de: "diese Box" },
  "shell.signOut": { en: "Sign out", sk: "Odhlásiť sa", de: "Abmelden" },
  "shell.notFound.title": { en: "Nothing here", sk: "Nič tu nie je", de: "Hier ist nichts" },
  "shell.notFound.body": {
    en: "That address does not match any section of this box's admin page.",
    sk: "Táto adresa nezodpovedá žiadnej časti správy tohto zariadenia.",
    de: "Diese Adresse passt zu keinem Bereich der Verwaltungsseite dieser Box.",
  },
  "shell.signIn.badShape": {
    en: "An admin key is 64 characters, digits and the letters a to f.",
    sk: "Správcovský kľúč má 64 znakov: číslice a písmená a až f.",
    de: "Ein Admin-Schlüssel hat 64 Zeichen: Ziffern und die Buchstaben a bis f.",
  },
  "shell.signIn.rejected": {
    en: "That key was not accepted.",
    sk: "Tento kľúč nebol prijatý.",
    de: "Dieser Schlüssel wurde nicht akzeptiert.",
  },
  "shell.signIn.noAnswer": {
    en: "This box did not answer. Check that it is switched on.",
    sk: "Zariadenie neodpovedá. Skontrolujte, či je zapnuté.",
    de: "Die Box antwortet nicht. Prüfe, ob sie eingeschaltet ist.",
  },
  "shell.signIn.title": { en: "Unlock this box", sk: "Odomknite toto zariadenie", de: "Diese Box entsperren" },
  "shell.signIn.passwordHint": {
    en: "Type your password, the same one you use for LosOS cloud.",
    sk: "Zadajte svoje heslo, to isté, ktorým sa prihlasujete do LosOS cloud.",
    de: "Gib dein Passwort ein, dasselbe wie für LosOS cloud.",
  },
  "shell.signIn.passwordLabel": { en: "Password", sk: "Heslo", de: "Passwort" },
  "shell.signIn.show": { en: "Show the password", sk: "Zobraziť heslo", de: "Passwort anzeigen" },
  "shell.signIn.hide": { en: "Hide the password", sk: "Skryť heslo", de: "Passwort verbergen" },
  "shell.signIn.wrongPassword": {
    en: "That password was not accepted.",
    sk: "Toto heslo nebolo prijaté.",
    de: "Dieses Passwort wurde nicht akzeptiert.",
  },
  "shell.signIn.cloudDown": {
    en: "LosOS cloud is not running, so the password cannot be checked right now. Wait a few minutes, or use the spare admin key from the printed sheet.",
    sk: "LosOS cloud práve nebeží, takže heslo sa teraz nedá overiť. Počkajte pár minút alebo použite náhradný správcovský kľúč z vytlačeného hárku.",
    de: "LosOS cloud läuft gerade nicht, deshalb kann das Passwort im Moment nicht geprüft werden. Warte ein paar Minuten oder nimm den Ersatz-Admin-Schlüssel vom gedruckten Blatt.",
  },
  "shell.signIn.useKey": {
    en: "Use the spare admin key instead",
    sk: "Použiť namiesto toho náhradný správcovský kľúč",
    de: "Stattdessen den Ersatz-Admin-Schlüssel verwenden",
  },
  "shell.signIn.usePassword": {
    en: "Use the password instead",
    sk: "Použiť namiesto toho heslo",
    de: "Stattdessen das Passwort verwenden",
  },
  "shell.signIn.hint": {
    en: "Paste the spare admin key. It was shown once, during setup, and is on the sheet if you printed it.",
    sk: "Vložte náhradný správcovský kľúč. Zobrazil sa raz, počas nastavenia, a je na hárku, ak ste si ho vytlačili.",
    de: "Füge den Ersatz-Admin-Schlüssel ein. Er wurde einmal gezeigt, bei der Einrichtung, und steht auf dem Blatt, falls du es gedruckt hast.",
  },
  "shell.signIn.label": { en: "Spare admin key", sk: "Náhradný správcovský kľúč", de: "Ersatz-Admin-Schlüssel" },
  "shell.signIn.remembered": {
    en: "This tab stays unlocked until it is closed.",
    sk: "Táto karta zostane odomknutá, kým ju nezatvoríte.",
    de: "Dieser Tab bleibt entsperrt, bis er geschlossen wird.",
  },
  "shell.signIn.checking": { en: "Checking", sk: "Overujeme", de: "Wird geprüft" },
  "shell.signIn.unlock": { en: "Unlock", sk: "Odomknúť", de: "Entsperren" },

  "ui.working": { en: "Working", sk: "Pracujeme", de: "Wird ausgeführt" },
  "ui.dismiss": { en: "Dismiss", sk: "Zavrieť", de: "Schließen" },
  "ui.theme.label": { en: "Appearance", sk: "Vzhľad", de: "Erscheinungsbild" },
  "ui.theme.auto": { en: "Match the browser", sk: "Podľa prehliadača", de: "Wie im Browser" },
  "ui.theme.light": { en: "Light", sk: "Svetlý", de: "Hell" },
  "ui.theme.dark": { en: "Dark", sk: "Tmavý", de: "Dunkel" },
  "ui.language.label": { en: "Language", sk: "Jazyk", de: "Sprache" },
  "ui.language.auto": {
    en: "Browser language",
    sk: "Jazyk prehliadača",
    de: "Browsersprache",
  },
});
