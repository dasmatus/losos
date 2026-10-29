import { defineMessages } from "./define";

/* The right-click menu's own items (components/context-menu.tsx): picked
 * text, links, and the page around everything else. */
export default defineMessages({
  "menu.label": { en: "Actions", sk: "Akcie", de: "Aktionen" },
  "menu.page": { en: "This page", sk: "Táto stránka", de: "Diese Seite" },
  "menu.text": { en: "Selected text", sk: "Označený text", de: "Markierter Text" },
  "menu.copy": { en: "Copy", sk: "Kopírovať", de: "Kopieren" },
  "menu.copied": { en: "Copied", sk: "Skopírované", de: "Kopiert" },
  "menu.copyFailed": {
    en: "The browser did not allow copying",
    sk: "Prehliadač kopírovanie nepovolil",
    de: "Der Browser hat das Kopieren nicht erlaubt",
  },
  "menu.searchFor": { en: "Search for \"{text}\"", sk: "Hľadať „{text}“", de: "Nach „{text}“ suchen" },
  "menu.open": { en: "Open", sk: "Otvoriť", de: "Öffnen" },
  "menu.newTab": { en: "Open in new tab", sk: "Otvoriť na novej karte", de: "In neuem Tab öffnen" },
  "menu.copyLink": { en: "Copy link", sk: "Kopírovať odkaz", de: "Link kopieren" },
  "menu.back": { en: "Back", sk: "Späť", de: "Zurück" },
  "menu.forward": { en: "Forward", sk: "Dopredu", de: "Vorwärts" },
  "menu.reload": { en: "Reload", sk: "Načítať znova", de: "Neu laden" },
  "menu.palette": { en: "Search the box", sk: "Hľadať v zariadení", de: "Auf der Box suchen" },
  "menu.handbook": { en: "Handbook", sk: "Príručka", de: "Handbuch" },
});
