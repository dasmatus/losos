import { defineMessages } from "./define";

/* Installing an app from the Apps pane's search (screens/settings/
 * install-dialog.tsx, values-form.tsx) and the list of apps installed that
 * way (pane-apps.tsx). `notshared` and `shared` are the names of the box's
 * two users and stay as they are in every language. */
export default defineMessages({
  // ── The search row ──────────────────────────────────────────────────────
  "install.button": { en: "Install", sk: "Inštalovať", de: "Installieren" },
  "install.buttonLabel": {
    en: "Install {name}",
    sk: "Inštalovať {name}",
    de: "{name} installieren",
  },
  "install.installedBadge": { en: "Installed", sk: "Nainštalované", de: "Installiert" },
  "install.noCluster": {
    en: "Apps from the search install only while Files or Code is set to Kept separate above.",
    sk: "Aplikácie z vyhľadávania sa dajú inštalovať, len keď sú Súbory alebo Kód vyššie nastavené na Oddelene.",
    de: "Apps aus der Suche lassen sich nur installieren, solange Dateien oder Code oben auf Abgeschottet steht.",
  },

  // ── The dialog: properties and user ─────────────────────────────────────
  "install.title": { en: "Install {name}", sk: "Inštalovať {name}", de: "{name} installieren" },
  "install.changeTitle": { en: "Change {name}", sk: "Zmeniť {name}", de: "{name} ändern" },
  "install.byline": {
    en: "Version {version}, published by {publisher}.",
    sk: "Verzia {version}, zverejnil {publisher}.",
    de: "Version {version}, veröffentlicht von {publisher}.",
  },
  "install.fetching": {
    en: "Fetching the app's properties…",
    sk: "Načítavajú sa vlastnosti aplikácie…",
    de: "Die Eigenschaften der App werden geladen…",
  },
  "install.fetchFailed": {
    en: "The box could not fetch this app: {message}",
    sk: "Zariadenie nemohlo načítať túto aplikáciu: {message}",
    de: "Die Box konnte diese App nicht laden: {message}",
  },
  "install.retry": { en: "Try again", sk: "Skúsiť znova", de: "Erneut versuchen" },
  "install.name": { en: "Name on this box", sk: "Názov na tomto zariadení", de: "Name auf dieser Box" },
  "install.nameHint": {
    en: "Lower-case letters, digits and dashes. It names the app's folder and cannot change later.",
    sk: "Malé písmená, číslice a pomlčky. Pomenuje priečinok aplikácie a neskôr sa nedá zmeniť.",
    de: "Kleinbuchstaben, Ziffern und Bindestriche. Er benennt den Ordner der App und lässt sich später nicht ändern.",
  },
  "install.nameInvalid": {
    en: "Start with a letter, then up to 39 lower-case letters, digits or dashes, not ending in a dash.",
    sk: "Začnite písmenom, potom najviac 39 malých písmen, číslic alebo pomlčiek, bez pomlčky na konci.",
    de: "Mit einem Buchstaben beginnen, dann bis zu 39 Kleinbuchstaben, Ziffern oder Bindestriche, ohne Bindestrich am Ende.",
  },
  "install.runAs": { en: "Run it as", sk: "Spustiť ako", de: "Ausführen als" },
  "install.notsharedDetail": {
    en: "The owner's side of the disk, next to your files. Only this box can read it.",
    sk: "Časť disku vlastníka, vedľa vašich súborov. Čítať ju môže len toto zariadenie.",
    de: "Der Teil der Festplatte, der dem Besitzer gehört, neben deinen Dateien. Nur diese Box kann ihn lesen.",
  },
  "install.sharedDetail": {
    en: "The side of the disk this box lends to the mesh, apart from your files.",
    sk: "Časť disku, ktorú toto zariadenie požičiava sieti, oddelene od vašich súborov.",
    de: "Der Teil der Festplatte, den diese Box dem Verbund leiht, getrennt von deinen Dateien.",
  },
  "install.sharedLocked": {
    en: "Off while this box does not share its disk. Switch sharing on under Storage to use it.",
    sk: "Nedostupné, kým toto zariadenie nezdieľa svoj disk. Zapnite zdieľanie v časti Úložisko.",
    de: "Nicht verfügbar, solange diese Box ihre Festplatte nicht teilt. Schalte das Teilen unter Speicher ein.",
  },
  "install.runAsCaption": {
    en: "The app runs as this user, with no admin rights, and keeps its data in {path}.",
    sk: "Aplikácia beží ako tento používateľ, bez práv správcu, a dáta si ukladá do {path}.",
    de: "Die App läuft als dieser Benutzer, ohne Administratorrechte, und legt ihre Daten in {path} ab.",
  },
  "install.properties": { en: "Properties", sk: "Vlastnosti", de: "Eigenschaften" },
  "install.asText": { en: "As text", sk: "Ako text", de: "Als Text" },
  "install.changedCount": {
    en: { one: "{count} change", other: "{count} changes" },
    sk: { one: "{count} zmena", few: "{count} zmeny", many: "{count} zmien", other: "{count} zmien" },
    de: { one: "{count} Änderung", other: "{count} Änderungen" },
  },
  "install.textCaption": {
    en: "The app's own settings file. A property changed on the other tab wins over the same line here.",
    sk: "Vlastný súbor nastavení aplikácie. Vlastnosť zmenená na druhej karte má prednosť pred rovnakým riadkom tu.",
    de: "Die eigene Einstellungsdatei der App. Eine auf dem anderen Reiter geänderte Eigenschaft hat Vorrang vor derselben Zeile hier.",
  },
  "install.textReset": { en: "Back to the app's file", sk: "Späť na súbor aplikácie", de: "Zurück zur Datei der App" },
  "install.noProperties": {
    en: "This app has no properties to set.",
    sk: "Táto aplikácia nemá žiadne vlastnosti na nastavenie.",
    de: "Diese App hat keine Eigenschaften zum Einstellen.",
  },
  "install.findProperty": { en: "Find a property", sk: "Nájsť vlastnosť", de: "Eigenschaft suchen" },
  "install.noMatch": {
    en: "No property matches that.",
    sk: "Tomu nezodpovedá žiadna vlastnosť.",
    de: "Keine Eigenschaft passt dazu.",
  },
  "install.resetProperty": { en: "Reset", sk: "Obnoviť", de: "Zurücksetzen" },
  "install.notJson": {
    en: "This is not valid JSON, so it is not used.",
    sk: "Toto nie je platný JSON, takže sa nepoužije.",
    de: "Das ist kein gültiges JSON und wird deshalb nicht verwendet.",
  },
  "install.cancel": { en: "Cancel", sk: "Zrušiť", de: "Abbrechen" },
  "install.continue": { en: "Continue", sk: "Pokračovať", de: "Weiter" },
  "install.continueChange": { en: "Continue", sk: "Pokračovať", de: "Weiter" },

  // ── The dialog: confirmation ────────────────────────────────────────────
  "install.confirmTitle": {
    en: "Install {name} on this box?",
    sk: "Inštalovať {name} na toto zariadenie?",
    de: "{name} auf dieser Box installieren?",
  },
  "install.confirmChangeTitle": {
    en: "Change {name} on this box?",
    sk: "Zmeniť {name} na tomto zariadení?",
    de: "{name} auf dieser Box ändern?",
  },
  "install.confirmBody": {
    en: "Nobody on the LosOS side has checked this app. You are trusting {publisher}.",
    sk: "Nikto na strane LosOS túto aplikáciu nekontroloval. Dôverujete {publisher}.",
    de: "Niemand auf der Seite von LosOS hat diese App geprüft. Du vertraust {publisher}.",
  },
  "install.pointCode": {
    en: "The box downloads and runs code that {publisher} wrote.",
    sk: "Zariadenie stiahne a spustí kód, ktorý napísal {publisher}.",
    de: "Die Box lädt Code herunter, den {publisher} geschrieben hat, und führt ihn aus.",
  },
  "install.pointUser": {
    en: "It runs as {user}, with no admin rights, and keeps its data in {path}.",
    sk: "Beží ako {user}, bez práv správcu, a dáta si ukladá do {path}.",
    de: "Sie läuft als {user}, ohne Administratorrechte, und legt ihre Daten in {path} ab.",
  },
  "install.pointDefaults": {
    en: "Every property keeps the app's default.",
    sk: "Každá vlastnosť si ponechá predvolenú hodnotu aplikácie.",
    de: "Jede Eigenschaft behält den Standardwert der App.",
  },
  "install.pointChanged": {
    en: {
      one: "You changed {count} property; the rest keep the app's defaults.",
      other: "You changed {count} properties; the rest keep the app's defaults.",
    },
    sk: {
      one: "Zmenili ste {count} vlastnosť; ostatné si ponechajú predvolené hodnoty.",
      few: "Zmenili ste {count} vlastnosti; ostatné si ponechajú predvolené hodnoty.",
      many: "Zmenili ste {count} vlastností; ostatné si ponechajú predvolené hodnoty.",
      other: "Zmenili ste {count} vlastností; ostatné si ponechajú predvolené hodnoty.",
    },
    de: {
      one: "Du hast {count} Eigenschaft geändert; der Rest behält die Standardwerte.",
      other: "Du hast {count} Eigenschaften geändert; der Rest behält die Standardwerte.",
    },
  },
  "install.pointOpen": {
    en: "It answers on a port of its own, on your local network only. The box refuses an app that needs admin rights, another app's port or access to the system.",
    sk: "Odpovedá na vlastnom porte, len vo vašej lokálnej sieti. Zariadenie odmietne aplikáciu, ktorá potrebuje práva správcu, port inej aplikácie alebo prístup k systému.",
    de: "Sie antwortet auf einem eigenen Port, nur in deinem lokalen Netz. Die Box lehnt eine App ab, die Administratorrechte, den Port einer anderen App oder Zugriff auf das System braucht.",
  },
  "install.back": { en: "Back", sk: "Späť", de: "Zurück" },
  "install.sending": { en: "Sending", sk: "Odosiela sa", de: "Wird gesendet" },
  "install.confirm": { en: "Install", sk: "Inštalovať", de: "Installieren" },
  "install.confirmChange": { en: "Apply the change", sk: "Použiť zmenu", de: "Änderung übernehmen" },
  "install.startedTitle": {
    en: "Installing {name}",
    sk: "Inštaluje sa {name}",
    de: "{name} wird installiert",
  },
  "install.changingTitle": {
    en: "Changing {name}",
    sk: "Mení sa {name}",
    de: "{name} wird geändert",
  },
  "install.startedBody": {
    en: "It shows under Installed apps. A large app takes a while to download.",
    sk: "Zobrazí sa v časti Nainštalované aplikácie. Veľká aplikácia sa sťahuje dlhšie.",
    de: "Sie erscheint unter Installierte Apps. Eine große App braucht eine Weile zum Herunterladen.",
  },
  "install.failedFallback": {
    en: "The box did not say why.",
    sk: "Zariadenie neuviedlo prečo.",
    de: "Die Box hat keinen Grund genannt.",
  },

  // ── Installed apps ──────────────────────────────────────────────────────
  "install.installed": { en: "Installed apps", sk: "Nainštalované aplikácie", de: "Installierte Apps" },
  "install.installedCaption": {
    en: "Each app answers on its own port of this box, on your local network only. Removing one keeps its data folder.",
    sk: "Každá aplikácia odpovedá na vlastnom porte tohto zariadenia, len vo vašej lokálnej sieti. Odstránenie ponechá jej priečinok s dátami.",
    de: "Jede App antwortet auf einem eigenen Port dieser Box, nur in deinem lokalen Netz. Beim Entfernen bleibt ihr Datenordner erhalten.",
  },
  "install.runsAs": {
    en: "{chart} {version}, runs as {user}",
    sk: "{chart} {version}, beží ako {user}",
    de: "{chart} {version}, läuft als {user}",
  },
  "install.phase.installing": { en: "Installing", sk: "Inštaluje sa", de: "Wird installiert" },
  "install.phase.removing": { en: "Removing", sk: "Odstraňuje sa", de: "Wird entfernt" },
  "install.phase.running": { en: "Running", sk: "Beží", de: "Läuft" },
  "install.phase.failed": { en: "Did not start", sk: "Nespustila sa", de: "Nicht gestartet" },
  "install.noPort": {
    en: "Running, with no web page of its own.",
    sk: "Beží, bez vlastnej webovej stránky.",
    de: "Läuft, ohne eigene Webseite.",
  },
  "install.open": { en: "Open", sk: "Otvoriť", de: "Öffnen" },
  "install.change": { en: "Change", sk: "Zmeniť", de: "Ändern" },
  "install.remove": { en: "Remove", sk: "Odstrániť", de: "Entfernen" },
  "install.listFailed": {
    en: "The box could not list its apps: {message}",
    sk: "Zariadenie nemohlo vypísať svoje aplikácie: {message}",
    de: "Die Box konnte ihre Apps nicht auflisten: {message}",
  },

  // ── Removing ────────────────────────────────────────────────────────────
  "install.removeTitle": {
    en: "Remove {name}?",
    sk: "Odstrániť {name}?",
    de: "{name} entfernen?",
  },
  "install.removeBody": {
    en: "The app stops and its port closes. Its data stays in {path}, so installing it again under the same name picks it up.",
    sk: "Aplikácia sa zastaví a jej port sa zatvorí. Jej dáta zostanú v {path}, takže opätovná inštalácia pod rovnakým názvom ich znova použije.",
    de: "Die App stoppt und ihr Port schließt sich. Ihre Daten bleiben in {path}, eine erneute Installation unter demselben Namen nimmt sie wieder auf.",
  },
  "install.removeConfirm": { en: "Remove", sk: "Odstrániť", de: "Entfernen" },
  "install.removeFailedTitle": {
    en: "The app was not removed",
    sk: "Aplikácia sa neodstránila",
    de: "Die App wurde nicht entfernt",
  },
});
