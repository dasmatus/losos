import { defineMessages } from "./define";

/* Notes in LosOS Cloud (src/cloud/notes/, src/notes/) and the knowledge
 * bases published from them (src/kb/). */
export default defineMessages({
  // ── the files list ──
  "notes.title": { en: "Notes", sk: "Poznámky", de: "Notizen" },
  "notes.open": { en: "Open in Notes", sk: "Otvoriť v Poznámkach", de: "In Notizen öffnen" },
  "notes.openVault": { en: "Open as notes", sk: "Otvoriť ako poznámky", de: "Als Notizen öffnen" },

  // ── the vault ──
  "notes.loading": { en: "Loading notes", sk: "Načítavajú sa poznámky", de: "Notizen werden geladen" },
  "notes.new": { en: "New note", sk: "Nová poznámka", de: "Neue Notiz" },
  "notes.untitled": { en: "Untitled", sk: "Bez názvu", de: "Ohne Titel" },
  "notes.tree.label": { en: "Notes in this folder", sk: "Poznámky v tomto priečinku", de: "Notizen in diesem Ordner" },
  "notes.tags": { en: "Tags", sk: "Štítky", de: "Tags" },
  "notes.search.label": { en: "Search notes", sk: "Hľadať v poznámkach", de: "Notizen durchsuchen" },
  "notes.search.placeholder": { en: "Search names and text", sk: "Hľadať v názvoch a texte", de: "Namen und Text durchsuchen" },
  "notes.search.results": { en: "Search results", sk: "Výsledky hľadania", de: "Suchergebnisse" },
  "notes.search.none": { en: "Nothing found.", sk: "Nič sa nenašlo.", de: "Nichts gefunden." },
  "notes.empty.title": { en: "No notes yet", sk: "Zatiaľ žiadne poznámky", de: "Noch keine Notizen" },
  "notes.empty.text": {
    en: "Notes are the .note and .md files in this folder. Write one here, or sync an Obsidian vault into it.",
    sk: "Poznámky sú súbory .note a .md v tomto priečinku. Napíšte jednu tu alebo sem synchronizujte trezor z Obsidianu.",
    de: "Notizen sind die .note- und .md-Dateien in diesem Ordner. Schreib hier eine oder synchronisiere einen Obsidian-Vault hinein.",
  },
  "notes.gone.title": { en: "This note is gone", sk: "Táto poznámka už neexistuje", de: "Diese Notiz gibt es nicht mehr" },
  "notes.error.vault": { en: "Notes could not open this folder", sk: "Poznámky nemohli otvoriť tento priečinok", de: "Notizen konnten diesen Ordner nicht öffnen" },
  "notes.error.create": { en: "The note was not made", sk: "Poznámka sa nevytvorila", de: "Die Notiz wurde nicht angelegt" },
  "notes.error.save": { en: "The change was not saved", sk: "Zmena sa neuložila", de: "Die Änderung wurde nicht gespeichert" },
  "notes.tag.count": {
    en: { one: "{count} note", other: "{count} notes" },
    sk: { one: "{count} poznámka", few: "{count} poznámky", many: "{count} poznámky", other: "{count} poznámok" },
    de: { one: "{count} Notiz", other: "{count} Notizen" },
  },

  // ── a note ──
  "notes.view.label": { en: "Show the note as", sk: "Zobraziť poznámku ako", de: "Notiz anzeigen als" },
  "notes.view.read": { en: "Read", sk: "Čítať", de: "Lesen" },
  "notes.view.edit": { en: "Edit", sk: "Upraviť", de: "Bearbeiten" },
  "notes.view.map": { en: "Mind map", sk: "Myšlienková mapa", de: "Mindmap" },
  "notes.properties": { en: "Properties", sk: "Vlastnosti", de: "Eigenschaften" },
  "notes.embed.loading": { en: "Loading…", sk: "Načítava sa…", de: "Wird geladen…" },
  "notes.embed.missing": { en: "Not found", sk: "Nenašlo sa", de: "Nicht gefunden" },
  "notes.problems": { en: "What this note asks for and does not get", sk: "Čo si poznámka pýta a nedostane", de: "Was diese Notiz verlangt und nicht bekommt" },
  "notes.problems.line": { en: "Line {line}", sk: "Riadok {line}", de: "Zeile {line}" },
  "notes.convert.action": { en: "Convert to .note", sk: "Previesť na .note", de: "In .note umwandeln" },
  "notes.convert.what": {
    en: "Makes this note a .note, which can hold components such as callouts and tabs. It reads the same, and [[links]] to it keep working.",
    sk: "Z tejto poznámky urobí .note, ktorá môže obsahovať komponenty, napríklad upozornenia a karty. Číta sa rovnako a [[odkazy]] na ňu fungujú ďalej.",
    de: "Macht diese Notiz zu einer .note, die Komponenten wie Hinweise und Tabs enthalten kann. Sie liest sich gleich, und [[Links]] darauf funktionieren weiter.",
  },
  "notes.convert.done": { en: "{name} is a .note now", sk: "{name} je teraz .note", de: "{name} ist jetzt eine .note" },
  "notes.convert.refused": { en: "{name} stays .md", sk: "{name} zostáva .md", de: "{name} bleibt .md" },
  "notes.convert.taken": {
    en: "A .note of that name is already there.",
    sk: ".note s týmto názvom už existuje.",
    de: "Eine .note mit diesem Namen gibt es schon.",
  },
  "notes.canvas.unreadable": {
    en: "This canvas could not be read.",
    sk: "Toto plátno sa nedá prečítať.",
    de: "Diese Leinwand konnte nicht gelesen werden.",
  },
  "notes.map.label": { en: "Mind map of {name}", sk: "Myšlienková mapa: {name}", de: "Mindmap von {name}" },
  "notes.missing.create": { en: "Make the note {name}", sk: "Vytvoriť poznámku {name}", de: "Notiz {name} anlegen" },
  "notes.edit.hint": {
    en: "Type [[ to link a note. Everyone with the note open types together.",
    sk: "Napíšte [[ a prepojte poznámku. Všetci, kto má poznámku otvorenú, píšu spolu.",
    de: "Tippe [[, um eine Notiz zu verlinken. Alle, die die Notiz offen haben, schreiben gemeinsam.",
  },
  "notes.edit.placeholder": { en: "Start writing", sk: "Začnite písať", de: "Fang an zu schreiben" },
  "notes.edit.preview": { en: "Preview", sk: "Náhľad", de: "Vorschau" },

  // ── beside a note ──
  "notes.side.label": { en: "About this note", sk: "O tejto poznámke", de: "Über diese Notiz" },
  "notes.side.graph": { en: "Local graph", sk: "Lokálny graf", de: "Lokaler Graph" },
  "notes.side.depth": { en: "Links out", sk: "Počet odkazov", de: "Links entfernt" },
  "notes.side.depthN": {
    en: { one: "{count} link", other: "{count} links" },
    sk: { one: "{count} odkaz", few: "{count} odkazy", many: "{count} odkazu", other: "{count} odkazov" },
    de: { one: "{count} Link", other: "{count} Links" },
  },
  "notes.side.backlinks": { en: "Linked here", sk: "Odkazujú sem", de: "Verlinkt hierher" },
  "notes.side.noBacklinks": { en: "No note links here yet.", sk: "Sem zatiaľ neodkazuje žiadna poznámka.", de: "Noch keine Notiz verlinkt hierher." },
  "notes.side.outgoing": { en: "Links to", sk: "Odkazuje na", de: "Verlinkt auf" },
  "notes.side.missing": { en: "Not written yet", sk: "Ešte nenapísané", de: "Noch nicht geschrieben" },
  "notes.side.outline": { en: "Outline", sk: "Osnova", de: "Gliederung" },

  // ── the graph ──
  "notes.graph.open": { en: "Graph of all notes", sk: "Graf všetkých poznámok", de: "Graph aller Notizen" },
  "notes.graph.title": { en: "Graph", sk: "Graf", de: "Graph" },
  "notes.graph.local": { en: "Notes around {name}", sk: "Poznámky okolo: {name}", de: "Notizen rund um {name}" },
  "notes.graph.count": {
    en: "{notes} dots, {links} links",
    sk: "Bodov: {notes}, odkazov: {links}",
    de: "{notes} Punkte, {links} Links",
  },
  "notes.graph.filter": { en: "Only names with…", sk: "Len názvy s…", de: "Nur Namen mit…" },
  "notes.graph.tags": { en: "Tags", sk: "Štítky", de: "Tags" },
  "notes.graph.attachments": { en: "Attachments", sk: "Prílohy", de: "Anhänge" },
  "notes.graph.missing": { en: "Not written yet", sk: "Ešte nenapísané", de: "Noch nicht geschrieben" },
  "notes.graph.orphans": { en: "Unlinked notes", sk: "Neprepojené poznámky", de: "Unverlinkte Notizen" },

  // ── publishing a folder ──
  "notes.publish.action": { en: "Publish as knowledge base", sk: "Zverejniť ako znalostnú bázu", de: "Als Wissensbasis veröffentlichen" },
  "notes.publish.manage": { en: "Knowledge base", sk: "Znalostná báza", de: "Wissensbasis" },
  "notes.publish.title": { en: "Publish {name}", sk: "Zverejniť {name}", de: "{name} veröffentlichen" },
  "notes.publish.what": {
    en: "The notes in this folder become a site to read, with links, search and the graph. It follows the folder: what you write here shows there.",
    sk: "Poznámky v tomto priečinku sa stanú stránkou na čítanie s odkazmi, hľadaním a grafom. Stránka sleduje priečinok: čo tu napíšete, ukáže sa aj tam.",
    de: "Die Notizen in diesem Ordner werden zu einer Seite zum Lesen, mit Links, Suche und Graph. Sie folgt dem Ordner: Was du hier schreibst, erscheint dort.",
  },
  "notes.publish.live": {
    en: "Published. The site follows the folder.",
    sk: "Zverejnené. Stránka sleduje priečinok.",
    de: "Veröffentlicht. Die Seite folgt dem Ordner.",
  },
  "notes.publish.mesh": {
    en: "Knowledge bases need this box to share and to be in a mesh. Turn both on in the box's settings first.",
    sk: "Znalostné bázy vyžadujú, aby toto zariadenie zdieľalo a bolo v sieti. Najprv obe zapnite v nastaveniach zariadenia.",
    de: "Wissensbasen brauchen eine Box, die teilt und in einem Mesh ist. Schalte zuerst beides in den Einstellungen der Box ein.",
  },
  "notes.publish.name": { en: "Title", sk: "Názov", de: "Titel" },
  "notes.publish.who": { en: "Who can read it", sk: "Kto ju môže čítať", de: "Wer sie lesen kann" },
  "notes.publish.anyone": { en: "Anyone with the link", sk: "Ktokoľvek s odkazom", de: "Alle mit dem Link" },
  "notes.publish.box": { en: "People on this box", sk: "Ľudia na tomto zariadení", de: "Leute auf dieser Box" },
  "notes.publish.anyoneHint": {
    en: "No sign-in. Anyone who can reach the box's address can read it.",
    sk: "Bez prihlásenia. Prečíta ju každý, kto sa dostane na adresu zariadenia.",
    de: "Ohne Anmeldung. Alle, die die Adresse der Box erreichen, können sie lesen.",
  },
  "notes.publish.boxHint": {
    en: "Readers sign in with LosOS Login first.",
    sk: "Čitatelia sa najprv prihlásia cez LosOS Login.",
    de: "Lesende melden sich zuerst mit LosOS Login an.",
  },
  "notes.publish.private": {
    en: "Notes with publish: false or private: true in their properties stay out, and so do hidden folders.",
    sk: "Poznámky s publish: false alebo private: true vo vlastnostiach sa nezverejnia, ani skryté priečinky.",
    de: "Notizen mit publish: false oder private: true in den Eigenschaften bleiben draußen, ebenso versteckte Ordner.",
  },
  "notes.publish.submit": { en: "Publish", sk: "Zverejniť", de: "Veröffentlichen" },
  "notes.publish.save": { en: "Save", sk: "Uložiť", de: "Speichern" },
  "notes.publish.stop": { en: "Stop publishing", sk: "Zrušiť zverejnenie", de: "Nicht mehr veröffentlichen" },
  "notes.publish.done": { en: "{title} is published", sk: "{title} je zverejnená", de: "{title} ist veröffentlicht" },
  "notes.publish.stopped": { en: "{title} is no longer published", sk: "{title} už nie je zverejnená", de: "{title} ist nicht mehr veröffentlicht" },
  "notes.publish.copy": { en: "Copy the address", sk: "Kopírovať adresu", de: "Adresse kopieren" },
  "notes.publish.copied": { en: "Address copied", sk: "Adresa skopírovaná", de: "Adresse kopiert" },
  "notes.publish.open": { en: "Open the site", sk: "Otvoriť stránku", de: "Seite öffnen" },

  // ── a knowledge base ──
  "kb.contents": { en: "Contents", sk: "Obsah", de: "Inhalt" },
  "kb.notFound.title": { en: "No knowledge base here", sk: "Tu nie je žiadna znalostná báza", de: "Hier gibt es keine Wissensbasis" },
  "kb.notFound.text": {
    en: "It may have been taken down, or the address is wrong.",
    sk: "Možno ju zrušili alebo je adresa nesprávna.",
    de: "Vielleicht wurde sie entfernt, oder die Adresse stimmt nicht.",
  },
  "kb.offline.title": { en: "The box did not answer", sk: "Zariadenie neodpovedalo", de: "Die Box hat nicht geantwortet" },
  "kb.offline.text": { en: "Try again in a moment.", sk: "Skúste to znova o chvíľu.", de: "Versuch es gleich noch einmal." },
  "kb.noNote.title": { en: "No such page", sk: "Taká stránka neexistuje", de: "Diese Seite gibt es nicht" },
  "kb.noNote.text": {
    en: "Pick a page from the contents.",
    sk: "Vyberte stránku z obsahu.",
    de: "Wähl eine Seite aus dem Inhalt.",
  },
});
