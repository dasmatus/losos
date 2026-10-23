import { defineMessages } from "./define";

/* The command palette (src/palette/): Ctrl/Cmd+Shift+P, a colon, or the
 * search button at the top of the sidebar. LosOS Cloud and LosOS Git are product names and stay
 * the same in every language. */
export default defineMessages({
  "palette.title": { en: "Search", sk: "Hľadať", de: "Suchen" },
  "palette.label": {
    en: "Search pages, apps, files, repositories and issues",
    sk: "Hľadať stránky, aplikácie, súbory, repozitáre a úlohy",
    de: "Seiten, Apps, Dateien, Repositorys und Issues durchsuchen",
  },
  "palette.placeholder": {
    en: "Search pages, apps, files, repositories and issues",
    sk: "Hľadať stránky, aplikácie, súbory, repozitáre a úlohy",
    de: "Seiten, Apps, Dateien, Repositorys und Issues suchen",
  },
  "palette.open": {
    en: "Search or jump to ({shortcut})",
    sk: "Hľadať alebo prejsť na ({shortcut})",
    de: "Suchen oder springen ({shortcut})",
  },
  "palette.results": { en: "Results", sk: "Výsledky", de: "Ergebnisse" },
  "palette.looking": { en: "Looking", sk: "Hľadám", de: "Suche läuft" },
  "palette.noMatch": {
    en: "Nothing matches \"{query}\".",
    sk: "Hľadaniu „{query}“ nič nezodpovedá.",
    de: "Nichts passt zu „{query}“.",
  },

  "palette.group.pages": { en: "Pages", sk: "Stránky", de: "Seiten" },
  "palette.group.apps": { en: "Apps", sk: "Aplikácie", de: "Apps" },
  "palette.group.files": { en: "Files in LosOS Cloud", sk: "Súbory v LosOS Cloud", de: "Dateien in LosOS Cloud" },
  "palette.group.repos": {
    en: "Repositories in LosOS Git",
    sk: "Repozitáre v LosOS Git",
    de: "Repositorys in LosOS Git",
  },
  "palette.group.issues": { en: "Issues in LosOS Git", sk: "Úlohy v LosOS Git", de: "Issues in LosOS Git" },
  "palette.group.text": { en: "Text in files", sk: "Text v súboroch", de: "Text in Dateien" },
  "palette.group.install": { en: "Apps to install", sk: "Aplikácie na inštaláciu", de: "Apps zum Installieren" },
  "palette.group.actions": { en: "Actions", sk: "Akcie", de: "Aktionen" },

  "palette.inCloud": { en: "LosOS Cloud", sk: "LosOS Cloud", de: "LosOS Cloud" },
  "palette.inGit": { en: "LosOS Git", sk: "LosOS Git", de: "LosOS Git" },

  "palette.files.looking": { en: "Looking in LosOS Cloud", sk: "Hľadám v LosOS Cloud", de: "Suche in LosOS Cloud" },
  "palette.files.signIn": {
    en: "File search works after your next sign-in with your password.",
    sk: "Hľadanie v súboroch začne fungovať po vašom ďalšom prihlásení heslom.",
    de: "Die Dateisuche funktioniert ab deiner nächsten Anmeldung mit dem Passwort.",
  },
  "palette.files.notUp": {
    en: "LosOS Cloud is not running, so its files cannot be searched.",
    sk: "LosOS Cloud nebeží, takže v jeho súboroch sa nedá hľadať.",
    de: "LosOS Cloud läuft nicht, darum lassen sich seine Dateien nicht durchsuchen.",
  },
  "palette.files.failed": {
    en: "LosOS Cloud did not answer.",
    sk: "LosOS Cloud neodpovedal.",
    de: "LosOS Cloud hat nicht geantwortet.",
  },
  "palette.issues.looking": { en: "Looking for issues", sk: "Hľadám úlohy", de: "Suche nach Issues" },
  "palette.issues.failed": {
    en: "LosOS Git could not search its issues.",
    sk: "LosOS Git nemohol prehľadať úlohy.",
    de: "LosOS Git konnte seine Issues nicht durchsuchen.",
  },
  "palette.repos.looking": { en: "Looking in LosOS Git", sk: "Hľadám v LosOS Git", de: "Suche in LosOS Git" },
  "palette.repos.notUp": { en: "LosOS Git is not running.", sk: "LosOS Git nebeží.", de: "LosOS Git läuft nicht." },
  "palette.repos.failed": {
    en: "LosOS Git did not answer.",
    sk: "LosOS Git neodpovedal.",
    de: "LosOS Git hat nicht geantwortet.",
  },
  "palette.text.looking": { en: "Looking inside files", sk: "Hľadám v obsahu súborov", de: "Suche in den Dateien" },
  "palette.text.partial": {
    en: "LosOS Cloud is still reading your files, so more may turn up.",
    sk: "LosOS Cloud ešte číta vaše súbory, takže môže pribudnúť viac.",
    de: "LosOS Cloud liest deine Dateien noch, es kann also mehr dazukommen.",
  },
  "palette.text.failed": {
    en: "The search inside files did not answer.",
    sk: "Hľadanie v obsahu súborov neodpovedalo.",
    de: "Die Suche in den Dateien hat nicht geantwortet.",
  },
  "palette.text.line": { en: "line {line}", sk: "riadok {line}", de: "Zeile {line}" },
  "palette.install.looking": {
    en: "Searching the app catalogue",
    sk: "Hľadám v katalógu aplikácií",
    de: "Suche im App-Katalog",
  },
  "palette.install.failed": {
    en: "The app catalogue did not answer.",
    sk: "Katalóg aplikácií neodpovedal.",
    de: "Der App-Katalog hat nicht geantwortet.",
  },

  "palette.action.light": { en: "Use the light theme", sk: "Použiť svetlý vzhľad", de: "Helles Design verwenden" },
  "palette.action.dark": { en: "Use the dark theme", sk: "Použiť tmavý vzhľad", de: "Dunkles Design verwenden" },
  "palette.action.auto": {
    en: "Match the browser's theme",
    sk: "Vzhľad podľa prehliadača",
    de: "Design wie im Browser",
  },
  "palette.action.language": {
    en: "Switch to {language}",
    sk: "Prepnúť na {language}",
    de: "Auf {language} umstellen",
  },
  "palette.action.tour": { en: "Take the tour", sk: "Spustiť prehliadku", de: "Tour starten" },
  "palette.action.handbook": { en: "Open the handbook", sk: "Otvoriť príručku", de: "Handbuch öffnen" },

  "palette.hint.move": { en: "to move", sk: "posun", de: "bewegen" },
  "palette.hint.open": { en: "to open", sk: "otvoriť", de: "öffnen" },
  "palette.hint.close": { en: "to close", sk: "zavrieť", de: "schließen" },
});
