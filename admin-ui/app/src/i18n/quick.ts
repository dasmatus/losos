import { defineMessages } from "./define";

/* The bar over a selection (components/quick-actions.tsx): picked text in
 * LosOS Office and Notes, cards and arrows in LosOS Diagrams. */
export default defineMessages({
  "quick.text": { en: "Actions for the selected text", sk: "Akcie pre označený text", de: "Aktionen für den markierten Text" },
  "quick.diagram": { en: "Actions for the selection", sk: "Akcie pre výber", de: "Aktionen für die Auswahl" },
  "quick.ask": { en: "Ask AI", sk: "Opýtať sa AI", de: "KI fragen" },
  "quick.ask.use": { en: "Replace selection", sk: "Nahradiť výber", de: "Auswahl ersetzen" },
  "quick.back": { en: "Back to the actions", sk: "Späť na akcie", de: "Zurück zu den Aktionen" },
  "quick.done": { en: "Done", sk: "Hotovo", de: "Fertig" },
  "quick.hint": {
    en: "Alt+F10 moves to the actions for the selection",
    sk: "Alt+F10 presunie na akcie pre výber",
    de: "Alt+F10 springt zu den Aktionen für die Auswahl",
  },

  "quick.link": { en: "Link", sk: "Odkaz", de: "Link" },
  "quick.link.placeholder": { en: "Paste or type a link", sk: "Vložte alebo napíšte odkaz", de: "Link einfügen oder tippen" },
  "quick.note": { en: "Link to a note", sk: "Odkaz na poznámku", de: "Link auf eine Notiz" },
  "quick.note.placeholder": { en: "Find a note", sk: "Nájsť poznámku", de: "Notiz suchen" },
  "quick.mention": { en: "Mention a person or file", sk: "Spomenúť osobu alebo súbor", de: "Person oder Datei erwähnen" },
  "quick.mention.placeholder": { en: "Find a person or file", sk: "Nájsť osobu alebo súbor", de: "Person oder Datei suchen" },
  "quick.copy": { en: "Copy", sk: "Kopírovať", de: "Kopieren" },
  "quick.copyLink": { en: "Copy link to this section", sk: "Kopírovať odkaz na túto časť", de: "Link auf diesen Abschnitt kopieren" },
  "quick.linkCopied": { en: "Link copied", sk: "Odkaz skopírovaný", de: "Link kopiert" },

  "quick.dg.color": { en: "Colour", sk: "Farba", de: "Farbe" },
  "quick.dg.edit": { en: "Edit text", sk: "Upraviť text", de: "Text bearbeiten" },
  "quick.dg.align": { en: "Align", sk: "Zarovnať", de: "Ausrichten" },
  "quick.dg.group": { en: "Group", sk: "Zoskupiť", de: "Gruppieren" },
  "quick.dg.connect": { en: "Connect with an arrow", sk: "Spojiť šípkou", de: "Mit einem Pfeil verbinden" },
  "quick.dg.duplicate": { en: "Duplicate", sk: "Duplikovať", de: "Duplizieren" },
  "quick.dg.delete": { en: "Delete", sk: "Odstrániť", de: "Löschen" },
});
