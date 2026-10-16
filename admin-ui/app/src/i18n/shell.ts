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
  "shell.signIn.title": { en: "Sign in to this box", sk: "Prihláste sa do tohto zariadenia", de: "An dieser Box anmelden" },
  "shell.signIn.authHint": {
    en: "Sign in with your name and password. Each person has their own, and what you can change here depends on yours.",
    sk: "Prihláste sa svojím menom a heslom. Každý má vlastné a to, čo tu môžete meniť, závisí od vášho účtu.",
    de: "Melde dich mit deinem Namen und Passwort an. Jede Person hat ein eigenes, und was du hier ändern kannst, hängt von deinem ab.",
  },
  "shell.signIn.go": { en: "Sign in", sk: "Prihlásiť sa", de: "Anmelden" },
  "shell.signIn.useSignIn": {
    en: "Sign in with your name instead",
    sk: "Prihlásiť sa namiesto toho menom",
    de: "Stattdessen mit deinem Namen anmelden",
  },
  "shell.signIn.firstTitle": {
    en: "Make your sign-in",
    sk: "Vytvorte si prihlásenie",
    de: "Lege deine Anmeldung an",
  },
  "shell.signIn.firstHint": {
    en: "This box was set up before people signed in with their own name. Choose yours and a password; you become its first administrator. The spare admin key proves the box is yours.",
    sk: "Toto zariadenie bolo nastavené skôr, než sa ľudia prihlasovali vlastným menom. Zvoľte si meno a heslo; stanete sa jeho prvým správcom. Náhradný správcovský kľúč dokazuje, že zariadenie je vaše.",
    de: "Diese Box wurde eingerichtet, bevor sich Personen mit eigenem Namen anmeldeten. Wähle deinen Namen und ein Passwort; du wirst ihr erster Administrator. Der Ersatz-Admin-Schlüssel beweist, dass die Box dir gehört.",
  },
  "shell.signIn.nameLabel": { en: "Sign-in name", sk: "Prihlasovacie meno", de: "Anmeldename" },
  "shell.signIn.newPassword": { en: "Password", sk: "Heslo", de: "Passwort" },
  "shell.signIn.make": { en: "Make it", sk: "Vytvoriť", de: "Anlegen" },
  "shell.signIn.made": { en: "Your sign-in is ready", sk: "Prihlásenie je pripravené", de: "Deine Anmeldung ist bereit" },
  "shell.signIn.madeBody": {
    en: "From now on, sign in with {name} and that password.",
    sk: "Odteraz sa prihlasujte menom {name} a týmto heslom.",
    de: "Ab jetzt meldest du dich mit {name} und diesem Passwort an.",
  },
  "shell.signIn.notMade": {
    en: "The box did not make that sign-in.",
    sk: "Zariadenie toto prihlásenie nevytvorilo.",
    de: "Die Box hat diese Anmeldung nicht angelegt.",
  },
  "shell.signIn.useKey": {
    en: "Use the spare admin key instead",
    sk: "Použiť namiesto toho náhradný správcovský kľúč",
    de: "Stattdessen den Ersatz-Admin-Schlüssel verwenden",
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
  // The warning a box installed without a TPM shows (components/no-tpm-notice.tsx).
  "ui.noTpm.title": {
    en: "This box has no TPM chip",
    sk: "Toto zariadenie nemá čip TPM",
    de: "Diese Box hat keinen TPM-Chip",
  },
  "ui.noTpm.body": {
    en: "Its disk key sits on the unencrypted boot partition, because there is no chip to keep it in. Anyone who gets hold of the disk can read your files with it. To move the key into a chip, switch the TPM on in the firmware and reinstall.",
    sk: "Kľúč k disku leží na nešifrovanom zavádzacom oddiele, lebo nie je čip, ktorý by ho uchoval. Ktokoľvek, kto získa disk, si ním prečíta vaše súbory. Ak chcete kľúč presunúť do čipu, zapnite TPM vo firmvéri a nainštalujte zariadenie znova.",
    de: "Der Schlüssel zur Festplatte liegt auf der unverschlüsselten Boot-Partition, weil kein Chip da ist, der ihn verwahrt. Wer die Festplatte in die Hände bekommt, kann damit deine Dateien lesen. Um den Schlüssel in einen Chip zu verlegen, schalte das TPM in der Firmware ein und installiere die Box neu.",
  },
  "ui.noTpm.link": { en: "What a TPM does", sk: "Čo robí TPM", de: "Was ein TPM tut" },
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
