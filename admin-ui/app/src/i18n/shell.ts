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
  "shell.signIn.hint": {
    en: "Paste the admin key. It is printed on the box and never leaves it.",
    sk: "Vložte správcovský kľúč. Je vytlačený na zariadení a nikdy ho neopustí.",
    de: "Füge den Admin-Schlüssel ein. Er ist auf der Box aufgedruckt und verlässt sie nie.",
  },
  "shell.signIn.label": { en: "Admin key", sk: "Správcovský kľúč", de: "Admin-Schlüssel" },
  "shell.signIn.remembered": {
    en: "The key is remembered until this tab is closed.",
    sk: "Kľúč si zapamätáme, kým túto kartu nezatvoríte.",
    de: "Der Schlüssel bleibt gespeichert, bis du diesen Tab schließt.",
  },
  "shell.signIn.checking": { en: "Checking the key", sk: "Overujeme kľúč", de: "Schlüssel wird geprüft" },
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
