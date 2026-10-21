import { defineMessages } from "./define";

/* The first-run tour (src/tour/), shown once after setup and again from
 * the About pane. One title and one sentence per step: the scene above
 * them does the showing. */
export default defineMessages({
  "tour.count": { en: "{n} of {total}", sk: "{n} z {total}", de: "{n} von {total}" },
  "tour.skip": { en: "Skip the tour", sk: "Preskočiť prehliadku", de: "Tour überspringen" },
  "tour.back": { en: "Back", sk: "Späť", de: "Zurück" },
  "tour.next": { en: "Next", sk: "Ďalej", de: "Weiter" },
  "tour.done": { en: "Done", sk: "Hotovo", de: "Fertig" },
  "tour.step": { en: "Go to step {n}", sk: "Prejsť na krok {n}", de: "Zu Schritt {n}" },

  "tour.overview.title": {
    en: "Your apps, on your box",
    sk: "Vaše aplikácie, na vašom zariadení",
    de: "Deine Apps, auf deiner Box",
  },
  "tour.overview.body": {
    en: "LosOS Cloud and LosOS Git run here, and the Overview opens every app they bring.",
    sk: "LosOS Cloud a LosOS Git bežia tu a v Prehľade otvoríte každú aplikáciu, ktorú prinášajú.",
    de: "LosOS Cloud und LosOS Git laufen hier, und die Übersicht öffnet jede App, die sie mitbringen.",
  },
  "tour.apps.title": { en: "Add more apps", sk: "Pridajte ďalšie aplikácie", de: "Weitere Apps hinzufügen" },
  "tour.apps.body": {
    en: "Search under Apps and install what you find. It runs on this box, next to your files.",
    sk: "Vyhľadajte ich v časti Aplikácie a nainštalujte. Pobežia na tomto zariadení, vedľa vašich súborov.",
    de: "Such unter Apps und installiere, was du findest. Es läuft auf dieser Box, neben deinen Dateien.",
  },
  "tour.storage.title": { en: "See where the space goes", sk: "Pozrite sa, kam ide miesto", de: "Sieh, wohin der Platz geht" },
  "tour.storage.body": {
    en: "Storage shows what is yours, what is lent to the mesh and what is still free.",
    sk: "Úložisko ukazuje, čo je vaše, čo je požičané do mesh a čo je ešte voľné.",
    de: "Speicher zeigt, was dir gehört, was dem Mesh geliehen ist und was noch frei ist.",
  },
  "tour.mesh.title": { en: "Lend it while you sleep", sk: "Požičajte ho, kým spíte", de: "Verleih sie, während du schläfst" },
  "tour.mesh.body": {
    en: "Under Mesh, choose the night hours this box helps other boxes. The rest of the day it is yours.",
    sk: "V časti Mesh vyberte nočné hodiny, keď toto zariadenie pomáha iným. Zvyšok dňa patrí vám.",
    de: "Unter Mesh wählst du die Nachtstunden, in denen diese Box anderen hilft. Der Rest des Tages gehört dir.",
  },
  "tour.history.title": { en: "It updates itself at night", sk: "V noci sa aktualizuje sama", de: "Nachts aktualisiert sie sich selbst" },
  "tour.history.body": {
    en: "Every night it restarts and looks for updates. History lists each change it made.",
    sk: "Každú noc sa reštartuje a hľadá aktualizácie. História ukazuje každú zmenu.",
    de: "Jede Nacht startet sie neu und sucht nach Updates. Unter Historie steht jede Änderung.",
  },
  "tour.about.title": { en: "Find this tour again", sk: "Prehliadku nájdete znova", de: "Diese Tour wiederfinden" },
  "tour.about.body": {
    en: "It stays under Settings, About, whenever you want another look.",
    sk: "Je v časti Nastavenia, O zariadení, kedykoľvek si ju budete chcieť pozrieť.",
    de: "Sie liegt unter Einstellungen, Über, wann immer du sie noch einmal sehen willst.",
  },

  /* Words inside the scenes that no other screen already has. */
  "tour.scene.documents": { en: "Documents", sk: "Dokumenty", de: "Dokumente" },
  "tour.scene.lending": { en: "Lending", sk: "Požičiava", de: "Verleiht" },
});
