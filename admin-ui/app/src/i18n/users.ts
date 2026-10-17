import { defineMessages } from "./define";

/* The Users pane (screens/settings/pane-users.tsx): the people who may sign
 * in to this box's own LosOS Login, and what each of them can change. The
 * pane's name, summary and search words live under settings.panes.users.*
 * with the other panes'. Section names reuse the panes' own labels. */

export default defineMessages({
  "users.loading": {
    en: "Loading the people on this box…",
    sk: "Načítavam ľudí na tomto zariadení…",
    de: "Lade die Personen dieser Box…",
  },
  "users.failed": {
    en: "Could not load the people on this box.",
    sk: "Ľudí na tomto zariadení sa nepodarilo načítať.",
    de: "Die Personen dieser Box konnten nicht geladen werden.",
  },
  "users.retry": { en: "Try again", sk: "Skúsiť znova", de: "Erneut versuchen" },
  "users.unavailable": {
    en: "This box cannot manage people from here yet.",
    sk: "Toto zariadenie zatiaľ nevie spravovať ľudí odtiaľto.",
    de: "Diese Box kann Personen hier noch nicht verwalten.",
  },
  "users.elsewhere.title": {
    en: "Sign-in runs elsewhere",
    sk: "Prihlasovanie beží inde",
    de: "Die Anmeldung läuft woanders",
  },
  "users.elsewhere.body": {
    en: "This box does not sign people in itself, so there is nobody to add here.",
    sk: "Toto zariadenie neprihlasuje ľudí samo, takže tu nie je koho pridať.",
    de: "Diese Box meldet Personen nicht selbst an, daher gibt es hier niemanden hinzuzufügen.",
  },

  // ── The list ──────────────────────────────────────────────────────────
  "users.list.title": { en: "People", sk: "Ľudia", de: "Personen" },
  "users.list.caption": {
    en: "Removing someone stops them signing in. Their files in LosOS cloud and LosOS Git stay.",
    sk: "Odstránený človek sa už neprihlási. Súbory v LosOS cloud a LosOS Git zostanú.",
    de: "Wer entfernt wird, kann sich nicht mehr anmelden. Die Dateien in LosOS cloud und LosOS Git bleiben.",
  },
  "users.badge.owner": { en: "Owner", sk: "Vlastník", de: "Besitzer" },
  "users.badge.you": { en: "You", sk: "Vy", de: "Sie" },
  "users.badge.disabled": { en: "Disabled", sk: "Vypnutý", de: "Deaktiviert" },
  "users.level.admin": { en: "Administrator", sk: "Správca", de: "Administrator" },
  "users.level.viewer": { en: "Sees everything", sk: "Vidí všetko", de: "Sieht alles" },
  "users.level.parts": { en: "Chosen parts", sk: "Vybrané časti", de: "Ausgewählte Teile" },
  "users.canChange": {
    en: "Can change: {list}",
    sk: "Môže meniť: {list}",
    de: "Darf ändern: {list}",
  },
  "users.canChangeNothing": {
    en: "Can change only their own password",
    sk: "Môže meniť iba svoje heslo",
    de: "Darf nur das eigene Passwort ändern",
  },
  "users.seesEverything": {
    en: "Sees every pane and changes nothing",
    sk: "Vidí každý panel a nič nemení",
    de: "Sieht jeden Bereich und ändert nichts",
  },
  "users.action.access": { en: "Access", sk: "Prístup", de: "Zugriff" },
  "users.action.password": { en: "New password", sk: "Nové heslo", de: "Neues Passwort" },
  "users.action.remove": { en: "Remove", sk: "Odstrániť", de: "Entfernen" },

  // ── Adding someone ────────────────────────────────────────────────────
  "users.form.title": { en: "Add a person", sk: "Pridať človeka", de: "Person hinzufügen" },
  "users.form.name": { en: "Sign-in name", sk: "Prihlasovacie meno", de: "Anmeldename" },
  "users.form.nameDetail": {
    en: "Lower-case letters, digits, dots, dashes and underscores, starting with a letter.",
    sk: "Malé písmená, číslice, bodky, pomlčky a podčiarkovníky, na začiatku písmeno.",
    de: "Kleinbuchstaben, Ziffern, Punkte, Bindestriche und Unterstriche, am Anfang ein Buchstabe.",
  },
  "users.form.namePlaceholder": { en: "eva", sk: "eva", de: "eva" },
  "users.form.displayName": { en: "Full name", sk: "Celé meno", de: "Vollständiger Name" },
  "users.form.displayNamePlaceholder": { en: "Eva Novak", sk: "Eva Nováková", de: "Eva Novak" },
  "users.form.password": { en: "Password", sk: "Heslo", de: "Passwort" },
  "users.form.passwordDetail": {
    en: "At least {count} characters, with a lower-case and an upper-case letter, a digit and a symbol. They can change it once signed in.",
    sk: "Aspoň {count} znakov, s malým a veľkým písmenom, číslicou a symbolom. Po prihlásení si ho môžu zmeniť.",
    de: "Mindestens {count} Zeichen, mit Klein- und Großbuchstabe, Ziffer und Sonderzeichen. Nach der Anmeldung lässt es sich ändern.",
  },
  "users.form.access": { en: "Access", sk: "Prístup", de: "Zugriff" },
  "users.form.adminDetail": {
    en: "Can change everything on this box, including who can sign in.",
    sk: "Môže meniť všetko na tomto zariadení vrátane toho, kto sa môže prihlásiť.",
    de: "Darf alles an dieser Box ändern, auch wer sich anmelden kann.",
  },
  "users.form.viewerDetail": {
    en: "Sees every pane, and can change the parts switched on below.",
    sk: "Vidí každú časť nastavení a môže meniť tie, ktoré sú nižšie zapnuté.",
    de: "Sieht jeden Bereich und darf die unten eingeschalteten Teile ändern.",
  },
  "users.form.partsDetail": {
    en: "Sees and can change only the parts switched on below.",
    sk: "Vidí a môže meniť iba časti, ktoré sú nižšie zapnuté.",
    de: "Sieht und ändert nur die unten eingeschalteten Teile.",
  },
  "users.form.partsTitle": { en: "Can change", sk: "Môže meniť", de: "Darf ändern" },
  "users.form.add": { en: "Add", sk: "Pridať", de: "Hinzufügen" },
  "users.form.caption": {
    en: "They sign in at this box's sign-in page with this name and password. LosOS cloud and LosOS Git open under their own name, with files of their own.",
    sk: "Prihlásia sa na prihlasovacej stránke tohto zariadenia týmto menom a heslom. LosOS cloud a LosOS Git sa im otvoria pod ich vlastným menom, s vlastnými súbormi.",
    de: "Sie melden sich auf der Anmeldeseite dieser Box mit diesem Namen und Passwort an. LosOS cloud und LosOS Git öffnen sich unter ihrem eigenen Namen, mit eigenen Dateien.",
  },
  "users.form.failed": {
    en: "Could not add them.",
    sk: "Nepodarilo sa ich pridať.",
    de: "Hinzufügen fehlgeschlagen.",
  },
  "users.added": {
    en: "{name} can sign in now",
    sk: "{name} sa už môže prihlásiť",
    de: "{name} kann sich jetzt anmelden",
  },

  // ── Changing someone ──────────────────────────────────────────────────
  "users.accessDialog.title": {
    en: "Access for {name}",
    sk: "Prístup pre {name}",
    de: "Zugriff für {name}",
  },
  "users.accessDialog.body": {
    en: "The change applies to their next click; they do not need to sign in again.",
    sk: "Zmena platí od ich ďalšieho kliknutia, nemusia sa prihlasovať znova.",
    de: "Die Änderung gilt ab dem nächsten Klick; eine neue Anmeldung ist nicht nötig.",
  },
  "users.accessSaved": {
    en: "Access for {name} changed",
    sk: "Prístup pre {name} je zmenený",
    de: "Zugriff für {name} geändert",
  },
  "users.accessFailed": {
    en: "Could not change the access for {name}",
    sk: "Prístup pre {name} sa nepodarilo zmeniť",
    de: "Der Zugriff für {name} konnte nicht geändert werden",
  },
  "users.passwordDialog.title": {
    en: "New password for {name}",
    sk: "Nové heslo pre {name}",
    de: "Neues Passwort für {name}",
  },
  "users.passwordDialog.body": {
    en: "Their old password stops working at once.",
    sk: "Staré heslo prestane platiť hneď.",
    de: "Das alte Passwort gilt sofort nicht mehr.",
  },
  "users.passwordSaved": {
    en: "{name} has a new password",
    sk: "{name} má nové heslo",
    de: "{name} hat ein neues Passwort",
  },
  "users.removeDialog.title": {
    en: "Remove {name}?",
    sk: "Odstrániť {name}?",
    de: "{name} entfernen?",
  },
  "users.removeDialog.body": {
    en: "They can no longer sign in to this box. Their files in LosOS cloud and LosOS Git stay where they are.",
    sk: "Do tohto zariadenia sa už neprihlásia. Ich súbory v LosOS cloud a LosOS Git zostanú, kde sú.",
    de: "Sie können sich nicht mehr an dieser Box anmelden. Ihre Dateien in LosOS cloud und LosOS Git bleiben, wo sie sind.",
  },
  "users.removed": {
    en: "{name} can no longer sign in",
    sk: "{name} sa už nemôže prihlásiť",
    de: "{name} kann sich nicht mehr anmelden",
  },
  "users.removeFailed": {
    en: "Could not remove {name}",
    sk: "{name} sa nepodarilo odstrániť",
    de: "{name} konnte nicht entfernt werden",
  },
  "users.cancel": { en: "Cancel", sk: "Zrušiť", de: "Abbrechen" },
  "users.save": { en: "Save", sk: "Uložiť", de: "Speichern" },
  "users.keep": { en: "Keep", sk: "Ponechať", de: "Behalten" },
});
