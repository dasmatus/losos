import { defineMessages } from "./define";

/* Settings › Logs (screens/settings/pane-logs.tsx, and the page without
 * JavaScript the log reader draws, backend/plugins/page/src/logs.rs): the
 * journal of the box's services. The pane's name, summary and search words
 * live under settings.panes.logs.* with the other panes'. */

export default defineMessages({
  "logs.loading": { en: "Reading the journal…", sk: "Čítam denník…", de: "Lese das Journal…" },
  "logs.retry": { en: "Try again", sk: "Skúsiť znova", de: "Erneut versuchen" },
  "logs.error": {
    en: "The journal could not be read: {message}",
    sk: "Denník sa nepodarilo prečítať: {message}",
    de: "Das Journal konnte nicht gelesen werden: {message}",
  },
  "logs.badFilter": {
    en: "This page does not know that filter, so it shows lososd's lines from this boot.",
    sk: "Tento filter stránka nepozná, preto zobrazuje riadky lososd od tohto spustenia.",
    de: "Diesen Filter kennt die Seite nicht, daher zeigt sie die Zeilen von lososd seit diesem Start.",
  },

  // ── The filter ──────────────────────────────────────────────────────────
  "logs.filter.source": { en: "Service", sk: "Služba", de: "Dienst" },
  "logs.filter.priority": { en: "Severity", sk: "Závažnosť", de: "Schweregrad" },
  "logs.filter.span": { en: "Since", sk: "Od", de: "Seit" },
  "logs.filter.text": { en: "Containing", sk: "Obsahuje", de: "Enthält" },
  "logs.filter.textPlaceholder": { en: "Any text", sk: "Ľubovoľný text", de: "Beliebiger Text" },
  "logs.filter.show": { en: "Show", sk: "Zobraziť", de: "Anzeigen" },
  "logs.follow": { en: "Follow", sk: "Sledovať", de: "Mitlesen" },
  "logs.stop": { en: "Stop following", sk: "Prestať sledovať", de: "Mitlesen beenden" },
  "logs.following": {
    en: "Following: new lines appear as the box writes them.",
    sk: "Sledovanie: nové riadky sa zobrazia, ako ich zariadenie zapíše.",
    de: "Mitlesen: Neue Zeilen erscheinen, sobald die Box sie schreibt.",
  },
  "logs.download": { en: "Download", sk: "Stiahnuť", de: "Herunterladen" },
  "logs.downloadFailed": {
    en: "The lines could not be downloaded.",
    sk: "Riadky sa nepodarilo stiahnuť.",
    de: "Die Zeilen konnten nicht heruntergeladen werden.",
  },
  "logs.older": { en: "Show older lines", sk: "Zobraziť staršie riadky", de: "Ältere Zeilen anzeigen" },
  "logs.hidden": {
    en: "Passwords, keys and tokens are hidden before a line leaves the journal, in the download too.",
    sk: "Heslá, kľúče a tokeny sa skryjú skôr, než riadok opustí denník, aj v stiahnutom súbore.",
    de: "Passwörter, Schlüssel und Tokens werden ausgeblendet, bevor eine Zeile das Journal verlässt, auch im Download.",
  },

  // ── Severities ──────────────────────────────────────────────────────────
  "logs.priority.3": { en: "Errors", sk: "Chyby", de: "Fehler" },
  "logs.priority.4": { en: "Warnings and errors", sk: "Upozornenia a chyby", de: "Warnungen und Fehler" },
  "logs.priority.5": { en: "Notices and worse", sk: "Oznámenia a horšie", de: "Hinweise und schwerer" },
  "logs.priority.6": { en: "Everything but debug", sk: "Všetko okrem ladenia", de: "Alles außer Debug" },
  "logs.priority.7": { en: "Everything", sk: "Všetko", de: "Alles" },
  "logs.level.0": { en: "Emergency", sk: "Núdza", de: "Notfall" },
  "logs.level.1": { en: "Alert", sk: "Poplach", de: "Alarm" },
  "logs.level.2": { en: "Critical", sk: "Kritické", de: "Kritisch" },
  "logs.level.3": { en: "Error", sk: "Chyba", de: "Fehler" },
  "logs.level.4": { en: "Warning", sk: "Upozornenie", de: "Warnung" },
  "logs.level.5": { en: "Notice", sk: "Oznámenie", de: "Hinweis" },
  "logs.level.6": { en: "Info", sk: "Info", de: "Info" },
  "logs.level.7": { en: "Debug", sk: "Ladenie", de: "Debug" },

  // ── Since ───────────────────────────────────────────────────────────────
  "logs.span.boot": { en: "This start", sk: "Toto spustenie", de: "Dieser Start" },
  "logs.span.previous": { en: "The start before", sk: "Predchádzajúce spustenie", de: "Der Start davor" },
  "logs.span.hour": { en: "The last hour", sk: "Posledná hodina", de: "Die letzte Stunde" },
  "logs.span.day": { en: "The last day", sk: "Posledný deň", de: "Der letzte Tag" },
  "logs.span.week": { en: "The last week", sk: "Posledný týždeň", de: "Die letzte Woche" },

  // ── The services ────────────────────────────────────────────────────────
  "logs.source.lososd": { en: "Control service (lososd)", sk: "Riadiaca služba (lososd)", de: "Steuerdienst (lososd)" },
  "logs.source.rebuilds": { en: "Updates and changes", sk: "Aktualizácie a zmeny", de: "Updates und Änderungen" },
  "logs.source.login": { en: "LosOS Login", sk: "LosOS Login", de: "LosOS Login" },
  "logs.source.web": { en: "Web server", sk: "Webový server", de: "Webserver" },
  "logs.source.cloud": { en: "LosOS Cloud", sk: "LosOS Cloud", de: "LosOS Cloud" },
  "logs.source.git": { en: "LosOS Git", sk: "LosOS Git", de: "LosOS Git" },
  "logs.source.office": { en: "LosOS Office", sk: "LosOS Office", de: "LosOS Office" },
  "logs.source.mail": { en: "LosOS Mail", sk: "LosOS Mail", de: "LosOS Mail" },
  "logs.source.chat": { en: "LosOS Chat", sk: "LosOS Chat", de: "LosOS Chat" },
  "logs.source.calls": { en: "Calls", sk: "Hovory", de: "Anrufe" },
  "logs.source.apps": { en: "Apps", sk: "Aplikácie", de: "Apps" },
  "logs.source.ci": { en: "Mesh CI", sk: "Mesh CI", de: "Mesh CI" },
  "logs.source.edge": { en: "Edge tunnel", sk: "Tunel k edge", de: "Edge-Tunnel" },
  "logs.source.mesh": { en: "Mesh and cluster", sk: "Sieť a klaster", de: "Mesh und Cluster" },
  "logs.source.backup": { en: "Backups", sk: "Zálohy", de: "Backups" },
  "logs.source.boot": { en: "Start-up", sk: "Spustenie", de: "Systemstart" },
  "logs.source.logs": { en: "This page's reader", sk: "Čítačka tejto stránky", de: "Leser dieser Seite" },
  "logs.source.kernel": { en: "Kernel", sk: "Jadro", de: "Kernel" },
  "logs.source.losos": { en: "All LosOS services", sk: "Všetky služby LosOS", de: "Alle LosOS-Dienste" },
  "logs.source.everything": { en: "The whole journal", sk: "Celý denník", de: "Das ganze Journal" },

  // ── Nothing to show ─────────────────────────────────────────────────────
  "logs.empty.title": { en: "No lines", sk: "Žiadne riadky", de: "Keine Zeilen" },
  "logs.empty.body": {
    en: "Nothing matches this filter. Try a lower severity or a longer time.",
    sk: "Tomuto filtru nič nezodpovedá. Skúste nižšiu závažnosť alebo dlhší čas.",
    de: "Nichts passt zu diesem Filter. Versuche einen niedrigeren Schweregrad oder eine längere Zeit.",
  },

  // ── lososd down ─────────────────────────────────────────────────────────
  "logs.down.title": {
    en: "The control service does not answer",
    sk: "Riadiaca služba neodpovedá",
    de: "Der Steuerdienst antwortet nicht",
  },
  "logs.down.body": {
    en: "The other admin pages wait for it. This page reads the journal on its own, so its own lines may say why.",
    sk: "Ostatné stránky správy na ňu čakajú. Táto stránka číta denník sama, takže jej vlastné riadky môžu povedať prečo.",
    de: "Die anderen Verwaltungsseiten warten auf ihn. Diese Seite liest das Journal selbst, also sagen seine eigenen Zeilen vielleicht, warum.",
  },
  "logs.down.see": { en: "See lososd's lines", sk: "Zobraziť riadky lososd", de: "Zeilen von lososd anzeigen" },
  "logs.seeLogs": { en: "See the box's logs", sk: "Zobraziť záznamy zariadenia", de: "Protokolle der Box ansehen" },

  // ── Signing in, without JavaScript ──────────────────────────────────────
  "logs.signIn.title": { en: "Sign in to read the logs", sk: "Prihláste sa a čítajte záznamy", de: "Melde dich an, um die Protokolle zu lesen" },
  "logs.signIn.body": {
    en: "The logs are for administrators. The spare key works here even while the rest of the admin pages are down.",
    sk: "Záznamy sú pre správcov. Náhradný kľúč tu funguje, aj keď ostatné stránky správy nefungujú.",
    de: "Die Protokolle sind für Administratoren. Der Ersatzschlüssel funktioniert hier, auch wenn die übrigen Verwaltungsseiten nicht laufen.",
  },
  "logs.signIn.login": { en: "Sign in with LosOS Login", sk: "Prihlásiť sa cez LosOS Login", de: "Mit LosOS Login anmelden" },
  "logs.signIn.key": { en: "Spare key", sk: "Náhradný kľúč", de: "Ersatzschlüssel" },
  "logs.signIn.keySubmit": { en: "Open with the spare key", sk: "Otvoriť náhradným kľúčom", de: "Mit dem Ersatzschlüssel öffnen" },
  "logs.signIn.wrong": {
    en: "That is not this box's spare key.",
    sk: "Toto nie je náhradný kľúč tohto zariadenia.",
    de: "Das ist nicht der Ersatzschlüssel dieser Box.",
  },
  "logs.denied.title": { en: "For administrators", sk: "Pre správcov", de: "Für Administratoren" },
  "logs.denied.body": {
    en: "The box's logs say a lot about everyone who uses it, so only an administrator can read them.",
    sk: "Záznamy zariadenia prezrádzajú veľa o každom, kto ho používa, preto ich môže čítať iba správca.",
    de: "Die Protokolle der Box verraten viel über alle, die sie nutzen, deshalb kann nur ein Administrator sie lesen.",
  },
});
