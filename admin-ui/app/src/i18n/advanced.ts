import { defineMessages } from "./define";

/* The Advanced pane (every option the box declares, with a typed editor
 * each) and the History pane (the configuration repository on LosOS Git).
 * The pane names and summaries sit with the other panes in settings.ts. */

export default defineMessages({
  // ── Advanced: the pane ────────────────────────────────────────────────────
  "advanced.search.label": {
    en: "Find an option",
    sk: "Nájsť možnosť",
    de: "Option suchen",
  },
  "advanced.search.placeholder": {
    en: "Name or description…",
    sk: "Názov alebo popis…",
    de: "Name oder Beschreibung…",
  },
  "advanced.search.noMatch": {
    en: "No option matches “{query}”.",
    sk: "Žiadna možnosť nezodpovedá „{query}“.",
    de: "Keine Option passt zu „{query}“.",
  },
  "advanced.search.count": {
    en: { one: "{count} option", other: "{count} options" },
    sk: { one: "{count} možnosť", few: "{count} možnosti", many: "{count} možností", other: "{count} možností" },
    de: { one: "{count} Option", other: "{count} Optionen" },
  },
  "advanced.caption": {
    en: "Every setting this box declares, read from the modules it runs. A value set here is written to the box’s configuration on Apply, like any other setting, and committed to its history. Leave an option at its default unless you know what it does: the ones marked Careful can leave the box unreachable, and there is no other way in.",
    sk: "Každé nastavenie, ktoré toto zariadenie deklaruje, načítané z modulov, ktoré beží. Hodnota nastavená tu sa pri Použití zapíše do konfigurácie zariadenia ako každé iné nastavenie a uloží sa do jeho histórie. Možnosť nechajte na predvolenej hodnote, ak neviete, čo robí: tie označené Opatrne môžu zariadenie spraviť nedostupným a iná cesta dnu nie je.",
    de: "Jede Einstellung, die diese Box deklariert, gelesen aus den Modulen, die sie ausführt. Ein hier gesetzter Wert wird beim Anwenden wie jede andere Einstellung in die Konfiguration der Box geschrieben und in ihrer Historie festgehalten. Lass eine Option auf ihrem Standard, wenn du nicht weißt, was sie tut: die mit Vorsicht markierten können die Box unerreichbar machen, und einen anderen Weg hinein gibt es nicht.",
  },
  "advanced.unavailable": {
    en: "This box does not serve its option document yet. Apply still writes the settings on the other panes; the rest appears here after the next update.",
    sk: "Toto zariadenie zatiaľ neposkytuje dokument so svojimi možnosťami. Použiť stále zapisuje nastavenia z ostatných panelov; zvyšok sa tu objaví po ďalšej aktualizácii.",
    de: "Diese Box stellt ihr Optionsdokument noch nicht bereit. Anwenden schreibt weiterhin die Einstellungen der anderen Bereiche; der Rest erscheint hier nach dem nächsten Update.",
  },
  "advanced.excluded": {
    en: "Left out: losos.{prefix}.* — {reason}.",
    sk: "Vynechané: losos.{prefix}.* — {reason}.",
    de: "Ausgelassen: losos.{prefix}.* — {reason}.",
  },
  "advanced.group.general": {
    en: "General",
    sk: "Všeobecné",
    de: "Allgemein",
  },
  "advanced.row.default": {
    en: "Default: {text}",
    sk: "Predvolené: {text}",
    de: "Standard: {text}",
  },
  "advanced.row.running": {
    en: "Running with: {text}",
    sk: "Beží s: {text}",
    de: "Läuft mit: {text}",
  },
  "advanced.row.noDefault": {
    en: "No default",
    sk: "Bez predvolenej hodnoty",
    de: "Kein Standard",
  },
  "advanced.badge.installer": {
    en: "Set by the installer",
    sk: "Nastavil inštalátor",
    de: "Vom Installer gesetzt",
  },
  "advanced.badge.fixed": {
    en: "Set by the {owner}",
    sk: "Nastavuje {owner}",
    de: "Gesetzt durch {owner}",
  },
  "advanced.badge.flake": {
    en: "Chosen by the build",
    sk: "Zvolené zostavením",
    de: "Vom Build gewählt",
  },
  "advanced.badge.danger": {
    en: "Careful",
    sk: "Opatrne",
    de: "Vorsicht",
  },
  "advanced.badge.set": {
    en: "Set here",
    sk: "Nastavené tu",
    de: "Hier gesetzt",
  },
  "advanced.badge.stray": {
    en: "Not an option here",
    sk: "Tu nie je možnosťou",
    de: "Hier keine Option",
  },
  "advanced.badge.needs": {
    en: "Unavailable",
    sk: "Nedostupné",
    de: "Nicht verfügbar",
  },
  "advanced.needs": {
    en: "Takes effect only while losos.{name} is on, and it is off.",
    sk: "Platí len vtedy, keď je losos.{name} zapnuté, a to je vypnuté.",
    de: "Wirkt nur, solange losos.{name} an ist, und das ist aus.",
  },
  "advanced.needs.sharingMyStorage": {
    en: "Federation runs only while this box shares its disk (losos.sharingMyStorage), which unlocks the shared data pool. Sharing is off, so this does nothing for now.",
    sk: "Federácia beží len vtedy, keď toto zariadenie zdieľa svoj disk (losos.sharingMyStorage), čím sa odomkne zdieľaný dátový priestor. Zdieľanie je vypnuté, takže toto nastavenie zatiaľ nič nerobí.",
    de: "Föderation läuft nur, solange diese Box ihre Festplatte teilt (losos.sharingMyStorage), was den geteilten Datenbereich entsperrt. Teilen ist aus, also bewirkt das vorerst nichts.",
  },
  "advanced.owned.changeIn": {
    en: "Change under {pane}",
    sk: "Zmeniť v časti {pane}",
    de: "Unter {pane} ändern",
  },
  "advanced.useDefault": {
    en: "Use default",
    sk: "Použiť predvolené",
    de: "Standard verwenden",
  },
  "advanced.on": { en: "On", sk: "Zapnuté", de: "An" },
  "advanced.off": { en: "Off", sk: "Vypnuté", de: "Aus" },
  "advanced.none": { en: "none", sk: "žiadne", de: "keine" },
  "advanced.nullable.hint": {
    en: "Empty leaves it unset.",
    sk: "Prázdne pole ho ponechá nenastavené.",
    de: "Leer lässt es ungesetzt.",
  },
  "advanced.list.hint": {
    en: "One per line.",
    sk: "Jeden na riadok.",
    de: "Einer pro Zeile.",
  },
  "advanced.value.label": {
    en: "Value of losos.{name}",
    sk: "Hodnota losos.{name}",
    de: "Wert von losos.{name}",
  },
  "advanced.stray.title": {
    en: "Lines this box has no option for",
    sk: "Riadky, pre ktoré toto zariadenie nemá možnosť",
    de: "Zeilen, für die diese Box keine Option hat",
  },
  "advanced.stray.caption": {
    en: "These are in the box’s configuration but match no option it declares — a typo, or a setting from another version. The box refuses to apply while they are there, so remove them to apply anything.",
    sk: "Tieto sú v konfigurácii zariadenia, ale nezodpovedajú žiadnej možnosti, ktorú deklaruje — preklep alebo nastavenie z inej verzie. Kým tam sú, zariadenie odmieta použiť zmeny, takže ich odstráňte, aby sa dalo čokoľvek použiť.",
    de: "Diese stehen in der Konfiguration der Box, passen aber zu keiner Option, die sie deklariert — ein Tippfehler oder eine Einstellung aus einer anderen Version. Solange sie da sind, verweigert die Box das Anwenden; entferne sie, um etwas anzuwenden.",
  },
  "advanced.stray.remove": { en: "Remove", sk: "Odstrániť", de: "Entfernen" },
  "advanced.danger.title": {
    en: "Change losos.{name}?",
    sk: "Zmeniť losos.{name}?",
    de: "losos.{name} ändern?",
  },
  "advanced.danger.body": {
    en: "A wrong value here can leave this box unreachable, unable to boot, or locked out of the page you are on. There is no shell and no SSH to put it right from — only this page, and the installer.",
    sk: "Nesprávna hodnota tu môže zariadenie spraviť nedostupným, nespustiteľným alebo vás odstaviť od stránky, na ktorej ste. Nie je tu shell ani SSH, z ktorého by sa to dalo opraviť — len táto stránka a inštalátor.",
    de: "Ein falscher Wert hier kann diese Box unerreichbar machen, am Booten hindern oder dich von dieser Seite aussperren. Es gibt keine Shell und kein SSH, um das zu beheben — nur diese Seite und den Installer.",
  },
  "advanced.danger.keep": { en: "Keep as is", sk: "Nechať tak", de: "So lassen" },
  "advanced.danger.confirm": { en: "Change it", sk: "Zmeniť", de: "Ändern" },
  "advanced.commits": {
    en: "Every Apply is committed to the box’s configuration history, which you can read under History.",
    sk: "Každé Použitie sa uloží do histórie konfigurácie zariadenia, ktorú si môžete prečítať v časti História.",
    de: "Jedes Anwenden wird in der Konfigurationshistorie der Box festgehalten, die du unter Historie lesen kannst.",
  },

  // ── Advanced: why a value does not fit ───────────────────────────────────
  "advanced.fit.bool": {
    en: "Must be on or off.",
    sk: "Musí byť zapnuté alebo vypnuté.",
    de: "Muss an oder aus sein.",
  },
  "advanced.fit.int": {
    en: "Must be a whole number.",
    sk: "Musí byť celé číslo.",
    de: "Muss eine ganze Zahl sein.",
  },
  "advanced.fit.between": {
    en: "Must be between {min} and {max}.",
    sk: "Musí byť medzi {min} a {max}.",
    de: "Muss zwischen {min} und {max} liegen.",
  },
  "advanced.fit.atLeast": {
    en: "Must be at least {min}.",
    sk: "Musí byť aspoň {min}.",
    de: "Muss mindestens {min} sein.",
  },
  "advanced.fit.atMost": {
    en: "Must be at most {max}.",
    sk: "Musí byť najviac {max}.",
    de: "Darf höchstens {max} sein.",
  },
  "advanced.fit.float": {
    en: "Must be a decimal number such as 0.25.",
    sk: "Musí byť desatinné číslo, napríklad 0.25.",
    de: "Muss eine Dezimalzahl wie 0.25 sein.",
  },
  "advanced.fit.string": {
    en: "Must be text.",
    sk: "Musí byť text.",
    de: "Muss Text sein.",
  },
  "advanced.fit.oneOf": {
    en: "Must be one of: {values}.",
    sk: "Musí byť jedno z: {values}.",
    de: "Muss eines von: {values} sein.",
  },
  "advanced.fit.list": {
    en: "Must be a list of text lines.",
    sk: "Musí byť zoznam textových riadkov.",
    de: "Muss eine Liste von Textzeilen sein.",
  },
  "advanced.fit.interpolation": {
    en: "Must not contain ${.",
    sk: "Nesmie obsahovať ${.",
    de: "Darf kein ${ enthalten.",
  },
  "advanced.fit.opaque": {
    en: "Cannot be set from here.",
    sk: "Odtiaľto sa nedá nastaviť.",
    de: "Kann von hier nicht gesetzt werden.",
  },
  "advanced.fit.stray": {
    en: "Not an option this box declares; remove it to apply.",
    sk: "Nie je možnosťou, ktorú toto zariadenie deklaruje; odstráňte ju, aby sa dalo použiť.",
    de: "Keine Option, die diese Box deklariert; entferne sie, um anzuwenden.",
  },

  // ── History ──────────────────────────────────────────────────────────────
  "history.repo.title": {
    en: "Where the configuration lives",
    sk: "Kde je konfigurácia uložená",
    de: "Wo die Konfiguration liegt",
  },
  "history.repo.onGit": {
    en: "On LosOS Git",
    sk: "V LosOS Git",
    de: "In LosOS Git",
  },
  "history.repo.open": { en: "Open", sk: "Otvoriť", de: "Öffnen" },
  "history.repo.branch": { en: "Branch", sk: "Vetva", de: "Branch" },
  "history.repo.head": {
    en: "Latest change on the box",
    sk: "Posledná zmena na zariadení",
    de: "Letzte Änderung auf der Box",
  },
  "history.repo.clone": {
    en: "Clone address",
    sk: "Adresa na klonovanie",
    de: "Clone-Adresse",
  },
  "history.repo.off": {
    en: "LosOS Git is off on this box, so the configuration stays in the box’s own history only. Every change is still committed there.",
    sk: "LosOS Git je na tomto zariadení vypnutý, takže konfigurácia zostáva len vo vlastnej histórii zariadenia. Každá zmena sa tam stále ukladá.",
    de: "LosOS Git ist auf dieser Box aus, die Konfiguration bleibt also nur in der eigenen Historie der Box. Jede Änderung wird dort trotzdem festgehalten.",
  },
  "history.repo.caption": {
    en: "The box keeps its configuration in a private repository on LosOS Git, owned by your account, and pushes every change there. Clone it with your LosOS Git password, edit modules/overrides.nix, push, and the box takes the commit within a minute and rebuilds — after checking each line the way this page does. A rewritten history is left alone and reported here.",
    sk: "Zariadenie uchováva svoju konfiguráciu v súkromnom repozitári na LosOS Git, ktorý vlastní váš účet, a posiela tam každú zmenu. Naklonujte ho so svojím heslom do LosOS Git, upravte modules/overrides.nix, pošlite zmeny a zariadenie commit do minúty prevezme a prestaví sa — po kontrole každého riadku rovnako, ako to robí táto stránka. Prepísaná história sa nechá tak a nahlási sa tu.",
    de: "Die Box hält ihre Konfiguration in einem privaten Repository auf LosOS Git, das deinem Konto gehört, und pusht jede Änderung dorthin. Klone es mit deinem LosOS-Git-Passwort, bearbeite modules/overrides.nix, pushe, und die Box übernimmt den Commit innerhalb einer Minute und baut neu — nachdem sie jede Zeile so geprüft hat wie diese Seite. Eine umgeschriebene Historie wird nicht angefasst und hier gemeldet.",
  },
  "history.sync.title": {
    en: "Sync with LosOS Git",
    sk: "Synchronizácia s LosOS Git",
    de: "Abgleich mit LosOS Git",
  },
  "history.sync.now": { en: "Sync now", sk: "Synchronizovať teraz", de: "Jetzt abgleichen" },
  "history.sync.syncing": { en: "Syncing…", sk: "Synchronizuje sa…", de: "Gleicht ab…" },
  "history.sync.lastAt": {
    en: "Last checked {when}",
    sk: "Naposledy skontrolované {when}",
    de: "Zuletzt geprüft {when}",
  },
  "history.sync.remote": {
    en: "LosOS Git has {sha}",
    sk: "LosOS Git má {sha}",
    de: "LosOS Git hat {sha}",
  },
  "history.sync.state.off": { en: "Off", sk: "Vypnuté", de: "Aus" },
  "history.sync.state.pending": { en: "Not yet", sk: "Zatiaľ nie", de: "Noch nicht" },
  "history.sync.state.ok": { en: "In sync", sk: "Synchronizované", de: "Synchron" },
  "history.sync.state.waiting": { en: "Waiting", sk: "Čaká sa", de: "Wartet" },
  "history.sync.state.refused": { en: "Refused", sk: "Odmietnuté", de: "Abgelehnt" },
  "history.sync.state.diverged": { en: "Diverged", sk: "Rozišlo sa", de: "Auseinandergelaufen" },
  "history.sync.state.unavailable": {
    en: "LosOS Git not answering",
    sk: "LosOS Git neodpovedá",
    de: "LosOS Git antwortet nicht",
  },
  "history.sync.state.error": { en: "Error", sk: "Chyba", de: "Fehler" },
  "history.commits.title": {
    en: "Changes",
    sk: "Zmeny",
    de: "Änderungen",
  },
  "history.commits.empty": {
    en: "No changes recorded yet.",
    sk: "Zatiaľ nie sú zaznamenané žiadne zmeny.",
    de: "Noch keine Änderungen festgehalten.",
  },
  "history.commits.caption": {
    en: "The last forty. Each Apply, storage change and reset is one commit, named after the settings it changed; the full list is on LosOS Git.",
    sk: "Posledných štyridsať. Každé Použitie, zmena úložiska a reset je jeden commit pomenovaný podľa nastavení, ktoré zmenil; celý zoznam je na LosOS Git.",
    de: "Die letzten vierzig. Jedes Anwenden, jede Speicheränderung und jeder Reset ist ein Commit, benannt nach den geänderten Einstellungen; die vollständige Liste steht auf LosOS Git.",
  },
  "history.loadError": {
    en: "The history could not be read: {message}",
    sk: "Históriu sa nepodarilo načítať: {message}",
    de: "Die Historie konnte nicht gelesen werden: {message}",
  },
  "history.refresh": { en: "Refresh", sk: "Obnoviť", de: "Aktualisieren" },
});
