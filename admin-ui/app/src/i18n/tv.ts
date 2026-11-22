import { defineMessages } from "./define";

/* The television (src/tv/view.tsx). A living-room screen, read from a
 * sofa: short labels, and the remote named in the one hint a screen
 * reader gets. */
export default defineMessages({
  "tv.landmark": { en: "Television", sk: "Televízor", de: "Fernseher" },
  "tv.remoteHint": {
    en: "Arrows move the ring. Enter opens it. Escape goes back. Home returns to the start.",
    sk: "Šípky posúvajú krúžok. Enter otvorí položku. Escape sa vráti. Home ide na začiatok.",
    de: "Pfeile bewegen den Ring. Enter öffnet ihn. Escape geht zurück. Home kehrt zum Anfang.",
  },
  "tv.home": { en: "Home", sk: "Domov", de: "Start" },
  "tv.back": { en: "Back", sk: "Späť", de: "Zurück" },
  "tv.desk": { en: "Desk", sk: "Pracovný stôl", de: "Schreibtisch" },
  "tv.apps": { en: "Apps", sk: "Aplikácie", de: "Apps" },
  "tv.glance": { en: "At a glance", sk: "Na prvý pohľad", de: "Auf einen Blick" },
  "tv.health": { en: "The box", sk: "Zariadenie", de: "Die Box" },
  "tv.storage": { en: "Disk", sk: "Disk", de: "Platte" },
  "tv.storage.signIn": {
    en: "Sign in to see the disk",
    sk: "Prihláste sa, ak chcete vidieť disk",
    de: "Melde dich an, um die Platte zu sehen",
  },
  "tv.storage.unread": {
    en: "No reading yet",
    sk: "Zatiaľ bez údaja",
    de: "Noch kein Wert",
  },
  "tv.storage.of": {
    en: "{used} of {total}",
    sk: "{used} z {total}",
    de: "{used} von {total}",
  },
  "tv.storage.detail": {
    en: "Room in use on this box.",
    sk: "Miesto, ktoré toto zariadenie používa.",
    de: "Belegter Platz auf dieser Box.",
  },
  "tv.signIn": { en: "Sign in", sk: "Prihlásiť sa", de: "Anmelden" },
  "tv.signIn.body": {
    en: "The disk and the desk need a sign-in. The apps on the home screen do not.",
    sk: "Disk a pracovný stôl potrebujú prihlásenie. Aplikácie na domovskej obrazovke nie.",
    de: "Platte und Schreibtisch brauchen eine Anmeldung. Die Apps auf dem Startbildschirm nicht.",
  },
  "tv.signIn.name": { en: "Name", sk: "Meno", de: "Name" },
  "tv.signIn.password": { en: "Password", sk: "Heslo", de: "Passwort" },
  "tv.signIn.submit": { en: "Sign in", sk: "Prihlásiť sa", de: "Anmelden" },
  "tv.signIn.working": { en: "Signing in", sk: "Prihlasovanie", de: "Anmeldung läuft" },
  "tv.signIn.refused": {
    en: "That name or password was refused.",
    sk: "Toto meno alebo heslo bolo odmietnuté.",
    de: "Dieser Name oder dieses Passwort wurde abgelehnt.",
  },
  "tv.signIn.notYet": {
    en: "Sign-in is not answering yet. Try again in a moment.",
    sk: "Prihlásenie zatiaľ neodpovedá. Skúste to o chvíľu.",
    de: "Die Anmeldung antwortet noch nicht. Versuche es gleich noch einmal.",
  },
  "tv.desk.title": { en: "The desk", sk: "Pracovný stôl", de: "Der Schreibtisch" },
  "tv.desk.body": {
    en: "Settings, installs and forms are for a computer, up close. The television stays on this screen.",
    sk: "Nastavenia, inštalácie a formuláre sú pre počítač zblízka. Televízor zostáva na tejto obrazovke.",
    de: "Einstellungen, Installationen und Formulare sind für einen Computer aus der Nähe. Der Fernseher bleibt auf diesem Bildschirm.",
  },
  "tv.desk.open": { en: "Open the desk", sk: "Otvoriť pracovný stôl", de: "Schreibtisch öffnen" },
  "tv.desk.signOut": { en: "Sign out", sk: "Odhlásiť sa", de: "Abmelden" },
  "tv.health.ok": {
    en: "This box is answering.",
    sk: "Toto zariadenie odpovedá.",
    de: "Diese Box antwortet.",
  },
  "tv.health.down": {
    en: "This box is not answering.",
    sk: "Toto zariadenie neodpovedá.",
    de: "Diese Box antwortet nicht.",
  },
  "tv.health.checking": {
    en: "Checking whether this box is answering.",
    sk: "Overuje sa, či zariadenie odpovedá.",
    de: "Es wird geprüft, ob diese Box antwortet.",
  },
});
