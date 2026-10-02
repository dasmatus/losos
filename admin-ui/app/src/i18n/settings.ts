import { defineMessages } from "./define";

/* The settings screen's frame: the sidebar and its panes list, the apply bar,
 * the form's validation and rebuild reports, the capacity meter, the compute
 * window sentence and the size/duration formatters. The panes' own content
 * lives in panes.ts.
 *
 * `settings.panes.<id>.keywords` are extra search words in that language,
 * space-separated. The English keywords stay in screens/settings/panes.ts and
 * are always searched too, so these only add, never replace. */

export default defineMessages({
  // ── Panes ─────────────────────────────────────────────────────────────────
  "settings.panes.storage.label": { en: "Storage", sk: "Úložisko", de: "Speicher" },
  "settings.panes.storage.summary": {
    en: "How much room this box has, what it is holding for other people, and the space held back when it was set up.",
    sk: "Koľko miesta má toto zariadenie, čo uchováva pre iných ľudí a miesto, ktoré sa pri nastavení ponechalo v rezerve.",
    de: "Wie viel Platz diese Box hat, was sie für andere aufbewahrt und welcher Platz bei der Einrichtung zurückgehalten wurde.",
  },
  "settings.panes.storage.keywords": {
    en: "",
    sk: "disk miesto kapacita zväčšiť rozšíriť rezerva záloha kópie plný",
    de: "Festplatte Platz Speicherplatz Kapazität vergrößern erweitern Reserve Backup Kopien voll",
  },
  "settings.panes.mesh.label": { en: "Mesh", sk: "Mesh", de: "Mesh" },
  "settings.panes.mesh.summary": {
    en: "Joining other boxes, and the hours this one lends its spare capacity to them.",
    sk: "Pripojenie k iným zariadeniam a hodiny, počas ktorých im toto zariadenie požičiava svoj voľný výkon.",
    de: "Anderen Boxen beitreten und die Stunden, in denen diese Box ihnen ihre freie Kapazität verleiht.",
  },
  "settings.panes.mesh.keywords": {
    en: "",
    sk: "sieť zdieľať pripojiť výpočtový výkon okno hodiny spánok noc požičiavať ostatní",
    de: "teilen beitreten Rechenleistung Zeitfenster Stunden schlafen Nacht verleihen andere",
  },
  "settings.panes.apps.label": { en: "Apps", sk: "Aplikácie", de: "Apps" },
  "settings.panes.apps.summary": {
    en: "The apps this box runs, how each one runs, and where to look for more.",
    sk: "Aplikácie, ktoré toto zariadenie spúšťa, ako každá z nich beží a kde hľadať ďalšie.",
    de: "Die Apps, die auf dieser Box laufen, wie jede davon läuft und wo du weitere findest.",
  },
  "settings.panes.apps.keywords": {
    en: "",
    sk: "aplikácia súbory kód inštalovať hľadať pridať katalóg",
    de: "App Dateien Code installieren suchen hinzufügen Katalog",
  },
  "settings.panes.network.label": { en: "Network", sk: "Sieť", de: "Netzwerk" },
  "settings.panes.network.summary": {
    en: "The name this box answers to, and how it is reached from outside your home.",
    sk: "Názov, na ktorý toto zariadenie odpovedá, a ako sa k nemu dostanete mimo domova.",
    de: "Der Name, auf den diese Box hört, und wie sie von außerhalb deines Zuhauses erreichbar ist.",
  },
  "settings.panes.network.keywords": {
    en: "",
    sk: "názov meno adresa certifikát port vzdialený prístup internet mimo domova",
    de: "Name Adresse Zertifikat Port Fernzugriff Internet außerhalb unterwegs",
  },
  "settings.panes.hardware.label": { en: "Hardware", sk: "Hardvér", de: "Hardware" },
  "settings.panes.hardware.summary": {
    en: "What this box is allowed to use inside itself.",
    sk: "Čo smie toto zariadenie používať zo svojho vlastného hardvéru.",
    de: "Was diese Box in ihrem Inneren benutzen darf.",
  },
  "settings.panes.hardware.keywords": {
    en: "",
    sk: "grafika grafická karta video procesor akcelerácia",
    de: "Grafik Grafikkarte Karte Video Prozessor Beschleunigung",
  },
  "settings.panes.security.label": { en: "Security", sk: "Zabezpečenie", de: "Sicherheit" },
  "settings.panes.security.summary": {
    en: "Four extra protections that are off by default, and what each one costs.",
    sk: "Štyri ďalšie ochrany, ktoré sú predvolene vypnuté, a čo každá z nich stojí.",
    de: "Vier zusätzliche Schutzmaßnahmen, die standardmäßig aus sind, und was jede davon kostet.",
  },
  "settings.panes.security.keywords": {
    en: "",
    sk: "bezpečnosť ochrana spevnenie pamäť procesor uzamknúť bezpečný",
    de: "Härtung härten Schutz Speicher Prozessor absichern sicher",
  },
  "settings.panes.about.label": { en: "About", sk: "O zariadení", de: "Über" },
  "settings.panes.about.summary": {
    en: "What this box is, right now.",
    sk: "Čo je toto zariadenie práve teraz.",
    de: "Was diese Box gerade ist.",
  },
  "settings.panes.about.keywords": {
    en: "",
    sk: "verzia informácie identita kľúč správca aktualizácie",
    de: "Version Info Identität Schlüssel Admin Aktualisierungen",
  },
  "settings.panes.reset.label": { en: "Reset", sk: "Reset", de: "Zurücksetzen" },
  "settings.panes.reset.summary": {
    en: "Putting every setting back the way it came.",
    sk: "Vrátenie všetkých nastavení do pôvodného stavu.",
    de: "Alle Einstellungen so zurücksetzen, wie sie ab Werk waren.",
  },
  "settings.panes.reset.keywords": {
    en: "",
    sk: "výrobné predvolené vymazať začať odznova zmazať vrátiť obnoviť",
    de: "Werkseinstellungen Standard löschen neu anfangen rückgängig",
  },

  // ── Sidebar ───────────────────────────────────────────────────────────────
  "settings.sidebar.nav": {
    en: "Settings sections",
    sk: "Sekcie nastavení",
    de: "Einstellungsbereiche",
  },
  "settings.sidebar.searchLabel": {
    en: "Search settings",
    sk: "Hľadať v nastaveniach",
    de: "Einstellungen durchsuchen",
  },
  "settings.sidebar.searchPlaceholder": { en: "Search", sk: "Hľadať", de: "Suchen" },
  "settings.sidebar.clear": { en: "Clear search", sk: "Vymazať hľadanie", de: "Suche leeren" },
  "settings.sidebar.noMatch": {
    en: "Nothing here matches “{query}”.",
    sk: "Hľadaniu „{query}“ tu nič nezodpovedá.",
    de: "Hier passt nichts zu „{query}“.",
  },

  // ── Screen notices ────────────────────────────────────────────────────────
  "settings.notice.locked": {
    en: "Paste the admin key to change anything here. Nothing on this screen can be read or written without it.",
    sk: "Ak tu chcete niečo zmeniť, vložte správcovský kľúč. Bez neho sa na tejto obrazovke nedá nič prečítať ani zapísať.",
    de: "Füge den Admin-Schlüssel ein, um hier etwas zu ändern. Ohne ihn lässt sich auf dieser Seite nichts lesen oder schreiben.",
  },
  "settings.notice.loadError": {
    en: "This box's settings could not be read: {message}",
    sk: "Nastavenia tohto zariadenia sa nepodarilo načítať: {message}",
    de: "Die Einstellungen dieser Box konnten nicht gelesen werden: {message}",
  },

  // ── Apply bar ─────────────────────────────────────────────────────────────
  "settings.applyBar.spinner": { en: "Applying", sk: "Používa sa", de: "Wird angewendet" },
  "settings.applyBar.dismiss": { en: "Dismiss", sk: "Zavrieť", de: "Schließen" },
  "settings.applyBar.pending": {
    en: { one: "{count} change not applied yet", other: "{count} changes not applied yet" },
    sk: {
      one: "{count} zmena ešte nie je použitá",
      few: "{count} zmeny ešte nie sú použité",
      many: "{count} zmien ešte nie je použitých",
      other: "{count} zmien ešte nie je použitých",
    },
    de: {
      one: "{count} Änderung noch nicht angewendet",
      other: "{count} Änderungen noch nicht angewendet",
    },
  },
  "settings.applyBar.valid": {
    en: "This box rebuilds itself to take them on. It stays reachable while it works.",
    sk: "Zariadenie sa kvôli nim znovu zostaví. Počas toho zostane dostupné.",
    de: "Die Box baut sich neu, um sie zu übernehmen. Sie bleibt dabei erreichbar.",
  },
  "settings.applyBar.invalid": {
    en: "Fix what is marked before this can be applied.",
    sk: "Pred použitím opravte, čo je označené.",
    de: "Korrigiere zuerst, was markiert ist, dann lässt sich das anwenden.",
  },
  "settings.applyBar.discard": { en: "Discard", sk: "Zahodiť", de: "Verwerfen" },
  "settings.applyBar.apply": { en: "Apply", sk: "Použiť", de: "Anwenden" },

  // ── Form validation ───────────────────────────────────────────────────────
  "settings.form.hostEmpty": {
    en: "Give this box a name.",
    sk: "Pomenujte toto zariadenie.",
    de: "Gib dieser Box einen Namen.",
  },
  "settings.form.hostTooLong": {
    en: "A name is at most 63 characters.",
    sk: "Názov môže mať najviac 63 znakov.",
    de: "Ein Name hat höchstens 63 Zeichen.",
  },
  "settings.form.hostChars": {
    en: "Use letters, digits and hyphens only, starting and ending with a letter or a digit.",
    sk: "Použite iba písmená, číslice a pomlčky; na začiatku aj na konci musí byť písmeno alebo číslica.",
    de: "Nur Buchstaben, Ziffern und Bindestriche, am Anfang und Ende ein Buchstabe oder eine Ziffer.",
  },
  "settings.form.port": {
    en: "Pick a number between 1024 and 65535.",
    sk: "Zvoľte číslo od 1024 do 65535.",
    de: "Wähle eine Zahl zwischen 1024 und 65535.",
  },
  "settings.form.window": {
    en: "Set both ends of the window as a 24-hour time, HH:MM.",
    sk: "Zadajte oba konce okna ako 24-hodinový čas, HH:MM.",
    de: "Gib beide Enden des Zeitfensters als 24-Stunden-Zeit an, HH:MM.",
  },

  // ── Rebuild reports ───────────────────────────────────────────────────────
  "settings.form.applyingYours": {
    en: "Applying your changes",
    sk: "Vaše zmeny sa používajú",
    de: "Deine Änderungen werden angewendet",
  },
  "settings.form.buildingMessage": {
    en: "This box is rebuilding itself. It stays reachable while it works.",
    sk: "Zariadenie sa znovu zostavuje. Počas toho zostane dostupné.",
    de: "Die Box baut sich gerade neu. Sie bleibt dabei erreichbar.",
  },
  "settings.form.doneTitle": {
    en: "Changes applied",
    sk: "Zmeny sú použité",
    de: "Änderungen angewendet",
  },
  "settings.form.doneMessage": {
    en: "This box is running the new settings.",
    sk: "Zariadenie beží s novými nastaveniami.",
    de: "Die Box läuft mit den neuen Einstellungen.",
  },
  "settings.form.failedTitle": {
    en: "The changes could not be applied",
    sk: "Zmeny sa nepodarilo použiť",
    de: "Die Änderungen konnten nicht angewendet werden",
  },
  "settings.form.failedMessage": {
    en: "Nothing changed. This box is still running its previous settings.",
    sk: "Nič sa nezmenilo. Zariadenie stále beží s predchádzajúcimi nastaveniami.",
    de: "Nichts hat sich geändert. Die Box läuft weiter mit ihren bisherigen Einstellungen.",
  },
  "settings.form.joinedTitle": {
    en: "Applying changes",
    sk: "Zmeny sa používajú",
    de: "Änderungen werden angewendet",
  },
  "settings.form.joinedMessage": {
    en: "This box is already rebuilding itself.",
    sk: "Zariadenie sa už znovu zostavuje.",
    de: "Die Box baut sich bereits neu.",
  },
  "settings.form.resetTitle": {
    en: "Putting everything back",
    sk: "Všetko sa vracia do pôvodného stavu",
    de: "Alles wird zurückgesetzt",
  },
  "settings.form.starting": { en: "Starting…", sk: "Spúšťa sa…", de: "Wird gestartet…" },
  "settings.form.didNotStart": {
    en: "That did not start",
    sk: "Nepodarilo sa to spustiť",
    de: "Das ließ sich nicht starten",
  },
  "settings.form.noAnswer": {
    en: "This box did not answer.",
    sk: "Zariadenie neodpovedalo.",
    de: "Die Box hat nicht geantwortet.",
  },

  // ── Catalogue search ──────────────────────────────────────────────────────
  "settings.catalogue.noAnswer": {
    en: "The search did not come back.",
    sk: "Vyhľadávanie neodpovedalo.",
    de: "Die Suche hat nicht geantwortet.",
  },

  // ── Formatters ────────────────────────────────────────────────────────────
  "settings.format.noTime": { en: "no time", sk: "žiadny čas", de: "keine Zeit" },
  "settings.format.minutesShort": { en: "{count} min", sk: "{count} min", de: "{count} Min." },
  "settings.format.hoursMinutes": {
    en: "{hours} h {minutes} min",
    sk: "{hours} h {minutes} min",
    de: "{hours} Std. {minutes} Min.",
  },
  "settings.format.hours": {
    en: { one: "{count} hour", other: "{count} hours" },
    sk: {
      one: "{count} hodina",
      few: "{count} hodiny",
      many: "{count} hodín",
      other: "{count} hodín",
    },
    de: { one: "{count} Stunde", other: "{count} Stunden" },
  },
  "settings.format.minutes": {
    en: { one: "{count} minute", other: "{count} minutes" },
    sk: {
      one: "{count} minúta",
      few: "{count} minúty",
      many: "{count} minút",
      other: "{count} minút",
    },
    de: { one: "{count} Minute", other: "{count} Minuten" },
  },

  // ── Compute window ────────────────────────────────────────────────────────
  "settings.window.notSet": {
    en: "The hours are not set.",
    sk: "Hodiny nie sú nastavené.",
    de: "Die Stunden sind nicht festgelegt.",
  },
  "settings.window.nothing": {
    en: "Nothing is shared: the window starts and ends at the same minute.",
    sk: "Nič sa nezdieľa: okno sa začína aj končí v tú istú minútu.",
    de: "Nichts wird geteilt: Das Zeitfenster beginnt und endet in derselben Minute.",
  },
  "settings.window.shared": {
    en: "Shared from {start} until {end}. That is {length} a day, your local time.",
    sk: "Zdieľa sa od {start} do {end}. To je {length} denne, podľa vášho miestneho času.",
    de: "Geteilt von {start} bis {end}. Das ergibt {length} am Tag, in deiner Ortszeit.",
  },
  "settings.window.sharedWraps": {
    en: "Shared from {start} until {end}, across midnight. That is {length} a day, your local time.",
    sk: "Zdieľa sa od {start} do {end}, cez polnoc. To je {length} denne, podľa vášho miestneho času.",
    de: "Geteilt von {start} bis {end}, über Mitternacht. Das ergibt {length} am Tag, in deiner Ortszeit.",
  },

  // ── Capacity meter ────────────────────────────────────────────────────────
  "settings.capacity.label": {
    en: "{used} in use by this box, {lent} holding copies for the mesh, {free} free of {total}.",
    sk: "{used} používa toto zariadenie, {lent} zaberajú kópie pre sieť mesh, {free} voľných z {total}.",
    de: "{used} nutzt diese Box, {lent} belegen Kopien für das Mesh, {free} von {total} frei.",
  },
  "settings.capacity.yours": { en: "Yours", sk: "Vaše", de: "Deins" },
  "settings.capacity.lent": {
    en: "Lent to the mesh",
    sk: "Požičané sieti mesh",
    de: "An das Mesh verliehen",
  },
  "settings.capacity.free": { en: "Free", sk: "Voľné", de: "Frei" },
  "settings.capacity.total": { en: "{total} in total", sk: "spolu {total}", de: "{total} insgesamt" },
  "settings.capacity.unmeasured": {
    en: "This box has not reported how full its disk is.",
    sk: "Zariadenie nenahlásilo, aký plný je jeho disk.",
    de: "Die Box hat nicht gemeldet, wie voll ihre Festplatte ist.",
  },
  "settings.capacity.loading": {
    en: "Asking this box how full its disk is…",
    sk: "Zisťuje sa, aký plný je disk zariadenia…",
    de: "Die Box wird gefragt, wie voll ihre Festplatte ist…",
  },
  "settings.capacity.error": {
    en: "This box did not answer when asked how full its disk is.",
    sk: "Zariadenie neodpovedalo na otázku, aký plný je jeho disk.",
    de: "Die Box hat auf die Frage, wie voll ihre Festplatte ist, nicht geantwortet.",
  },
  "settings.capacity.absent": {
    en: "This box cannot yet report how full its disk is, so there is nothing to draw here.",
    sk: "Zariadenie zatiaľ nevie nahlásiť, aký plný je jeho disk, takže tu nie je čo zobraziť.",
    de: "Die Box kann noch nicht melden, wie voll ihre Festplatte ist, deshalb gibt es hier nichts anzuzeigen.",
  },
  "settings.capacity.noSize": {
    en: "This box answered without a size for its disk.",
    sk: "Zariadenie odpovedalo, ale bez veľkosti disku.",
    de: "Die Box hat geantwortet, aber ohne die Größe ihrer Festplatte.",
  },
});
