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
    en: "Removing someone stops them signing in. Their files in LosOS Cloud and LosOS Git stay.",
    sk: "Odstránený človek sa už neprihlási. Súbory v LosOS Cloud a LosOS Git zostanú.",
    de: "Wer entfernt wird, kann sich nicht mehr anmelden. Die Dateien in LosOS Cloud und LosOS Git bleiben.",
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
    en: "They sign in at this box's sign-in page with this name and password. LosOS Cloud and LosOS Git open under their own name, with files of their own.",
    sk: "Prihlásia sa na prihlasovacej stránke tohto zariadenia týmto menom a heslom. LosOS Cloud a LosOS Git sa im otvoria pod ich vlastným menom, s vlastnými súbormi.",
    de: "Sie melden sich auf der Anmeldeseite dieser Box mit diesem Namen und Passwort an. LosOS Cloud und LosOS Git öffnen sich unter ihrem eigenen Namen, mit eigenen Dateien.",
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
    en: "They can no longer sign in to this box. Their files in LosOS Cloud and LosOS Git stay where they are.",
    sk: "Do tohto zariadenia sa už neprihlásia. Ich súbory v LosOS Cloud a LosOS Git zostanú, kde sú.",
    de: "Sie können sich nicht mehr an dieser Box anmelden. Ihre Dateien in LosOS Cloud und LosOS Git bleiben, wo sie sind.",
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
  // ── Own account, two-factor and the rules ───────────────────────────
  "users.section.account": { en: "Own account", sk: "Vlastný účet", de: "Eigenes Konto" },
  "users.badge.twoFactor": { en: "Two-factor", sk: "Dvojstupňové", de: "Zwei-Faktor" },
  "users.badge.held": { en: "Held", sk: "Zadržaný", de: "Angehalten" },
  "users.held.twoFactor": {
    en: "Reaches only their own account until they add an authenticator app or a passkey",
    sk: "Dostane sa iba k vlastnému účtu, kým nepridá overovaciu aplikáciu alebo prístupový kľúč",
    de: "Erreicht nur das eigene Konto, bis eine Authenticator-App oder ein Passkey hinzugefügt ist",
  },
  "users.held.password": {
    en: "Reaches only their own account until they choose a new password",
    sk: "Dostane sa iba k vlastnému účtu, kým si nezvolí nové heslo",
    de: "Erreicht nur das eigene Konto, bis ein neues Passwort gewählt ist",
  },
  "users.action.resetFactors": {
    en: "Reset two-factor",
    sk: "Zrušiť dvojstupňové",
    de: "Zwei-Faktor zurücksetzen",
  },
  "users.resetDialog.title": {
    en: "Remove {name}'s authenticator app and passkeys?",
    sk: "Odstrániť overovaciu aplikáciu a prístupové kľúče používateľa {name}?",
    de: "Authenticator-App und Passkeys von {name} entfernen?",
  },
  "users.resetDialog.body": {
    en: "Do this when they lost their phone or key. They sign in with their password alone until they add new ones. If this box requires two-factor sign-in, they reach only their own account until then.",
    sk: "Urobte to, keď stratili telefón alebo kľúč. Kým nepridajú nové, prihlasujú sa iba heslom. Ak toto zariadenie vyžaduje dvojstupňové prihlásenie, dovtedy sa dostanú iba k vlastnému účtu.",
    de: "Tu das, wenn Telefon oder Schlüssel verloren sind. Bis neue hinzugefügt sind, reicht das Passwort allein. Verlangt diese Box die Zwei-Faktor-Anmeldung, erreicht die Person bis dahin nur ihr eigenes Konto.",
  },
  "users.resetDialog.confirm": { en: "Remove them", sk: "Odstrániť", de: "Entfernen" },
  "users.factorsReset": {
    en: "{name} signs in with a password alone now",
    sk: "{name} sa teraz prihlasuje iba heslom",
    de: "{name} meldet sich jetzt nur mit dem Passwort an",
  },
  "users.factorsResetFailed": {
    en: "Could not reset {name}'s two-factor sign-in",
    sk: "Dvojstupňové prihlásenie používateľa {name} sa nepodarilo zrušiť",
    de: "Die Zwei-Faktor-Anmeldung von {name} konnte nicht zurückgesetzt werden",
  },
  "users.rules.title": { en: "Sign-in rules", sk: "Pravidlá prihlásenia", de: "Anmelderegeln" },
  "users.rules.twoFactor": {
    en: "Require two-factor sign-in",
    sk: "Vyžadovať dvojstupňové prihlásenie",
    de: "Zwei-Faktor-Anmeldung verlangen",
  },
  "users.rules.twoFactorDetail": {
    en: "Everyone adds an authenticator app or a passkey. Until they do, they reach only their own account.",
    sk: "Každý si pridá overovaciu aplikáciu alebo prístupový kľúč. Kým to neurobí, dostane sa iba k vlastnému účtu.",
    de: "Alle fügen eine Authenticator-App oder einen Passkey hinzu. Bis dahin erreichen sie nur ihr eigenes Konto.",
  },
  "users.rules.expire": {
    en: "Passwords run out",
    sk: "Heslá vypršia",
    de: "Passwörter laufen ab",
  },
  "users.rules.expireDetail": {
    en: "After the days below, a person chooses a new password before they reach anything else.",
    sk: "Po uvedenom počte dní si človek zvolí nové heslo, než sa dostane k čomukoľvek inému.",
    de: "Nach den Tagen unten wählt eine Person ein neues Passwort, bevor sie etwas anderes erreicht.",
  },
  "users.rules.days": { en: "Days", sk: "Dni", de: "Tage" },
  "users.rules.daysDetail": {
    en: "1 to {max}, counted from when each person last set a password.",
    sk: "1 až {max}, počítané od posledného nastavenia hesla každého človeka.",
    de: "1 bis {max}, gezählt ab dem Zeitpunkt, an dem jede Person zuletzt ein Passwort gesetzt hat.",
  },
  "users.rules.save": { en: "Save rules", sk: "Uložiť pravidlá", de: "Regeln speichern" },
  "users.rules.saved": {
    en: "The sign-in rules are saved",
    sk: "Pravidlá prihlásenia sú uložené",
    de: "Die Anmelderegeln sind gespeichert",
  },
  "users.rules.failed": {
    en: "Could not save the sign-in rules.",
    sk: "Pravidlá prihlásenia sa nepodarilo uložiť.",
    de: "Die Anmelderegeln konnten nicht gespeichert werden.",
  },
  "users.rules.needsOwnFactor": {
    en: "Add an authenticator app or a passkey to your own account first, or the rule would hold you too.",
    sk: "Najprv si pridajte overovaciu aplikáciu alebo prístupový kľúč k vlastnému účtu, inak by pravidlo zadržalo aj vás.",
    de: "Füge zuerst deinem eigenen Konto eine Authenticator-App oder einen Passkey hinzu, sonst hält die Regel auch dich an.",
  },
  "users.rules.openAccount": {
    en: "Open your account",
    sk: "Otvoriť váš účet",
    de: "Dein Konto öffnen",
  },
  "users.rules.caption": {
    en: "People held by a rule reach only their own account, and LosOS Login keeps them out of LosOS Cloud and LosOS Git. The spare key is never held.",
    sk: "Ľudia zadržaní pravidlom sa dostanú iba k vlastnému účtu a LosOS Login ich nepustí do LosOS Cloud ani LosOS Git. Náhradný kľúč sa nezadržiava nikdy.",
    de: "Wen eine Regel anhält, erreicht nur das eigene Konto, und LosOS Login lässt ihn nicht in LosOS Cloud und LosOS Git. Der Ersatzschlüssel wird nie angehalten.",
  },
});
