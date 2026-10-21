import { defineMessages } from "./define";

/* Office documents: the editor page (src/cloud/office/, src/office/). */
export default defineMessages({
  "office.back": { en: "Back to files", sk: "Späť na súbory", de: "Zurück zu den Dateien" },
  "office.opening": { en: "Opening {name}", sk: "Otvára sa {name}", de: "{name} wird geöffnet" },
  "office.openingPlain": { en: "Opening the document", sk: "Otvára sa dokument", de: "Das Dokument wird geöffnet" },
  "office.readOnly": { en: "Read only", sk: "Len na čítanie", de: "Nur lesen" },
  "office.frameTitle": { en: "Office editor: {name}", sk: "Editor dokumentov: {name}", de: "Office-Editor: {name}" },
  "office.open": { en: "Open in Office", sk: "Otvoriť v Office", de: "In Office öffnen" },
  "office.error.notFound": {
    en: "This file is gone, or you no longer have access to it.",
    sk: "Tento súbor už neexistuje alebo k nemu už nemáte prístup.",
    de: "Diese Datei gibt es nicht mehr, oder du hast keinen Zugriff mehr darauf.",
  },
  "office.error.unsupported": {
    en: "Office does not open .{ext} files.",
    sk: "Office nevie otvoriť súbory .{ext}.",
    de: "Office kann .{ext}-Dateien nicht öffnen.",
  },
  "office.error.starting": {
    en: "Office is still starting. Try again in a minute.",
    sk: "Office sa ešte spúšťa. Skúste to znova o minútu.",
    de: "Office startet noch. Versuch es in einer Minute noch einmal.",
  },
  "office.error.signedOut": {
    en: "Sign in to LosOS to open this document.",
    sk: "Ak chcete otvoriť tento dokument, prihláste sa do LosOS.",
    de: "Melde dich bei LosOS an, um dieses Dokument zu öffnen.",
  },
  "office.error.other": {
    en: "The document did not open.",
    sk: "Dokument sa neotvoril.",
    de: "Das Dokument hat sich nicht geöffnet.",
  },
  "office.error.link": {
    en: "This link to a document is incomplete.",
    sk: "Tento odkaz na dokument je neúplný.",
    de: "Dieser Link zu einem Dokument ist unvollständig.",
  },
  "office.retry": { en: "Try again", sk: "Skúsiť znova", de: "Erneut versuchen" },
  "office.files.title": { en: "Documents", sk: "Dokumenty", de: "Dokumente" },
  "office.files.loading": { en: "Loading the documents", sk: "Načítavajú sa dokumenty", de: "Die Dokumente werden geladen" },
  "office.files.name": { en: "Name", sk: "Názov", de: "Name" },
  "office.files.modified": { en: "Changed", sk: "Zmenené", de: "Geändert" },
  "office.files.size": { en: "Size", sk: "Veľkosť", de: "Größe" },
  "office.files.empty.title": { en: "No documents yet", sk: "Zatiaľ žiadne dokumenty", de: "Noch keine Dokumente" },
  "office.files.empty.body": {
    en: "Documents put in the box's Office folder show up here.",
    sk: "Tu sa zobrazia dokumenty, ktoré vložíte do priečinka Office na zariadení.",
    de: "Hier erscheinen die Dokumente aus dem Office-Ordner der Box.",
  },
});
