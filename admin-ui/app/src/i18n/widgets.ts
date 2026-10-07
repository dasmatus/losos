import { defineMessages } from "./define";

/* The widget board: tiles, the gallery, the custom-widget editor, the five
 * built-ins, and the messages the expression language shows the person
 * typing into it. Names, tokens and operators quoted inside a message are
 * the owner's own source text and arrive through placeholders untouched. */
export default defineMessages({
  // ── board.tsx ──────────────────────────────────────────────────────────
  "widgets.board.heading": { en: "Your board", sk: "Vaša nástenka", de: "Deine Pinnwand" },
  "widgets.board.count": { en: "{count} of {max}", sk: "{count} z {max}", de: "{count} von {max}" },
  "widgets.board.add": { en: "Add a widget", sk: "Pridať widget", de: "Widget hinzufügen" },
  "widgets.board.emptyTitle": {
    en: "Nothing on the board yet",
    sk: "Na nástenke zatiaľ nič nie je",
    de: "Noch nichts auf der Pinnwand",
  },
  "widgets.board.emptyBody": {
    en: "Add a widget to keep an eye on the things you care about: how much room is left, whether the box has been answering, which apps are up. They live in this browser and change nothing on the box.",
    sk: "Pridajte widget a majte prehľad o tom, na čom vám záleží: koľko miesta zostáva, či zariadenie odpovedá, ktoré aplikácie bežia. Widgety žijú v tomto prehliadači a na zariadení nič nemenia.",
    de: "Füge ein Widget hinzu, um im Blick zu behalten, was dir wichtig ist: wie viel Platz noch frei ist, ob die Box antwortet, welche Apps laufen. Widgets leben in diesem Browser und ändern nichts an der Box.",
  },

  // ── gallery.tsx ────────────────────────────────────────────────────────
  "widgets.gallery.title": { en: "Add a widget", sk: "Pridať widget", de: "Widget hinzufügen" },
  "widgets.gallery.description": {
    en: "Widgets live in this browser. Adding one changes nothing on the box and starts nothing running.",
    sk: "Widgety žijú v tomto prehliadači. Pridaním widgetu sa na zariadení nič nezmení ani nespustí.",
    de: "Widgets leben in diesem Browser. Ein neues Widget ändert nichts an der Box und startet dort nichts.",
  },
  "widgets.gallery.full": {
    en: "The board is full.",
    sk: "Nástenka je plná.",
    de: "Die Pinnwand ist voll.",
  },
  "widgets.gallery.fullHint": {
    en: "Remove a widget first. {max} is the most.",
    sk: "Najprv odstráňte niektorý widget. Najviac ich môže byť {max}.",
    de: "Entferne zuerst ein Widget. Mehr als {max} gehen nicht.",
  },
  "widgets.gallery.added": { en: "{name} added.", sk: "{name}: pridané.", de: "{name} hinzugefügt." },
  "widgets.gallery.notAdded": {
    en: "That widget could not be added.",
    sk: "Tento widget sa nepodarilo pridať.",
    de: "Dieses Widget konnte nicht hinzugefügt werden.",
  },
  "widgets.gallery.add": { en: "Add", sk: "Pridať", de: "Hinzufügen" },
  "widgets.gallery.addAnother": { en: "Add another", sk: "Pridať ďalší", de: "Noch eins hinzufügen" },
  "widgets.gallery.custom": { en: "Custom", sk: "Vlastný", de: "Eigenes" },
  "widgets.gallery.customBlurb": {
    en: "Build your own from one of the readings this box publishes. Pick a shape, write what to show, and see it before you keep it.",
    sk: "Zostavte si vlastný z niektorého údaja, ktorý toto zariadenie zverejňuje. Vyberte tvar, napíšte, čo sa má zobraziť, a pozrite si ho skôr, než si ho necháte.",
    de: "Bau dir dein eigenes aus einem der Messwerte, die diese Box bereitstellt. Wähle eine Form, schreib, was angezeigt werden soll, und sieh es dir an, bevor du es behältst.",
  },
  "widgets.gallery.buildOne": { en: "Build one", sk: "Zostaviť", de: "Eins bauen" },
  "widgets.gallery.holds": {
    en: "The board holds {max}. Remove one to add another.",
    sk: "Na nástenku sa zmestí {max}. Ak chcete pridať ďalší, jeden odstráňte.",
    de: "Auf die Pinnwand passen {max}. Entferne eins, um ein weiteres hinzuzufügen.",
  },
  "widgets.gallery.done": { en: "Done", sk: "Hotovo", de: "Fertig" },

  // ── catalogue.ts ───────────────────────────────────────────────────────
  "widgets.catalogue.uptime.name": { en: "Answering", sk: "Odpovedá", de: "Antwortet" },
  "widgets.catalogue.uptime.blurb": {
    en: "A year of days, one square each, shaded by how much of the day this box answered.",
    sk: "Rok dní, každý deň jeden štvorček, vyfarbený podľa toho, ako veľkú časť dňa toto zariadenie odpovedalo.",
    de: "Ein Jahr in Tagen, ein Kästchen pro Tag, getönt danach, wie viel des Tages diese Box geantwortet hat.",
  },
  "widgets.catalogue.disk.name": { en: "Room", sk: "Miesto", de: "Platz" },
  "widgets.catalogue.disk.blurb": {
    en: "How much of the disk is in use, and how much is held back for the box to grow into.",
    sk: "Koľko z disku sa používa a koľko je odložené, aby zariadenie malo kam rásť.",
    de: "Wie viel der Festplatte belegt ist und wie viel zurückgehalten wird, damit die Box wachsen kann.",
  },
  "widgets.catalogue.mesh.name": { en: "Sharing", sk: "Zdieľanie", de: "Teilen" },
  "widgets.catalogue.mesh.blurb": {
    en: "The hours a day this box lends its spare time to other boxes, against the hours it keeps.",
    sk: "Koľko hodín denne toto zariadenie požičiava svoj voľný čas iným zariadeniam a koľko si ich necháva.",
    de: "Wie viele Stunden am Tag diese Box ihre freie Zeit anderen Boxen leiht, und wie viele sie für sich behält.",
  },
  "widgets.catalogue.apps.name": { en: "Apps", sk: "Aplikácie", de: "Apps" },
  "widgets.catalogue.apps.blurb": {
    en: "Which apps this box serves, and whether each one answered just now.",
    sk: "Ktoré aplikácie toto zariadenie poskytuje a či každá z nich práve odpovedala.",
    de: "Welche Apps diese Box bereitstellt und ob jede davon gerade geantwortet hat.",
  },
  "widgets.catalogue.rebuilds.name": { en: "Changes", sk: "Zmeny", de: "Änderungen" },
  "widgets.catalogue.rebuilds.blurb": {
    en: "Settings changes this box has applied, newest first, and whether each one took.",
    sk: "Zmeny nastavení, ktoré toto zariadenie použilo, od najnovšej, a či sa každá podarila.",
    de: "Einstellungsänderungen, die diese Box übernommen hat, die neueste zuerst, und ob jede geklappt hat.",
  },

  // ── tile.tsx ───────────────────────────────────────────────────────────
  "widgets.tile.notInVersion": {
    en: "This widget is not part of this version of the box.",
    sk: "Tento widget nie je súčasťou tejto verzie zariadenia.",
    de: "Dieses Widget gehört nicht zu dieser Version der Box.",
  },
  "widgets.tile.couldNotRead": {
    en: "This widget could not be read.",
    sk: "Tento widget sa nepodarilo načítať.",
    de: "Dieses Widget konnte nicht gelesen werden.",
  },
  "widgets.tile.stopped": {
    en: "This widget stopped working.",
    sk: "Tento widget prestal fungovať.",
    de: "Dieses Widget funktioniert nicht mehr.",
  },
  "widgets.tile.nothingToDraw": {
    en: "This widget did not return anything to draw.",
    sk: "Tento widget nevrátil nič na zobrazenie.",
    de: "Dieses Widget hat nichts zum Anzeigen zurückgegeben.",
  },
  "widgets.tile.fallbackTitle": { en: "Widget", sk: "Widget", de: "Widget" },
  "widgets.tile.moveEarlier": {
    en: "Move {title} earlier",
    sk: "Posunúť „{title}“ dopredu",
    de: "„{title}“ nach vorne schieben",
  },
  "widgets.tile.moveLater": {
    en: "Move {title} later",
    sk: "Posunúť „{title}“ dozadu",
    de: "„{title}“ nach hinten schieben",
  },
  "widgets.tile.remove": {
    en: "Remove {title}",
    sk: "Odstrániť „{title}“",
    de: "„{title}“ entfernen",
  },
  "widgets.tile.tryAgain": { en: "Try again", sk: "Skúsiť znova", de: "Erneut versuchen" },

  // ── render/bar.tsx ─────────────────────────────────────────────────────
  "widgets.bar.ofMax": { en: "of {max}", sk: "z {max}", de: "von {max}" },
  "widgets.bar.used": { en: "Used", sk: "Využité", de: "Belegt" },
  "widgets.bar.heldBack": { en: "Held back", sk: "Odložené", de: "Zurückgehalten" },
  "widgets.bar.notMeasured": {
    en: "{name}: not measured",
    sk: "{name}: nezmerané",
    de: "{name}: nicht gemessen",
  },
  "widgets.bar.valueOfMax": {
    en: "{name}: {value} of {max}",
    sk: "{name}: {value} z {max}",
    de: "{name}: {value} von {max}",
  },
  "widgets.bar.reserve": { en: "{name}: {value}", sk: "{name}: {value}", de: "{name}: {value}" },

  // ── render/list.tsx ────────────────────────────────────────────────────
  "widgets.list.empty": {
    en: "Nothing to show yet.",
    sk: "Zatiaľ nie je čo zobraziť.",
    de: "Noch nichts anzuzeigen.",
  },
  "widgets.list.moreHidden": {
    en: "{count} more not shown.",
    sk: {
      one: "{count} ďalší sa nezobrazuje.",
      few: "{count} ďalšie sa nezobrazujú.",
      many: "{count} ďalších sa nezobrazuje.",
      other: "{count} ďalších sa nezobrazuje.",
    },
    de: "{count} weitere nicht angezeigt.",
  },

  // ── render/heatmap.tsx ─────────────────────────────────────────────────
  "widgets.heatmap.dailyRecord": { en: "Daily record", sk: "Denný záznam", de: "Tagesübersicht" },
  "widgets.heatmap.quiet": { en: "Quiet", sk: "Ticho", de: "Still" },
  "widgets.heatmap.answering": { en: "Answering", sk: "Odpovedá", de: "Antwortet" },
  "widgets.heatmap.notWatched": { en: "Not watched", sk: "Nesledované", de: "Nicht beobachtet" },
  "widgets.heatmap.cellNotWatched": {
    en: "{date}: not watched",
    sk: "{date}: nesledované",
    de: "{date}: nicht beobachtet",
  },
  "widgets.heatmap.cellQuiet": {
    en: "{date}: did not answer",
    sk: "{date}: neodpovedalo",
    de: "{date}: hat nicht geantwortet",
  },
  "widgets.heatmap.cellAnswered": {
    en: "{date}: {pct} answered",
    sk: "{date}: odpovedalo {pct}",
    de: "{date}: {pct} geantwortet",
  },
  "widgets.heatmap.cellNote": { en: "{date}: {note}", sk: "{date}: {note}", de: "{date}: {note}" },
  "widgets.heatmap.counts": {
    en: "{watched} watched · {quiet} quiet",
    sk: "sledované: {watched} · ticho: {quiet}",
    de: "{watched} beobachtet · {quiet} still",
  },

  // ── format.ts ──────────────────────────────────────────────────────────
  "widgets.format.percent": { en: "{n}%", sk: "{n} %", de: "{n} %" },
  "widgets.format.seconds": { en: "{n} s", sk: "{n} s", de: "{n} s" },
  "widgets.format.minutes": { en: "{n} min", sk: "{n} min", de: "{n} min" },
  "widgets.format.hours": { en: "{n} h", sk: "{n} h", de: "{n} h" },
  "widgets.format.hoursMinutes": { en: "{h} h {m} min", sk: "{h} h {m} min", de: "{h} h {m} min" },
  "widgets.format.days": { en: "{n} d", sk: "{n} d", de: "{n} T" },
  "widgets.format.daysHours": { en: "{d} d {h} h", sk: "{d} d {h} h", de: "{d} T {h} h" },
  "widgets.format.justNow": { en: "just now", sk: "práve teraz", de: "gerade eben" },
  "widgets.format.ago": { en: "{duration} ago", sk: "pred {duration}", de: "vor {duration}" },

  // ── sandbox.ts ─────────────────────────────────────────────────────────
  "widgets.sandbox.noReading": {
    en: "There is no reading called {name}",
    sk: "Údaj s názvom {name} neexistuje",
    de: "Es gibt keinen Messwert namens {name}",
  },
  "widgets.sandbox.noAnswer": {
    en: "This box did not answer.",
    sk: "Toto zariadenie neodpovedalo.",
    de: "Diese Box hat nicht geantwortet.",
  },
  "widgets.sandbox.signedOut": {
    en: "This tab is no longer signed in.",
    sk: "Táto karta už nie je prihlásená.",
    de: "Dieser Tab ist nicht mehr angemeldet.",
  },
  "widgets.sandbox.forbidden": {
    en: "This page is not allowed to ask for that.",
    sk: "Táto stránka o to nesmie žiadať.",
    de: "Diese Seite darf das nicht abfragen.",
  },
  "widgets.sandbox.notReported": {
    en: "This box does not report that yet.",
    sk: "Toto zariadenie to zatiaľ neoznamuje.",
    de: "Diese Box meldet das noch nicht.",
  },
  "widgets.sandbox.busy": {
    en: "This box is busy or restarting. It will be back.",
    sk: "Toto zariadenie je zaneprázdnené alebo sa reštartuje. Čoskoro bude späť.",
    de: "Diese Box ist beschäftigt oder startet neu. Sie ist gleich wieder da.",
  },

  // ── metrics.ts ─────────────────────────────────────────────────────────
  "widgets.metrics.files": { en: "Files", sk: "Súbory", de: "Dateien" },
  "widgets.metrics.code": { en: "Code", sk: "Kód", de: "Code" },

  // ── builtin.ts: uptime ─────────────────────────────────────────────────
  "widgets.builtin.uptime.title": { en: "Answering", sk: "Odpovedá", de: "Antwortet" },
  "widgets.builtin.uptime.low": { en: "Quiet", sk: "Ticho", de: "Still" },
  "widgets.builtin.uptime.nothingYet": {
    en: "Nothing watched yet",
    sk: "Zatiaľ nič nesledované",
    de: "Noch nichts beobachtet",
  },
  "widgets.builtin.uptime.answered": {
    en: "{pct} answered",
    sk: "odpovedalo {pct}",
    de: "{pct} geantwortet",
  },
  "widgets.builtin.uptime.noQuiet": {
    en: "no quiet spells",
    sk: "žiadne tiché obdobie",
    de: "keine stillen Phasen",
  },
  "widgets.builtin.uptime.quietSpells": {
    en: { one: "{count} quiet spell", other: "{count} quiet spells" },
    sk: {
      one: "{count} tiché obdobie",
      few: "{count} tiché obdobia",
      many: "{count} tichých období",
      other: "{count} tichých období",
    },
    de: { one: "{count} stille Phase", other: "{count} stille Phasen" },
  },
  "widgets.builtin.uptime.longest": {
    en: "longest {duration}",
    sk: "najdlhšie {duration}",
    de: "längste {duration}",
  },
  "widgets.builtin.uptime.notWatched": {
    en: "This browser has not watched this box yet. Leave this page open and squares will fill in.",
    sk: "Tento prehliadač toto zariadenie ešte nesledoval. Nechajte túto stránku otvorenú a štvorčeky sa začnú vypĺňať.",
    de: "Dieser Browser hat diese Box noch nicht beobachtet. Lass diese Seite offen, dann füllen sich die Kästchen.",
  },
  "widgets.builtin.uptime.watched": {
    en: {
      one: "Watched from this browser on {count} day.",
      other: "Watched from this browser on {count} days.",
    },
    sk: {
      one: "Sledované z tohto prehliadača {count} deň.",
      few: "Sledované z tohto prehliadača {count} dni.",
      many: "Sledované z tohto prehliadača {count} dní.",
      other: "Sledované z tohto prehliadača {count} dní.",
    },
    de: {
      one: "Von diesem Browser an {count} Tag beobachtet.",
      other: "Von diesem Browser an {count} Tagen beobachtet.",
    },
  },

  // ── builtin.ts: disk ───────────────────────────────────────────────────
  "widgets.builtin.disk.title": {
    en: "Room on this box",
    sk: "Miesto na tomto zariadení",
    de: "Platz auf dieser Box",
  },
  "widgets.builtin.disk.notMeasured": {
    en: "Not measured yet",
    sk: "Zatiaľ nezmerané",
    de: "Noch nicht gemessen",
  },
  "widgets.builtin.disk.reportsSize": {
    en: "This box reports its size when you claim space on the Storage page.",
    sk: "Zariadenie oznámi svoju veľkosť, keď si na stránke Úložisko sprístupníte miesto.",
    de: "Die Box meldet ihre Größe, sobald du auf der Seite Speicher Platz freischaltest.",
  },
  "widgets.builtin.disk.inUse": { en: "In use", sk: "Používa sa", de: "Belegt" },
  "widgets.builtin.disk.heldBack": {
    en: "Held back to grow into",
    sk: "Odložené na rast",
    de: "Zum Wachsen zurückgehalten",
  },
  "widgets.builtin.disk.fillingUp": { en: "Filling up", sk: "Zapĺňa sa", de: "Wird voll" },
  "widgets.builtin.disk.formatted": {
    en: "{total} formatted, {reserve} not yet claimed",
    sk: "{total} naformátovaných, {reserve} ešte nesprístupnených",
    de: "{total} formatiert, {reserve} noch nicht freigeschaltet",
  },
  "widgets.builtin.disk.free": { en: "{free} free now", sk: "{free} teraz voľných", de: "{free} jetzt frei" },
  "widgets.builtin.disk.freeMore": {
    en: "{free} free now, {reserve} more to claim",
    sk: "{free} teraz voľných, ďalších {reserve} možno sprístupniť",
    de: "{free} jetzt frei, {reserve} mehr zum Freischalten",
  },
  "widgets.builtin.disk.footReserve": {
    en: "{reserve} is held back. You can claim it without opening the box.",
    sk: "{reserve} je odložených. Môžete ich sprístupniť bez otvárania zariadenia.",
    de: "{reserve} sind zurückgehalten. Du kannst sie freischalten, ohne die Box zu öffnen.",
  },
  "widgets.builtin.disk.footAll": {
    en: "All of the disk is in the filesystem.",
    sk: "Celý disk je v súborovom systéme.",
    de: "Die ganze Festplatte ist im Dateisystem.",
  },

  // ── builtin.ts: mesh ───────────────────────────────────────────────────
  "widgets.builtin.mesh.title": { en: "Sharing", sk: "Zdieľanie", de: "Teilen" },
  "widgets.builtin.mesh.alone": {
    en: "This box is on its own",
    sk: "Toto zariadenie je samo",
    de: "Diese Box ist für sich allein",
  },
  "widgets.builtin.mesh.yours": { en: "Yours", sk: "Vaše", de: "Deins" },
  "widgets.builtin.mesh.joinHint": {
    en: "Join other boxes from the Mesh page to lend and borrow time.",
    sk: "Na stránke Mesh sa pripojte k ďalším zariadeniam a požičiavajte si čas navzájom.",
    de: "Tritt auf der Seite Mesh anderen Boxen bei, um Zeit zu verleihen und zu leihen.",
  },
  "widgets.builtin.mesh.runElsewhere": {
    en: "Run for you elsewhere",
    sk: "Bežalo pre vás inde",
    de: "Für dich anderswo gelaufen",
  },
  "widgets.builtin.mesh.runHere": {
    en: "Run here for others",
    sk: "Bežalo tu pre iných",
    de: "Hier für andere gelaufen",
  },
  "widgets.builtin.mesh.givenTaken": {
    en: "{given} given · {taken} taken",
    sk: "dané: {given} · prijaté: {taken}",
    de: "{given} gegeben · {taken} genommen",
  },
  "widgets.builtin.mesh.counted": {
    en: "Counted since this box joined.",
    sk: "Počítané od pripojenia tohto zariadenia.",
    de: "Gezählt, seit diese Box beigetreten ist.",
  },
  "widgets.builtin.mesh.kept": { en: "Kept for you", sk: "Ponechané pre vás", de: "Für dich behalten" },
  "widgets.builtin.mesh.lent": {
    en: "Lent to the mesh",
    sk: "Požičané sieti mesh",
    de: "An das Mesh verliehen",
  },
  "widgets.builtin.mesh.lentBetween": {
    en: "Spare time is lent between {start} and {end}",
    sk: "Voľný čas sa požičiava od {start} do {end}",
    de: "Freie Zeit wird zwischen {start} und {end} verliehen",
  },
  "widgets.builtin.mesh.notLent": {
    en: "Spare time is not lent right now",
    sk: "Voľný čas sa momentálne nepožičiava",
    de: "Freie Zeit wird gerade nicht verliehen",
  },
  "widgets.builtin.mesh.storageShared": {
    en: "Storage shared",
    sk: "Úložisko zdieľané",
    de: "Speicher geteilt",
  },
  "widgets.builtin.mesh.notReported": {
    en: "How much work each side has done is not reported yet.",
    sk: "Koľko práce každá strana vykonala, sa zatiaľ neoznamuje.",
    de: "Wie viel Arbeit jede Seite geleistet hat, wird noch nicht gemeldet.",
  },

  // ── builtin.ts: apps ───────────────────────────────────────────────────
  "widgets.builtin.apps.title": { en: "Apps", sk: "Aplikácie", de: "Apps" },
  "widgets.builtin.apps.answering": { en: "Answering", sk: "Odpovedá", de: "Antwortet" },
  "widgets.builtin.apps.notAnswering": {
    en: "Not answering",
    sk: "Neodpovedá",
    de: "Antwortet nicht",
  },
  "widgets.builtin.apps.notChecked": { en: "Not checked", sk: "Neoverené", de: "Nicht geprüft" },
  "widgets.builtin.apps.onMesh": { en: "on the mesh", sk: "v sieti mesh", de: "im Mesh" },
  "widgets.builtin.apps.onBox": { en: "on this box", sk: "na tomto zariadení", de: "auf dieser Box" },
  "widgets.builtin.apps.empty": {
    en: "No apps yet. Finish setting the box up and they appear here.",
    sk: "Zatiaľ žiadne aplikácie. Dokončite nastavenie zariadenia a objavia sa tu.",
    de: "Noch keine Apps. Schließ die Einrichtung der Box ab, dann erscheinen sie hier.",
  },
  "widgets.builtin.apps.servedBy": {
    en: "Served by {host}.",
    sk: "Poskytuje {host}.",
    de: "Bereitgestellt von {host}.",
  },

  // ── builtin.ts: rebuilds ───────────────────────────────────────────────
  "widgets.builtin.rebuilds.title": { en: "Changes", sk: "Zmeny", de: "Änderungen" },
  "widgets.builtin.rebuilds.applying": {
    en: "Applying a change",
    sk: "Zmena sa používa",
    de: "Änderung wird angewendet",
  },
  "widgets.builtin.rebuilds.applied": {
    en: "Change applied",
    sk: "Zmena použitá",
    de: "Änderung angewendet",
  },
  "widgets.builtin.rebuilds.failed": {
    en: "Change failed",
    sk: "Zmena zlyhala",
    de: "Änderung fehlgeschlagen",
  },
  "widgets.builtin.rebuilds.idle": { en: "Idle", sk: "Nečinné", de: "Untätig" },
  "widgets.builtin.rebuilds.empty": {
    en: "Nothing has been changed from this browser.",
    sk: "Z tohto prehliadača sa zatiaľ nič nezmenilo.",
    de: "Von diesem Browser aus wurde nichts geändert.",
  },
  "widgets.builtin.rebuilds.busy": {
    en: "A change is being applied now. The box may be slow for a few minutes.",
    sk: "Práve sa používa zmena. Zariadenie môže byť niekoľko minút pomalšie.",
    de: "Gerade wird eine Änderung angewendet. Die Box ist vielleicht ein paar Minuten langsam.",
  },
  "widgets.builtin.rebuilds.onlyThis": {
    en: "Only changes this browser was open for.",
    sk: "Len zmeny, počas ktorých bol tento prehliadač otvorený.",
    de: "Nur Änderungen, während dieser Browser offen war.",
  },

  // ── spec.ts ────────────────────────────────────────────────────────────
  "widgets.spec.label.title": { en: "The title", sk: "Názov", de: "Der Titel" },
  "widgets.spec.label.foot": { en: "The note", sk: "Poznámka", de: "Die Notiz" },
  "widgets.spec.label.caption": { en: "The caption", sk: "Popis", de: "Die Beschriftung" },
  "widgets.spec.label.value": { en: "The figure", sk: "Hodnota", de: "Der Wert" },
  "widgets.spec.label.max": { en: "The full extent", sk: "Plný rozsah", de: "Der volle Umfang" },
  "widgets.spec.label.reserve": {
    en: "The held-back part",
    sk: "Odložená časť",
    de: "Der zurückgehaltene Teil",
  },
  "widgets.spec.label.fillLabel": {
    en: "The fill label",
    sk: "Popis výplne",
    de: "Die Beschriftung der Füllung",
  },
  "widgets.spec.label.reserveLabel": {
    en: "The held-back label",
    sk: "Popis odloženej časti",
    de: "Die Beschriftung des zurückgehaltenen Teils",
  },
  "widgets.spec.label.items": { en: "The rows", sk: "Riadky", de: "Die Zeilen" },
  "widgets.spec.label.itemLabel": { en: "The row label", sk: "Popis riadka", de: "Die Zeilenbeschriftung" },
  "widgets.spec.label.itemDetail": { en: "The row detail", sk: "Detail riadka", de: "Das Zeilendetail" },
  "widgets.spec.label.empty": {
    en: "The empty message",
    sk: "Text pre prázdny zoznam",
    de: "Der Text für eine leere Liste",
  },
  "widgets.spec.label.days": { en: "The days", sk: "Dni", de: "Die Tage" },
  "widgets.spec.label.lowLabel": { en: "The low label", sk: "Popis dolného konca", de: "Die untere Beschriftung" },
  "widgets.spec.label.highLabel": { en: "The high label", sk: "Popis horného konca", de: "Die obere Beschriftung" },
  "widgets.spec.mustBeText": {
    en: "{label} has to be text.",
    sk: "{label}: musí to byť text.",
    de: "{label} muss Text sein.",
  },
  "widgets.spec.tooLong": {
    en: "{label} is too long.",
    sk: "{label}: text je príliš dlhý.",
    de: "{label} ist zu lang.",
  },
  "widgets.spec.fieldProblem": {
    en: "{label}: {problem}",
    sk: "{label}: {problem}",
    de: "{label}: {problem}",
  },
  "widgets.spec.notValid": { en: "not valid", sk: "neplatné", de: "ungültig" },
  "widgets.spec.notRecord": {
    en: "A widget has to be a record of settings.",
    sk: "Widget musí byť záznam s nastaveniami.",
    de: "Ein Widget muss ein Datensatz mit Einstellungen sein.",
  },
  "widgets.spec.needTitle": {
    en: "Give the widget a title.",
    sk: "Dajte widgetu názov.",
    de: "Gib dem Widget einen Titel.",
  },
  "widgets.spec.pickKind": {
    en: "Pick one of: {kinds}.",
    sk: "Vyberte jedno z: {kinds}.",
    de: "Wähle eins davon: {kinds}.",
  },
  "widgets.spec.pickMetric": {
    en: "Pick a reading this box publishes.",
    sk: "Vyberte údaj, ktorý toto zariadenie zverejňuje.",
    de: "Wähle einen Messwert, den diese Box bereitstellt.",
  },
  "widgets.spec.badFormat": {
    en: "That is not a way of formatting a number.",
    sk: "Takto sa číslo formátovať nedá.",
    de: "So lässt sich keine Zahl formatieren.",
  },
  "widgets.spec.needValue": {
    en: "Say which figure to show.",
    sk: "Uveďte, ktorá hodnota sa má zobraziť.",
    de: "Gib an, welcher Wert angezeigt werden soll.",
  },
  "widgets.spec.needMax": {
    en: "Say what the bar is full at.",
    sk: "Uveďte, pri akej hodnote je pruh plný.",
    de: "Gib an, bei welchem Wert der Balken voll ist.",
  },
  "widgets.spec.needItems": {
    en: "Say which rows to list.",
    sk: "Uveďte, ktoré riadky sa majú vypísať.",
    de: "Gib an, welche Zeilen aufgelistet werden sollen.",
  },
  "widgets.spec.needDays": {
    en: "Say which days to draw.",
    sk: "Uveďte, ktoré dni sa majú vykresliť.",
    de: "Gib an, welche Tage gezeichnet werden sollen.",
  },
  "widgets.spec.didNotWork": {
    en: "That did not work out.",
    sk: "Toto nevyšlo.",
    de: "Das hat nicht geklappt.",
  },
  "widgets.spec.rowsNotList": {
    en: "The rows have to come out as a list.",
    sk: "Riadky musia vyjsť ako zoznam.",
    de: "Die Zeilen müssen als Liste herauskommen.",
  },
  "widgets.spec.daysNotList": {
    en: "The days have to come out as a list.",
    sk: "Dni musia vyjsť ako zoznam.",
    de: "Die Tage müssen als Liste herauskommen.",
  },
  "widgets.spec.noDates": {
    en: "None of those days had a date on them.",
    sk: "Žiadny z týchto dní nemal dátum.",
    de: "Keiner dieser Tage hatte ein Datum.",
  },
  "widgets.spec.blank.title": { en: "Room to grow", sk: "Miesto na rast", de: "Platz zum Wachsen" },
  "widgets.spec.blank.caption": {
    en: "held back, claimable without opening the box",
    sk: "odložené, dá sa sprístupniť bez otvárania zariadenia",
    de: "zurückgehalten, freischaltbar, ohne die Box zu öffnen",
  },
  "widgets.spec.example.daysWatched.label": {
    en: "Days watched",
    sk: "Sledované dni",
    de: "Beobachtete Tage",
  },
  "widgets.spec.example.daysWatched.note": {
    en: "How many days this browser has a reading for.",
    sk: "Pre koľko dní má tento prehliadač údaj.",
    de: "Für wie viele Tage dieser Browser einen Messwert hat.",
  },
  "widgets.spec.example.answeredShare.label": {
    en: "Answered, as a share",
    sk: "Podiel času, keď odpovedalo",
    de: "Anteil, in dem sie geantwortet hat",
  },
  "widgets.spec.example.answeredShare.note": {
    en: "Set the format to percent.",
    sk: "Nastavte formát na percentá.",
    de: "Stell das Format auf Prozent.",
  },
  "widgets.spec.example.longestQuiet.label": {
    en: "Longest quiet spell",
    sk: "Najdlhšie tiché obdobie",
    de: "Längste stille Phase",
  },
  "widgets.spec.example.longestQuiet.note": {
    en: "Set the format to duration.",
    sk: "Nastavte formát na dĺžku času.",
    de: "Stell das Format auf Zeitdauer.",
  },
  "widgets.spec.example.roomLeft.label": { en: "Room left", sk: "Zostávajúce miesto", de: "Freier Platz" },
  "widgets.spec.example.roomLeft.note": {
    en: "Set the format to bytes.",
    sk: "Nastavte formát na veľkosť.",
    de: "Stell das Format auf Größe.",
  },
  "widgets.spec.example.hoursLent.label": {
    en: "Hours lent a day",
    sk: "Požičané hodiny za deň",
    de: "Verliehene Stunden pro Tag",
  },
  "widgets.spec.example.hoursLent.note": {
    en: "Zero when this box is not lending time.",
    sk: "Nula, keď toto zariadenie čas nepožičiava.",
    de: "Null, wenn diese Box keine Zeit verleiht.",
  },
  "widgets.spec.example.appsAnswering.label": {
    en: "Apps answering",
    sk: "Odpovedajúce aplikácie",
    de: "Antwortende Apps",
  },
  "widgets.spec.example.appsAnswering.note": {
    en: "Row label: it.name. Row detail: it.path",
    sk: "Popis riadka: it.name. Detail riadka: it.path",
    de: "Zeilenbeschriftung: it.name. Zeilendetail: it.path",
  },
  "widgets.spec.example.typicalDay.label": { en: "Typical day", sk: "Typický deň", de: "Typischer Tag" },
  "widgets.spec.example.typicalDay.note": {
    en: "stat.* takes a list and gives back one number.",
    sk: "stat.* berie zoznam a vracia jedno číslo.",
    de: "stat.* nimmt eine Liste und gibt eine Zahl zurück.",
  },

  // ── expr.ts ────────────────────────────────────────────────────────────
  "widgets.expr.notNumber": {
    en: "{text} is not a number",
    sk: "{text} nie je číslo",
    de: "{text} ist keine Zahl",
  },
  "widgets.expr.unclosedQuote": {
    en: "This text is missing its closing quote",
    sk: "Tomuto textu chýba koncová úvodzovka",
    de: "Diesem Text fehlt das schließende Anführungszeichen",
  },
  "widgets.expr.unknownChar": {
    en: "{ch} does not mean anything here",
    sk: "{ch} tu nemá žiadny význam",
    de: "{ch} bedeutet hier nichts",
  },
  "widgets.expr.noEndAfter": {
    en: "This expression does not end after {token}",
    sk: "Tento výraz za {token} nekončí",
    de: "Dieser Ausdruck endet nicht nach {token}",
  },
  "widgets.expr.noEndHere": {
    en: "This expression does not end after here",
    sk: "Tento výraz tu nekončí",
    de: "Dieser Ausdruck endet hier nicht",
  },
  "widgets.expr.expected": {
    en: "Expected {token} here",
    sk: "Tu sa očakáva {token}",
    de: "Hier wird {token} erwartet",
  },
  "widgets.expr.tooManyNodes": {
    en: "This expression is too long to be worth reading",
    sk: "Tento výraz je pridlhý na to, aby sa oplatilo ho čítať",
    de: "Dieser Ausdruck ist zu lang, um ihn sinnvoll zu lesen",
  },
  "widgets.expr.tooDeep": {
    en: "This expression nests too deeply",
    sk: "Tento výraz je vnorený príliš hlboko",
    de: "Dieser Ausdruck ist zu tief verschachtelt",
  },
  "widgets.expr.nameAfterDot": {
    en: "Expected a name after the dot",
    sk: "Za bodkou sa očakáva názov",
    de: "Nach dem Punkt wird ein Name erwartet",
  },
  "widgets.expr.tooManyArgs": {
    en: "Too many values passed here",
    sk: "Tu je odovzdaných priveľa hodnôt",
    de: "Hier werden zu viele Werte übergeben",
  },
  "widgets.expr.listTooLong": {
    en: "This list is too long",
    sk: "Tento zoznam je príliš dlhý",
    de: "Diese Liste ist zu lang",
  },
  "widgets.expr.stopsEarly": {
    en: "This expression stops early",
    sk: "Tento výraz končí predčasne",
    de: "Dieser Ausdruck hört zu früh auf",
  },
  "widgets.expr.doesNotBelong": {
    en: "{token} does not belong here",
    sk: "{token} sem nepatrí",
    de: "{token} gehört nicht hierher",
  },
  "widgets.expr.notText": {
    en: "An expression has to be text",
    sk: "Výraz musí byť text",
    de: "Ein Ausdruck muss Text sein",
  },
  "widgets.expr.empty": {
    en: "This expression is empty",
    sk: "Tento výraz je prázdny",
    de: "Dieser Ausdruck ist leer",
  },
  "widgets.expr.tooLong": {
    en: "An expression may be at most {max} characters",
    sk: "Výraz môže mať najviac {max} znakov",
    de: "Ein Ausdruck darf höchstens {max} Zeichen lang sein",
  },
  "widgets.expr.notReadable": {
    en: "{name} is not readable",
    sk: "{name} sa nedá čítať",
    de: "{name} ist nicht lesbar",
  },
  "widgets.expr.tooMuchWork": {
    en: "This expression does too much work",
    sk: "Tento výraz robí priveľa práce",
    de: "Dieser Ausdruck macht zu viel Arbeit",
  },
  "widgets.expr.unknownName": {
    en: "There is nothing called {name} here",
    sk: "Nič s názvom {name} tu nie je",
    de: "Hier gibt es nichts namens {name}",
  },
  "widgets.expr.onlyCallable": {
    en: "Only stat.* and fmt.* can be called",
    sk: "Volať sa dajú len stat.* a fmt.*",
    de: "Nur stat.* und fmt.* können aufgerufen werden",
  },
  "widgets.expr.noSuchFunction": {
    en: "There is no such function as {name}",
    sk: "Funkcia {name} neexistuje",
    de: "Eine Funktion {name} gibt es nicht",
  },

  // ── custom-editor.tsx ──────────────────────────────────────────────────
  "widgets.editor.metric.uptime": {
    en: "Whether this box answered, day by day",
    sk: "Či toto zariadenie odpovedalo, deň po dni",
    de: "Ob diese Box geantwortet hat, Tag für Tag",
  },
  "widgets.editor.metric.storage": {
    en: "Room on this box",
    sk: "Miesto na tomto zariadení",
    de: "Platz auf dieser Box",
  },
  "widgets.editor.metric.mesh": {
    en: "Sharing with other boxes",
    sk: "Zdieľanie s inými zariadeniami",
    de: "Teilen mit anderen Boxen",
  },
  "widgets.editor.metric.apps": {
    en: "The apps this box serves",
    sk: "Aplikácie, ktoré toto zariadenie poskytuje",
    de: "Die Apps, die diese Box bereitstellt",
  },
  "widgets.editor.metric.rebuilds": {
    en: "Changes this box has applied",
    sk: "Zmeny, ktoré toto zariadenie použilo",
    de: "Änderungen, die diese Box übernommen hat",
  },
  "widgets.editor.metric.settings": {
    en: "How this box is set up",
    sk: "Ako je toto zariadenie nastavené",
    de: "Wie diese Box eingerichtet ist",
  },
  "widgets.editor.metric.status": {
    en: "What this box is doing right now",
    sk: "Čo toto zariadenie práve robí",
    de: "Was diese Box gerade tut",
  },
  "widgets.editor.kind.number": { en: "One big figure", sk: "Jedno veľké číslo", de: "Eine große Zahl" },
  "widgets.editor.kind.bar": {
    en: "A figure with a bar",
    sk: "Číslo s pruhom",
    de: "Eine Zahl mit Balken",
  },
  "widgets.editor.kind.list": { en: "A list of rows", sk: "Zoznam riadkov", de: "Eine Liste von Zeilen" },
  "widgets.editor.kind.heatmap": {
    en: "A square for every day",
    sk: "Štvorček za každý deň",
    de: "Ein Kästchen für jeden Tag",
  },
  "widgets.editor.format.number": { en: "Plain number", sk: "Obyčajné číslo", de: "Einfache Zahl" },
  "widgets.editor.format.bytes": {
    en: "Size, like 412 GB",
    sk: "Veľkosť, napríklad 412 GB",
    de: "Größe, etwa 412 GB",
  },
  "widgets.editor.format.percent": { en: "Percentage", sk: "Percentá", de: "Prozent" },
  "widgets.editor.format.duration": { en: "Length of time", sk: "Dĺžka času", de: "Zeitdauer" },
  "widgets.editor.format.plain": {
    en: "Exactly as given",
    sk: "Presne tak, ako je",
    de: "Genau wie angegeben",
  },
  "widgets.editor.vocab.m": {
    en: "the reading you picked above, e.g. m.totalBytes",
    sk: "údaj, ktorý ste vybrali vyššie, napr. m.totalBytes",
    de: "der Messwert, den du oben gewählt hast, z. B. m.totalBytes",
  },
  "widgets.editor.vocab.it": {
    en: "one row, in the two row fields",
    sk: "jeden riadok, v dvoch poliach pre riadky",
    de: "eine Zeile, in den beiden Zeilenfeldern",
  },
  "widgets.editor.doesNotAddUp": {
    en: "That does not add up yet.",
    sk: "Toto zatiaľ nesedí.",
    de: "Das geht noch nicht auf.",
  },
  "widgets.editor.pasteIncomplete": {
    en: "That is not a complete record. Check the brackets.",
    sk: "Toto nie je úplný záznam. Skontrolujte zátvorky.",
    de: "Das ist kein vollständiger Datensatz. Prüf die Klammern.",
  },
  "widgets.editor.pasteUnreadable": {
    en: "That widget could not be read.",
    sk: "Tento widget sa nepodarilo načítať.",
    de: "Dieses Widget konnte nicht gelesen werden.",
  },
  "widgets.editor.title": { en: "Build a widget", sk: "Zostaviť widget", de: "Widget bauen" },
  "widgets.editor.description": {
    en: "Pick a reading and a shape, then say what to show. Only readings this box publishes are available, and a widget can only read them.",
    sk: "Vyberte údaj a tvar a potom uveďte, čo sa má zobraziť. K dispozícii sú len údaje, ktoré toto zariadenie zverejňuje, a widget ich môže iba čítať.",
    de: "Wähle einen Messwert und eine Form und gib dann an, was angezeigt werden soll. Es gibt nur Messwerte, die diese Box bereitstellt, und ein Widget kann sie nur lesen.",
  },
  "widgets.editor.tab.build": { en: "Build", sk: "Zostaviť", de: "Bauen" },
  "widgets.editor.tab.paste": { en: "Paste", sk: "Vložiť", de: "Einfügen" },
  "widgets.editor.tab.help": {
    en: "What you can write",
    sk: "Čo môžete napísať",
    de: "Was du schreiben kannst",
  },
  "widgets.editor.field.title": { en: "Title", sk: "Názov", de: "Titel" },
  "widgets.editor.field.reading": { en: "Reading", sk: "Údaj", de: "Messwert" },
  "widgets.editor.field.shape": { en: "Shape", sk: "Tvar", de: "Form" },
  "widgets.editor.field.value": { en: "The figure", sk: "Hodnota", de: "Der Wert" },
  "widgets.editor.field.format": { en: "Shown as", sk: "Zobraziť ako", de: "Anzeigen als" },
  "widgets.editor.field.max": { en: "Full at", sk: "Plný pri", de: "Voll bei" },
  "widgets.editor.field.reserve": { en: "Held back", sk: "Odložené", de: "Zurückgehalten" },
  "widgets.editor.field.fillLabel": {
    en: "Label for the fill",
    sk: "Popis výplne",
    de: "Beschriftung der Füllung",
  },
  "widgets.editor.field.reserveLabel": {
    en: "Label for the held-back part",
    sk: "Popis odloženej časti",
    de: "Beschriftung des zurückgehaltenen Teils",
  },
  "widgets.editor.field.items": { en: "The rows", sk: "Riadky", de: "Die Zeilen" },
  "widgets.editor.field.itemLabel": { en: "Row label", sk: "Popis riadka", de: "Zeilenbeschriftung" },
  "widgets.editor.field.itemDetail": { en: "Row detail", sk: "Detail riadka", de: "Zeilendetail" },
  "widgets.editor.field.empty": {
    en: "When there is nothing",
    sk: "Keď nič nie je",
    de: "Wenn nichts da ist",
  },
  "widgets.editor.field.days": { en: "The days", sk: "Dni", de: "Die Tage" },
  "widgets.editor.field.lowLabel": { en: "Low end", sk: "Dolný koniec", de: "Unteres Ende" },
  "widgets.editor.field.highLabel": { en: "High end", sk: "Horný koniec", de: "Oberes Ende" },
  "widgets.editor.field.caption": { en: "Caption", sk: "Popis", de: "Beschriftung" },
  "widgets.editor.field.foot": { en: "Note", sk: "Poznámka", de: "Notiz" },
  "widgets.editor.hint.value": {
    en: "For example: {example}",
    sk: "Napríklad: {example}",
    de: "Zum Beispiel: {example}",
  },
  "widgets.editor.hint.max": {
    en: "What counts as a full bar, e.g. {example}",
    sk: "Čo sa považuje za plný pruh, napr. {example}",
    de: "Was als voller Balken zählt, z. B. {example}",
  },
  "widgets.editor.hint.reserve": {
    en: "Drawn hatched, right after the fill. Leave empty for none.",
    sk: "Vykreslí sa šrafovane hneď za výplňou. Ak nechcete žiadnu, nechajte prázdne.",
    de: "Wird schraffiert direkt nach der Füllung gezeichnet. Leer lassen für keinen.",
  },
  "widgets.editor.hint.items": {
    en: "A list, for example: {example}",
    sk: "Zoznam, napríklad: {example}",
    de: "Eine Liste, zum Beispiel: {example}",
  },
  "widgets.editor.hint.days": {
    en: "A list of days, for example: {example}",
    sk: "Zoznam dní, napríklad: {example}",
    de: "Eine Liste von Tagen, zum Beispiel: {example}",
  },
  "widgets.editor.hint.caption": {
    en: "A line under the figure.",
    sk: "Riadok pod hodnotou.",
    de: "Eine Zeile unter dem Wert.",
  },
  "widgets.editor.hint.foot": {
    en: "The small line at the bottom of the tile.",
    sk: "Malý riadok v spodnej časti dlaždice.",
    de: "Die kleine Zeile unten auf der Kachel.",
  },
  "widgets.editor.preview": { en: "Preview", sk: "Náhľad", de: "Vorschau" },
  "widgets.editor.paste.label": { en: "A widget, as text", sk: "Widget ako text", de: "Ein Widget als Text" },
  "widgets.editor.paste.hint": {
    en: "Widgets are plain records. Copy one out of another browser, or write one here, then load it into the form to check it.",
    sk: "Widgety sú obyčajné záznamy. Skopírujte ho z iného prehliadača alebo ho napíšte sem a potom ho načítajte do formulára na kontrolu.",
    de: "Widgets sind einfache Datensätze. Kopier eins aus einem anderen Browser oder schreib eins hier und lade es dann zur Prüfung ins Formular.",
  },
  "widgets.editor.paste.load": {
    en: "Load into the form",
    sk: "Načítať do formulára",
    de: "Ins Formular laden",
  },
  "widgets.editor.paste.current": {
    en: "What the form holds now",
    sk: "Čo formulár práve obsahuje",
    de: "Was gerade im Formular steht",
  },
  "widgets.editor.help.intro": {
    en: "A widget here is not a program. It is a reading, a shape, and a few short expressions. That limit is deliberate. This page is served with a rule that forbids the browser from running code it was handed at the last moment, and a box you cannot log in to is not the place to discover that the rule applies.",
    sk: "Widget tu nie je program. Je to údaj, tvar a niekoľko krátkych výrazov. To obmedzenie je zámerné. Táto stránka sa poskytuje s pravidlom, ktoré prehliadaču zakazuje spúšťať kód odovzdaný na poslednú chvíľu, a zariadenie, do ktorého sa nedá prihlásiť, nie je miesto, kde by ste mali zistiť, že to pravidlo platí.",
    de: "Ein Widget ist hier kein Programm. Es ist ein Messwert, eine Form und ein paar kurze Ausdrücke. Diese Grenze ist Absicht. Diese Seite wird mit einer Regel ausgeliefert, die dem Browser verbietet, Code auszuführen, den er im letzten Moment bekommen hat, und eine Box, auf der du dich nicht anmelden kannst, ist nicht der Ort, um herauszufinden, dass die Regel gilt.",
  },
  "widgets.editor.help.operators": {
    en: "Between them you can use {arith}, comparisons, {and} and {or}, {cond}, and {coalesce} for a fallback when a figure has not been measured. Dividing by nothing gives nothing rather than an error.",
    sk: "Medzi nimi môžete použiť {arith}, porovnania, {and} a {or}, {cond} a {coalesce} ako náhradu, keď hodnota ešte nebola zmeraná. Delenie nulou dá nulu, nie chybu.",
    de: "Dazwischen kannst du {arith}, Vergleiche, {and} und {or}, {cond} und {coalesce} als Ersatz verwenden, wenn ein Wert noch nicht gemessen wurde. Teilen durch nichts ergibt nichts statt eines Fehlers.",
  },
  "widgets.editor.help.more": {
    en: "A widget that wants more than that, with its own requests, its own state or a loop, belongs in the box's own admin page and arrives with the next system update, the same way these five did.",
    sk: "Widget, ktorý potrebuje viac, s vlastnými požiadavkami, vlastným stavom alebo cyklom, patrí do vlastnej správcovskej stránky zariadenia a príde s ďalšou aktualizáciou systému, rovnako ako týchto päť.",
    de: "Ein Widget, das mehr will, mit eigenen Anfragen, eigenem Zustand oder einer Schleife, gehört in die Admin-Seite der Box selbst und kommt mit dem nächsten Systemupdate, genau wie diese fünf.",
  },
  "widgets.editor.help.kept": {
    en: "Widgets are kept in this browser only, and refresh about once a minute.",
    sk: "Widgety sa uchovávajú len v tomto prehliadači a obnovujú sa približne raz za minútu.",
    de: "Widgets werden nur in diesem Browser gespeichert und etwa einmal pro Minute aktualisiert.",
  },
  "widgets.editor.cancel": { en: "Cancel", sk: "Zrušiť", de: "Abbrechen" },
  "widgets.editor.addToBoard": {
    en: "Add to the board",
    sk: "Pridať na nástenku",
    de: "Zur Pinnwand hinzufügen",
  },
  "widgets.editor.didNotWork": {
    en: "That did not work out.",
    sk: "Toto nevyšlo.",
    de: "Das hat nicht geklappt.",
  },
  "widgets.editor.previewEmpty": {
    en: "Fill the form in and it appears here.",
    sk: "Vyplňte formulár a zobrazí sa tu.",
    de: "Füll das Formular aus, dann erscheint es hier.",
  },
  "widgets.editor.working": { en: "Working…", sk: "Pracuje sa…", de: "Wird ausgeführt…" },
  "widgets.editor.tryOne": { en: "Try one", sk: "Vyskúšajte", de: "Probier eins" },
});
