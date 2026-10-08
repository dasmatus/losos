import { defineMessages } from "./define";

/* The shell (App.tsx) and the shared ui/ primitives. */
export default defineMessages({
  "shell.nav.overview": { en: "Overview", sk: "Prehľad", de: "Übersicht" },
  "shell.nav.apps": { en: "Apps", sk: "Aplikácie", de: "Apps" },
  "shell.nav.storage": { en: "Storage", sk: "Úložisko", de: "Speicher" },
  "shell.nav.mesh": { en: "Mesh", sk: "Mesh", de: "Mesh" },
  /* LosOS Lab, the setup visualizer at /lab/ (admin-ui/lab/). */
  "shell.nav.lab": { en: "Lab", sk: "Laboratórium", de: "Labor" },
  "shell.nav.settings": { en: "Settings", sk: "Nastavenia", de: "Einstellungen" },
  "shell.nav.label": { en: "Sections", sk: "Sekcie", de: "Bereiche" },
  /* Under Market in the sidebar, greyed with it: the disk-sharing switch
   * lives on the Market pane and opens when the market does. */
  "shell.nav.diskSharing": { en: "Disk sharing", sk: "Zdieľanie disku", de: "Festplattenfreigabe" },
  "shell.nav.collapse": { en: "Collapse the sidebar", sk: "Zbaliť bočný panel", de: "Seitenleiste einklappen" },
  "shell.nav.expand": { en: "Expand the sidebar", sk: "Rozbaliť bočný panel", de: "Seitenleiste ausklappen" },
  /* The fold toggle on an entry with sub-entries; aria-expanded says which way. */
  "shell.nav.subEntries": { en: "Entries under {name}", sk: "Položky pod {name}", de: "Einträge unter {name}" },
  "shell.asking": {
    en: "Asking this box whether it has been set up.",
    sk: "Zisťujeme, či je toto zariadenie už nastavené.",
    de: "Frage die Box, ob sie schon eingerichtet ist.",
  },
  "shell.thisBox": { en: "this box", sk: "toto zariadenie", de: "diese Box" },
  "shell.logoTip": {
    en: "Losos is Slovak for salmon. The logo is a live coho salmon, photographed underwater.",
    sk: "LosOS je pomenovaný po lososovi. Logo je živý losos kisuč, odfotený pod vodou.",
    de: "Losos ist Slowakisch für Lachs. Das Logo ist ein lebender Silberlachs, unter Wasser fotografiert.",
  },
  "shell.logoTipHalloween": {
    en: "Losos is Slovak for salmon. For Halloween, the logo is a plate of it.",
    sk: "LosOS je pomenovaný po lososovi. Na Halloween je logo tanier lososa.",
    de: "Losos ist Slowakisch für Lachs. An Halloween ist das Logo ein Teller davon.",
  },
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
  "ui.whatToDo": { en: "What to do", sk: "Čo robiť", de: "Was tun" },
  "ui.notifications": { en: "Notifications", sk: "Upozornenia", de: "Benachrichtigungen" },
  "ui.confirmations": { en: "Confirmations", sk: "Potvrdenia", de: "Bestätigungen" },
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

  // ── Toasts the shell raises ───────────────────────────────────────────────
  "shell.signIn.unlocked": { en: "Unlocked", sk: "Odomknuté", de: "Entsperrt" },
  "shell.signIn.unlockedBody": {
    en: "The admin pages are open in this tab until you close it.",
    sk: "Správcovské stránky sú v tejto karte otvorené, kým ju nezavriete.",
    de: "Die Admin-Seiten sind in diesem Tab geöffnet, bis du ihn schließt.",
  },
});
