import { defineMessages } from "./define";

/* Your account (screens/settings/pane-account.tsx): a person's own name,
 * email address, password and second factors on LosOS Login. The pane's
 * name, summary and search words live under settings.panes.account.* with
 * the other panes'. */

export default defineMessages({
  "account.loading": {
    en: "Loading your account…",
    sk: "Načítavam váš účet…",
    de: "Lade dein Konto…",
  },
  "account.failed": {
    en: "Could not load your account.",
    sk: "Váš účet sa nepodarilo načítať.",
    de: "Dein Konto konnte nicht geladen werden.",
  },
  "account.retry": { en: "Try again", sk: "Skúsiť znova", de: "Erneut versuchen" },
  "account.noAccount": {
    en: "The spare key has no account of its own. Sign in with your name to see yours.",
    sk: "Náhradný kľúč nemá vlastný účet. Prihláste sa svojím menom a uvidíte svoj.",
    de: "Der Ersatzschlüssel hat kein eigenes Konto. Melde dich mit deinem Namen an, um deins zu sehen.",
  },

  // ── Held by a rule ───────────────────────────────────────────────────
  "account.held.title": {
    en: "Finish this first",
    sk: "Najprv dokončite toto",
    de: "Erst das hier erledigen",
  },
  "account.held.twoFactor": {
    en: "This box asks everyone for a second step when they sign in. Add an authenticator app or a passkey below.",
    sk: "Toto zariadenie pri prihlásení od každého žiada druhý krok. Pridajte nižšie overovaciu aplikáciu alebo prístupový kľúč.",
    de: "Diese Box verlangt bei der Anmeldung von allen einen zweiten Schritt. Füge unten eine Authenticator-App oder einen Passkey hinzu.",
  },
  "account.held.password": {
    en: "Your password has run out. Choose a new one below.",
    sk: "Vaše heslo vypršalo. Nižšie si zvoľte nové.",
    de: "Dein Passwort ist abgelaufen. Wähle unten ein neues.",
  },
  "account.held.after": {
    en: "Until then you reach only this page, and LosOS cloud and LosOS Git stay closed. The rest opens as soon as you are done.",
    sk: "Dovtedy sa dostanete iba na túto stránku a LosOS cloud a LosOS Git zostanú zatvorené. Zvyšok sa otvorí hneď, ako budete hotoví.",
    de: "Bis dahin erreichst du nur diese Seite, und LosOS cloud und LosOS Git bleiben zu. Der Rest öffnet sich, sobald du fertig bist.",
  },

  // ── Details ──────────────────────────────────────────────────────────
  "account.details.title": { en: "Your details", sk: "Vaše údaje", de: "Deine Angaben" },
  "account.details.signInName": {
    en: "Sign-in name",
    sk: "Prihlasovacie meno",
    de: "Anmeldename",
  },
  "account.details.name": { en: "Full name", sk: "Celé meno", de: "Vollständiger Name" },
  "account.details.email": { en: "Email address", sk: "E-mailová adresa", de: "E-Mail-Adresse" },
  "account.details.save": { en: "Save", sk: "Uložiť", de: "Speichern" },
  "account.details.saved": {
    en: "Your details are saved",
    sk: "Vaše údaje sú uložené",
    de: "Deine Angaben sind gespeichert",
  },
  "account.details.caption": {
    en: "LosOS cloud and LosOS Git show the same name and address. Only an administrator can change your sign-in name.",
    sk: "LosOS cloud a LosOS Git zobrazujú to isté meno a adresu. Prihlasovacie meno môže zmeniť iba správca.",
    de: "LosOS cloud und LosOS Git zeigen denselben Namen und dieselbe Adresse. Den Anmeldenamen kann nur ein Administrator ändern.",
  },

  // ── Password ─────────────────────────────────────────────────────────
  "account.password.title": { en: "Password", sk: "Heslo", de: "Passwort" },
  "account.password.changed": {
    en: "Last changed",
    sk: "Naposledy zmenené",
    de: "Zuletzt geändert",
  },
  "account.password.expires": { en: "Runs out", sk: "Vyprší", de: "Läuft ab" },
  "account.password.never": { en: "Never", sk: "Nikdy", de: "Nie" },
  "account.password.new": { en: "New password", sk: "Nové heslo", de: "Neues Passwort" },
  "account.password.detail": {
    en: "At least {count} characters, with upper and lower case, a digit and a symbol.",
    sk: "Aspoň {count} znakov, s veľkými aj malými písmenami, číslicou a symbolom.",
    de: "Mindestens {count} Zeichen, mit Groß- und Kleinbuchstaben, einer Ziffer und einem Sonderzeichen.",
  },
  "account.password.save": { en: "Change password", sk: "Zmeniť heslo", de: "Passwort ändern" },
  "account.password.saved": {
    en: "Your password is changed",
    sk: "Vaše heslo je zmenené",
    de: "Dein Passwort ist geändert",
  },
  "account.password.caption": {
    en: "The same password opens LosOS cloud and LosOS Git through LosOS Login.",
    sk: "To isté heslo otvára cez LosOS Login aj LosOS cloud a LosOS Git.",
    de: "Dasselbe Passwort öffnet über LosOS Login auch LosOS cloud und LosOS Git.",
  },

  // ── Two-factor sign-in ───────────────────────────────────────────────
  "account.factors.title": {
    en: "Two-factor sign-in",
    sk: "Dvojstupňové prihlásenie",
    de: "Zwei-Faktor-Anmeldung",
  },
  "account.factors.app": {
    en: "Authenticator app",
    sk: "Overovacia aplikácia",
    de: "Authenticator-App",
  },
  "account.factors.passkeys": { en: "Passkeys", sk: "Prístupové kľúče", de: "Passkeys" },
  "account.factors.added": { en: "Added", sk: "Pridaná", de: "Hinzugefügt" },
  "account.factors.notAdded": { en: "Not added", sk: "Nepridaná", de: "Nicht hinzugefügt" },
  "account.factors.none": { en: "None", sk: "Žiadne", de: "Keine" },
  "account.factors.open": {
    en: "Open LosOS Login",
    sk: "Otvoriť LosOS Login",
    de: "LosOS Login öffnen",
  },
  "account.factors.openDetail": {
    en: "Add or remove an authenticator app or a passkey on LosOS Login's own page.",
    sk: "Overovaciu aplikáciu alebo prístupový kľúč pridáte či odstránite na vlastnej stránke LosOS Login.",
    de: "Authenticator-App oder Passkey fügst du auf der eigenen Seite von LosOS Login hinzu oder entfernst sie dort.",
  },
  "account.factors.caption": {
    en: "With either one added, LosOS Login asks for it after your password every time you sign in.",
    sk: "Keď pridáte niektorý z nich, LosOS Login si ho po hesle vypýta pri každom prihlásení.",
    de: "Ist eines davon hinzugefügt, fragt LosOS Login bei jeder Anmeldung nach dem Passwort danach.",
  },
  "account.factors.required": {
    en: "This box requires it of everyone.",
    sk: "Toto zariadenie ho vyžaduje od každého.",
    de: "Diese Box verlangt es von allen.",
  },

  // ── One-time code ────────────────────────────────────────────────────
  "account.code.title": { en: "One-time code", sk: "Jednorazový kód", de: "Einmalcode" },
  "account.code.detail": {
    en: "LosOS Login asks for a code before it changes your second factors. This box sends no email, so the code is here, behind your password.",
    sk: "LosOS Login si pred zmenou druhého faktora vypýta kód. Toto zariadenie neposiela e-maily, preto je kód tu, za vaším heslom.",
    de: "LosOS Login fragt nach einem Code, bevor es deine zweiten Faktoren ändert. Diese Box verschickt keine E-Mails, daher steht der Code hier, hinter deinem Passwort.",
  },
  "account.code.password": {
    en: "Your password",
    sk: "Vaše heslo",
    de: "Dein Passwort",
  },
  "account.code.show": { en: "Show the code", sk: "Zobraziť kód", de: "Code zeigen" },
  "account.code.wrongPassword": {
    en: "That is not your password.",
    sk: "To nie je vaše heslo.",
    de: "Das ist nicht dein Passwort.",
  },
  "account.code.none": {
    en: "LosOS Login has not made a code for you yet. Ask it for one on its page first.",
    sk: "LosOS Login pre vás ešte kód nevytvoril. Najprv si ho vypýtajte na jeho stránke.",
    de: "LosOS Login hat noch keinen Code für dich erzeugt. Fordere zuerst auf seiner Seite einen an.",
  },
  "account.code.failed": {
    en: "Could not get the code.",
    sk: "Kód sa nepodarilo získať.",
    de: "Der Code konnte nicht abgerufen werden.",
  },
  "account.code.value": {
    en: "Type this code on LosOS Login's page:",
    sk: "Tento kód zadajte na stránke LosOS Login:",
    de: "Gib diesen Code auf der Seite von LosOS Login ein:",
  },
  "account.windows.title": {
    en: "Your files on Windows, Mac and Linux",
    sk: "Vaše súbory vo Windows, Macu a Linuxe",
    de: "Deine Dateien unter Windows, Mac und Linux",
  },
  "account.windows.address": {
    en: "Address in File Explorer",
    sk: "Adresa v Prieskumníkovi",
    de: "Adresse im Explorer",
  },
  "account.windows.signIn": {
    en: "Sign in with your LosOS password",
    sk: "Prihlásenie heslom LosOS",
    de: "Anmeldung mit deinem LosOS-Passwort",
  },
  "account.windows.ready": {
    en: "Ready",
    sk: "Pripravené",
    de: "Bereit",
  },
  "account.windows.notReady": {
    en: "Not yet",
    sk: "Ešte nie",
    de: "Noch nicht",
  },
  "account.windows.allow": {
    en: "Let Windows take your password",
    sk: "Povoliť Windows vaše heslo",
    de: "Windows dein Passwort erlauben",
  },
  "account.windows.allowDetail": {
    en: "Windows checks passwords its own way, so the box needs to see yours once. Type it here, or change it, and it works from then on.",
    sk: "Windows overuje heslá po svojom, takže zariadenie musí to vaše raz vidieť. Zadajte ho sem alebo ho zmeňte a odvtedy to bude fungovať.",
    de: "Windows prüft Passwörter auf seine eigene Art, darum muss die Box deins einmal sehen. Gib es hier ein oder ändere es, dann klappt es ab sofort.",
  },
  "account.windows.save": {
    en: "Allow",
    sk: "Povoliť",
    de: "Erlauben",
  },
  "account.windows.wrongPassword": {
    en: "That is not your password.",
    sk: "To nie je vaše heslo.",
    de: "Das ist nicht dein Passwort.",
  },
  "account.windows.failed": {
    en: "That did not work. Try again in a moment.",
    sk: "Nepodarilo sa. Skúste to o chvíľu znova.",
    de: "Das hat nicht geklappt. Versuch es gleich noch einmal.",
  },
  "account.windows.caption": {
    en: "On a computer on the same network as this box, open Network in File Explorer, or Network in Finder or your Linux file manager, and open the box. Sign in with your LosOS name and password. Files you change there change in LosOS Cloud too. Something you delete there is gone for good: it skips LosOS Cloud's trash.",
    sk: "Na počítači v rovnakej sieti ako toto zariadenie otvorte Sieť v Prieskumníkovi, vo Finderi alebo v správcovi súborov v Linuxe a otvorte zariadenie. Prihláste sa svojím menom a heslom LosOS. Súbory, ktoré tam zmeníte, sa zmenia aj v LosOS Cloud. Čo tam vymažete, zmizne natrvalo: obíde kôš LosOS Cloud.",
    de: "Öffne auf einem Computer im selben Netz wie diese Box Netzwerk im Explorer, im Finder oder in deinem Linux-Dateimanager und öffne die Box. Melde dich mit deinem LosOS-Namen und -Passwort an. Dateien, die du dort änderst, ändern sich auch in LosOS Cloud. Was du dort löschst, ist endgültig weg: Es landet nicht im Papierkorb von LosOS Cloud.",
  },
});
