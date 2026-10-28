import { defineMessages } from "./define";

/* LosOS Wiki (src/wikis/ in the admin shell, src/wiki/ for a space's
 * published site): the box's directory of wikis, and the spaces whose pages
 * are written here. A space is a LosOS Git repository's wiki. */
export default defineMessages({
  "wiki.title": { en: "Wiki", sk: "Wiki", de: "Wiki" },
  "wiki.summary": {
    en: "Every wiki on this box you may read: the spaces kept in LosOS Git, the knowledge bases published from Notes, and the handbook.",
    sk: "Všetky wiki na tomto boxe, ktoré smiete čítať: priestory uložené v LosOS Git, znalostné bázy zverejnené z Poznámok a príručka.",
    de: "Jedes Wiki auf dieser Box, das du lesen darfst: die Bereiche in LosOS Git, die aus Notizen veröffentlichten Wissensdatenbanken und das Handbuch.",
  },
  "wiki.all": { en: "All wikis", sk: "Všetky wiki", de: "Alle Wikis" },
  "wiki.search": { en: "Search every wiki", sk: "Hľadať vo všetkých wiki", de: "Alle Wikis durchsuchen" },
  "wiki.searchLabel": {
    en: "Search the pages of every wiki on this box",
    sk: "Hľadať na stránkach všetkých wiki na tomto boxe",
    de: "Die Seiten aller Wikis auf dieser Box durchsuchen",
  },
  "wiki.results": { en: "Pages with “{query}”", sk: "Stránky s „{query}“", de: "Seiten mit „{query}“" },
  "wiki.back": { en: "Back", sk: "Späť", de: "Zurück" },
  "wiki.noResults": { en: "No page says that.", sk: "Žiadna stránka to neobsahuje.", de: "Keine Seite enthält das." },
  "wiki.searchHandbook": {
    en: "Search the handbook for “{query}”",
    sk: "Hľadať „{query}“ v príručke",
    de: "„{query}“ im Handbuch suchen",
  },
  "wiki.gitDown": {
    en: "LosOS Git does not answer, so its spaces are missing for now.",
    sk: "LosOS Git neodpovedá, preto tu jeho priestory zatiaľ chýbajú.",
    de: "LosOS Git antwortet nicht, darum fehlen seine Bereiche vorerst.",
  },
  "wiki.noSpaces": {
    en: "No space yet. Make one, or open the wiki of a repository you have.",
    sk: "Zatiaľ žiadny priestor. Vytvorte ho alebo otvorte wiki repozitára, ktorý máte.",
    de: "Noch kein Bereich. Leg einen an oder öffne das Wiki eines deiner Repositorys.",
  },
  "wiki.noMatch": { en: "No space by that name.", sk: "Žiadny priestor s takým názvom.", de: "Kein Bereich mit diesem Namen." },
  "wiki.pages": {
    en: { one: "{count} page", other: "{count} pages" },
    sk: { one: "{count} stránka", few: "{count} stránky", many: "{count} stránky", other: "{count} stránok" },
    de: { one: "{count} Seite", other: "{count} Seiten" },
  },
  "wiki.notes": {
    en: { one: "{count} note", other: "{count} notes" },
    sk: { one: "{count} poznámka", few: "{count} poznámky", many: "{count} poznámky", other: "{count} poznámok" },
    de: { one: "{count} Notiz", other: "{count} Notizen" },
  },
  "wiki.pagesOf": { en: "Pages of {space}", sk: "Stránky priestoru {space}", de: "Seiten von {space}" },
  "wiki.cancel": { en: "Cancel", sk: "Zrušiť", de: "Abbrechen" },

  // ── the directory ──
  "wiki.group.spaces": { en: "Spaces", sk: "Priestory", de: "Bereiche" },
  "wiki.group.kb": { en: "Knowledge bases", sk: "Znalostné bázy", de: "Wissensdatenbanken" },
  "wiki.group.builtIn": { en: "Built in", sk: "Vstavané", de: "Eingebaut" },
  "wiki.pick.title": { en: "Pick a wiki", sk: "Vyberte wiki", de: "Wähle ein Wiki" },
  "wiki.pick.text": {
    en: "Its pages open here. Search looks through every wiki at once.",
    sk: "Jej stránky sa otvoria tu. Hľadanie prejde všetky wiki naraz.",
    de: "Seine Seiten öffnen sich hier. Die Suche durchsucht alle Wikis auf einmal.",
  },
  "wiki.audience.anyone": { en: "Anyone with the address", sk: "Ktokoľvek s adresou", de: "Alle mit der Adresse" },
  "wiki.audience.box": { en: "People on this box", sk: "Ľudia na tomto boxe", de: "Personen auf dieser Box" },
  "wiki.audience.members": { en: "Members only", sk: "Len členovia", de: "Nur Mitglieder" },
  "wiki.audience.lan": { en: "This network", sk: "Táto sieť", de: "Dieses Netzwerk" },

  // ── the handbook ──
  "wiki.handbook.title": { en: "LosOS handbook", sk: "Príručka LosOS", de: "LosOS-Handbuch" },
  "wiki.handbook.kind": { en: "Handbook", sk: "Príručka", de: "Handbuch" },
  "wiki.handbook.text": {
    en: "How LosOS works and what to do when something does not. Every box serves its own copy, so it opens without the internet.",
    sk: "Ako LosOS funguje a čo robiť, keď niečo nejde. Každý box má vlastnú kópiu, takže sa otvorí aj bez internetu.",
    de: "Wie LosOS funktioniert und was zu tun ist, wenn etwas nicht geht. Jede Box liefert ihre eigene Kopie, sie öffnet sich also auch ohne Internet.",
  },
  "wiki.handbook.open": { en: "Open the handbook", sk: "Otvoriť príručku", de: "Handbuch öffnen" },
  "wiki.handbook.start": { en: "Getting started", sk: "Začíname", de: "Erste Schritte" },
  "wiki.handbook.types": { en: "Kinds of box", sk: "Druhy boxov", de: "Arten von Boxen" },
  "wiki.handbook.manual": { en: "Manual", sk: "Návod", de: "Anleitung" },
  "wiki.handbook.troubleshooting": { en: "Troubleshooting", sk: "Riešenie problémov", de: "Fehlerbehebung" },
  "wiki.handbook.reference": { en: "Reference", sk: "Referencia", de: "Referenz" },
  "wiki.handbook.inDepth": { en: "In depth", sk: "Do hĺbky", de: "Im Detail" },

  // ── a knowledge base ──
  "wiki.kb.kind": { en: "Knowledge base", sk: "Znalostná báza", de: "Wissensdatenbank" },
  "wiki.kb.text": {
    en: "A folder of notes published from LosOS Cloud's Notes. Its owner writes it there, and the site follows every change.",
    sk: "Priečinok poznámok zverejnený z Poznámok v LosOS Cloud. Vlastník ho píše tam a stránka sleduje každú zmenu.",
    de: "Ein Ordner mit Notizen, veröffentlicht aus den Notizen von LosOS Cloud. Wer ihn besitzt, schreibt dort, und die Seite folgt jeder Änderung.",
  },
  "wiki.kb.open": { en: "Open the site", sk: "Otvoriť stránku", de: "Seite öffnen" },

  // ── a new space ──
  "wiki.space.new": { en: "New space", sk: "Nový priestor", de: "Neuer Bereich" },
  "wiki.space.newTitle": { en: "New space", sk: "Nový priestor", de: "Neuer Bereich" },
  "wiki.space.newBody": {
    en: "A space is a repository in LosOS Git with its wiki. Whoever may read the repository reads the space, and whoever may push to it writes.",
    sk: "Priestor je repozitár v LosOS Git s jeho wiki. Kto smie čítať repozitár, číta aj priestor, a kto doň smie posielať zmeny, píše.",
    de: "Ein Bereich ist ein Repository in LosOS Git mit seinem Wiki. Wer das Repository lesen darf, liest den Bereich, und wer hineinpushen darf, schreibt.",
  },
  "wiki.space.name": { en: "Name", sk: "Názov", de: "Name" },
  "wiki.space.nameHint": {
    en: "Letters, numbers, dots, dashes and underscores make the repository's name.",
    sk: "Názov repozitára tvoria písmená, číslice, bodky, pomlčky a podčiarkovníky.",
    de: "Buchstaben, Ziffern, Punkte, Bindestriche und Unterstriche bilden den Namen des Repositorys.",
  },
  "wiki.space.repo": { en: "Repository: {repo}", sk: "Repozitár: {repo}", de: "Repository: {repo}" },
  "wiki.space.description": { en: "What it is about", sk: "O čom je", de: "Worum es geht" },
  "wiki.space.make": { en: "Make the space", sk: "Vytvoriť priestor", de: "Bereich anlegen" },
  "wiki.space.made": { en: "Space made", sk: "Priestor je vytvorený", de: "Bereich angelegt" },
  "wiki.space.taken": {
    en: "You already have a repository by that name.",
    sk: "Repozitár s týmto názvom už máte.",
    de: "Du hast schon ein Repository mit diesem Namen.",
  },
  "wiki.space.homeText": {
    en: "This is the first page of the space. Edit it to say what the space is for.",
    sk: "Toto je prvá stránka priestoru. Upravte ju a napíšte, na čo priestor slúži.",
    de: "Das ist die erste Seite des Bereichs. Bearbeite sie und schreib, wofür der Bereich da ist.",
  },
  "wiki.space.site": { en: "Published site", sk: "Zverejnená stránka", de: "Veröffentlichte Seite" },

  // ── an empty space ──
  "wiki.empty.title": { en: "No pages yet", sk: "Zatiaľ žiadne stránky", de: "Noch keine Seiten" },
  "wiki.empty.writer": {
    en: "Write the first page, or push one with git to the address below.",
    sk: "Napíšte prvú stránku alebo ju pošlite cez git na adresu nižšie.",
    de: "Schreib die erste Seite oder pushe eine mit git an die Adresse unten.",
  },
  "wiki.empty.reader": {
    en: "Nobody has written a page here yet.",
    sk: "Sem zatiaľ nikto nenapísal žiadnu stránku.",
    de: "Hier hat noch niemand eine Seite geschrieben.",
  },
  "wiki.empty.first": { en: "Write the first page", sk: "Napísať prvú stránku", de: "Erste Seite schreiben" },

  // ── a page ──
  "wiki.page.new": { en: "New page", sk: "Nová stránka", de: "Neue Seite" },
  "wiki.page.child": { en: "New page below this one", sk: "Nová stránka pod touto", de: "Neue Seite unter dieser" },
  "wiki.page.edit": { en: "Edit", sk: "Upraviť", de: "Bearbeiten" },
  "wiki.page.delete": { en: "Delete page", sk: "Odstrániť stránku", de: "Seite löschen" },
  "wiki.page.missing": { en: "No such page", sk: "Taká stránka neexistuje", de: "Diese Seite gibt es nicht" },
  "wiki.page.missingText": {
    en: "It may have been renamed or deleted. The space's other pages are on the left.",
    sk: "Možno ju niekto premenoval alebo odstránil. Ostatné stránky priestoru sú vľavo.",
    de: "Vielleicht wurde sie umbenannt oder gelöscht. Die anderen Seiten des Bereichs stehen links.",
  },
  "wiki.page.updated": { en: "Changed by {who} {when}", sk: "Zmenil(a) {who} {when}", de: "Geändert von {who} {when}" },
  "wiki.page.history": { en: "Changes", sk: "Zmeny", de: "Änderungen" },
  "wiki.page.change": { en: "{who}, {when}", sk: "{who}, {when}", de: "{who}, {when}" },

  // ── writing ──
  "wiki.edit.newTitle": { en: "New page", sk: "Nová stránka", de: "Neue Seite" },
  "wiki.edit.title": { en: "Editing {title}", sk: "Úprava stránky {title}", de: "{title} bearbeiten" },
  "wiki.edit.pageTitle": { en: "Title", sk: "Názov", de: "Titel" },
  "wiki.edit.titleHint": {
    en: "The title names the page and its address.",
    sk: "Názov pomenuje stránku aj jej adresu.",
    de: "Der Titel benennt die Seite und ihre Adresse.",
  },
  "wiki.edit.file": { en: "Saved as {file}", sk: "Uloží sa ako {file}", de: "Wird als {file} gespeichert" },
  "wiki.edit.write": { en: "Write", sk: "Písať", de: "Schreiben" },
  "wiki.edit.preview": { en: "Preview", sk: "Náhľad", de: "Vorschau" },
  "wiki.edit.text": { en: "Text in Markdown", sk: "Text v Markdowne", de: "Text in Markdown" },
  "wiki.edit.placeholder": {
    en: "Write in Markdown. [[Another page]] links to a page of this space.",
    sk: "Píšte v Markdowne. [[Iná stránka]] odkazuje na stránku tohto priestoru.",
    de: "Schreib in Markdown. [[Andere Seite]] verlinkt eine Seite dieses Bereichs.",
  },
  "wiki.edit.nothing": { en: "Nothing written yet.", sk: "Zatiaľ nič nenapísané.", de: "Noch nichts geschrieben." },
  "wiki.edit.message": { en: "What changed", sk: "Čo sa zmenilo", de: "Was sich geändert hat" },
  "wiki.edit.messageHint": {
    en: "Optional, kept with the change in the space's history",
    sk: "Nepovinné, uloží sa so zmenou do histórie priestoru",
    de: "Optional, wird mit der Änderung im Verlauf des Bereichs gespeichert",
  },
  "wiki.edit.save": { en: "Save", sk: "Uložiť", de: "Speichern" },
  "wiki.edit.saved": { en: "Page saved", sk: "Stránka je uložená", de: "Seite gespeichert" },
  "wiki.edit.conflict": {
    en: "Someone saved this page while you wrote. Copy your text, reopen the page and add it again.",
    sk: "Niekto túto stránku uložil, kým ste písali. Skopírujte si text, otvorte stránku znova a doplňte ho.",
    de: "Jemand hat diese Seite gespeichert, während du geschrieben hast. Kopier deinen Text, öffne die Seite neu und füg ihn wieder ein.",
  },
  "wiki.edit.forbidden": {
    en: "You may read this space but not write in it.",
    sk: "Tento priestor smiete čítať, ale nie doň písať.",
    de: "Du darfst diesen Bereich lesen, aber nicht darin schreiben.",
  },
  "wiki.edit.failed": { en: "The page was not saved: {detail}", sk: "Stránka sa neuložila: {detail}", de: "Die Seite wurde nicht gespeichert: {detail}" },

  // ── deleting ──
  "wiki.delete.title": { en: "Delete “{title}”?", sk: "Odstrániť „{title}“?", de: "„{title}“ löschen?" },
  "wiki.delete.body": {
    en: "The page leaves the space. Its text stays in the space's history, so it can be brought back with git.",
    sk: "Stránka z priestoru zmizne. Jej text zostane v histórii priestoru, takže sa dá vrátiť cez git.",
    de: "Die Seite verschwindet aus dem Bereich. Ihr Text bleibt im Verlauf des Bereichs und lässt sich mit git zurückholen.",
  },
  "wiki.delete.confirm": { en: "Delete", sk: "Odstrániť", de: "Löschen" },
  "wiki.delete.done": { en: "Page deleted", sk: "Stránka je odstránená", de: "Seite gelöscht" },

  // ── a space's published site (src/wiki/site.tsx) ──
  "wiki.site.title": { en: "LosOS Wiki", sk: "LosOS Wiki", de: "LosOS Wiki" },
  "wiki.site.summary": {
    en: "The spaces on this box you may read, each a site of its own.",
    sk: "Priestory na tomto boxe, ktoré smiete čítať, každý ako samostatná stránka.",
    de: "Die Bereiche auf dieser Box, die du lesen darfst, jeder als eigene Seite.",
  },
  "wiki.site.none": {
    en: "No space here has pages you may read.",
    sk: "Žiadny priestor tu nemá stránky, ktoré smiete čítať.",
    de: "Kein Bereich hier hat Seiten, die du lesen darfst.",
  },
  "wiki.site.loading": { en: "Loading…", sk: "Načítava sa…", de: "Wird geladen…" },
  "wiki.site.signIn": { en: "Sign in", sk: "Prihlásiť sa", de: "Anmelden" },
  "wiki.site.search": { en: "Search this space", sk: "Hľadať v tomto priestore", de: "Diesen Bereich durchsuchen" },
  "wiki.site.pages": { en: "Pages", sk: "Stránky", de: "Seiten" },
  "wiki.site.edit": { en: "Edit", sk: "Upraviť", de: "Bearbeiten" },
  "wiki.site.emptyWriter": {
    en: "Write the first page in LosOS Wiki, or push one with git.",
    sk: "Napíšte prvú stránku v LosOS Wiki alebo ju pošlite cez git.",
    de: "Schreib die erste Seite in LosOS Wiki oder pushe eine mit git.",
  },
  "wiki.site.notFound.title": { en: "No such space", sk: "Taký priestor neexistuje", de: "Diesen Bereich gibt es nicht" },
  "wiki.site.notFound.text": {
    en: "It is not here, or it is not open to you.",
    sk: "Nie je tu, alebo pre vás nie je prístupný.",
    de: "Er ist nicht hier, oder er ist für dich nicht offen.",
  },
  "wiki.site.notFound.signIn": {
    en: "It is not here, or it is open only to people signed in to this box.",
    sk: "Nie je tu, alebo je prístupný len ľuďom prihláseným na tomto boxe.",
    de: "Er ist nicht hier, oder er ist nur für Personen offen, die auf dieser Box angemeldet sind.",
  },
  "wiki.site.offline.title": { en: "The space did not load", sk: "Priestor sa nenačítal", de: "Der Bereich wurde nicht geladen" },
  "wiki.site.offline.text": {
    en: "LosOS Git does not answer right now. Try again in a minute.",
    sk: "LosOS Git práve neodpovedá. Skúste to znova o minútu.",
    de: "LosOS Git antwortet gerade nicht. Versuch es in einer Minute noch einmal.",
  },
});
