import { defineMessages } from "./define";

/* The pages as MDX (src/pages/): the Pages section of the Look pane and the
 * editor that docks beside a page. Each block's line is here too, by its
 * name in blocks.json, so the editor can say what a block draws in the
 * language on screen. */

export default defineMessages({
  // ── The section on the Look pane ────────────────────────────────────────
  "pages.group": { en: "Pages", sk: "Stránky", de: "Seiten" },
  "pages.caption": {
    en: "Each page is written in MDX: text, LosOS components and the page's own blocks, in the order you put them. A page you have not changed looks exactly as LosOS ships it. Every save is a commit in a Git repository of its own on this box. Only administrators see this section.",
    sk: "Každá stránka je napísaná v MDX: text, komponenty LosOS a vlastné bloky stránky v poradí, v akom ich uložíte. Stránka, ktorú ste nezmenili, vyzerá presne tak, ako ju LosOS dodáva. Každé uloženie je commit vo vlastnom repozitári Git na tomto zariadení. Túto časť vidia iba správcovia.",
    de: "Jede Seite ist in MDX geschrieben: Text, LosOS-Komponenten und die eigenen Blöcke der Seite, in der Reihenfolge, die du festlegst. Eine Seite, die du nicht geändert hast, sieht genau so aus, wie LosOS sie ausliefert. Jedes Speichern ist ein Commit in einem eigenen Git-Repository auf dieser Box. Nur Administratoren sehen diesen Abschnitt.",
  },
  "pages.page.overview": { en: "Overview", sk: "Prehľad", de: "Übersicht" },
  "pages.page.settings.storage": { en: "Storage", sk: "Úložisko", de: "Speicher" },
  "pages.changed": { en: "Changed", sk: "Zmenená", de: "Geändert" },
  "pages.shipped": { en: "As shipped", sk: "Ako ju dodáva LosOS", de: "Wie ausgeliefert" },
  "pages.edit": { en: "Edit {name}", sk: "Upraviť {name}", de: "{name} bearbeiten" },
  "pages.style.title": { en: "Stylesheet", sk: "Štýly", de: "Stylesheet" },
  "pages.style.none": {
    en: "CSS for every page. None yet.",
    sk: "CSS pre všetky stránky. Zatiaľ žiadne.",
    de: "CSS für alle Seiten. Noch keins.",
  },
  "pages.style.set": {
    en: "CSS for every page, saved.",
    sk: "CSS pre všetky stránky, uložené.",
    de: "CSS für alle Seiten, gespeichert.",
  },
  "pages.style.edit": { en: "Edit the stylesheet", sk: "Upraviť štýly", de: "Stylesheet bearbeiten" },

  // ── The editor ──────────────────────────────────────────────────────────
  "pages.editor.title": { en: "{name} page", sk: "Stránka {name}", de: "Seite {name}" },
  "pages.editor.styleTitle": {
    en: "Stylesheet for the pages",
    sk: "Štýly stránok",
    de: "Stylesheet der Seiten",
  },
  "pages.editor.label": {
    en: "Source of the {name} page",
    sk: "Zdroj stránky {name}",
    de: "Quelltext der Seite {name}",
  },
  "pages.editor.styleLabel": {
    en: "The pages' stylesheet",
    sk: "Štýly stránok",
    de: "Stylesheet der Seiten",
  },
  "pages.editor.preview": {
    en: "The page beside this panel shows what you type. Nothing is saved until you press Save.",
    sk: "Stránka vedľa tohto panelu ukazuje, čo píšete. Nič sa neuloží, kým nestlačíte Uložiť.",
    de: "Die Seite neben diesem Bereich zeigt, was du tippst. Gespeichert wird erst, wenn du Speichern drückst.",
  },
  "pages.editor.stylePreview": {
    en: "Every page you open shows this stylesheet while the panel is open. html[data-page=\"overview\"] names one page.",
    sk: "Každá stránka, ktorú otvoríte, ukazuje tieto štýly, kým je panel otvorený. html[data-page=\"overview\"] pomenúva jednu stránku.",
    de: "Jede Seite, die du öffnest, zeigt dieses Stylesheet, solange der Bereich offen ist. html[data-page=\"overview\"] benennt eine Seite.",
  },
  "pages.editor.loading": { en: "Loading the page…", sk: "Načítava sa stránka…", de: "Seite wird geladen …" },
  "pages.editor.notLoaded": {
    en: "The page's source could not be loaded: {message}",
    sk: "Zdroj stránky sa nepodarilo načítať: {message}",
    de: "Der Quelltext der Seite konnte nicht geladen werden: {message}",
  },
  "pages.editor.save": { en: "Save", sk: "Uložiť", de: "Speichern" },
  "pages.editor.saving": { en: "Saving…", sk: "Ukladá sa…", de: "Wird gespeichert …" },
  "pages.editor.saved": {
    en: "Saved. Every browser that opens this box draws it now.",
    sk: "Uložené. Každý prehliadač, ktorý otvorí toto zariadenie, ju teraz zobrazí.",
    de: "Gespeichert. Jeder Browser, der diese Box öffnet, zeigt sie jetzt.",
  },
  "pages.editor.styleSaved": {
    en: "Saved. Every page uses the stylesheet now.",
    sk: "Uložené. Každá stránka teraz používa tieto štýly.",
    de: "Gespeichert. Jede Seite verwendet das Stylesheet jetzt.",
  },
  "pages.editor.notSaved": { en: "Not saved", sk: "Neuložené", de: "Nicht gespeichert" },
  "pages.editor.reset": {
    en: "Back to the default",
    sk: "Späť na predvolenú",
    de: "Zurück zum Standard",
  },
  "pages.editor.styleReset": {
    en: "Remove the stylesheet",
    sk: "Odstrániť štýly",
    de: "Stylesheet entfernen",
  },
  "pages.editor.resetDone": {
    en: "The page is LosOS's default again. Its history stays in the repository.",
    sk: "Stránka je opäť predvolená. Jej história zostáva v repozitári.",
    de: "Die Seite ist wieder der Standard von LosOS. Ihre Geschichte bleibt im Repository.",
  },
  "pages.editor.styleResetDone": {
    en: "The stylesheet is removed. Its history stays in the repository.",
    sk: "Štýly sú odstránené. Ich história zostáva v repozitári.",
    de: "Das Stylesheet ist entfernt. Seine Geschichte bleibt im Repository.",
  },
  "pages.editor.close": { en: "Close", sk: "Zavrieť", de: "Schließen" },
  "pages.editor.problems": {
    en: "Fix these before you save",
    sk: "Pred uložením opravte",
    de: "Vor dem Speichern beheben",
  },
  "pages.editor.problem": {
    en: "Line {line}, column {column}: {message}",
    sk: "Riadok {line}, stĺpec {column}: {message}",
    de: "Zeile {line}, Spalte {column}: {message}",
  },
  "pages.editor.defaultChanged.title": {
    en: "LosOS changed this page's default",
    sk: "LosOS zmenil predvolenú podobu tejto stránky",
    de: "LosOS hat den Standard dieser Seite geändert",
  },
  "pages.editor.defaultChanged.detail": {
    en: "Your page was written against an older default, so a block LosOS added since is not on it. Open the new default to see what changed.",
    sk: "Vaša stránka vychádza zo staršej predvolenej podoby, takže blok, ktorý LosOS odvtedy pridal, na nej nie je. Otvorte novú predvolenú podobu a pozrite si zmeny.",
    de: "Deine Seite beruht auf einem älteren Standard, ein Block, den LosOS seitdem hinzugefügt hat, fehlt also. Öffne den neuen Standard, um zu sehen, was sich geändert hat.",
  },
  "pages.editor.defaultChanged.open": {
    en: "Open the new default",
    sk: "Otvoriť novú predvolenú podobu",
    de: "Neuen Standard öffnen",
  },
  "pages.editor.blocks": { en: "Blocks of this page", sk: "Bloky tejto stránky", de: "Blöcke dieser Seite" },
  "pages.editor.blocksCaption": {
    en: "Write each block on a line of its own, as <Name />. A block you leave out is not drawn. You can also write text, the components of a LosOS document (Callout, Columns, Steps, Badge and the rest) and <Button href=\"/apps\">Label</Button> for a button.",
    sk: "Každý blok napíšte na samostatný riadok ako <Name />. Blok, ktorý vynecháte, sa nezobrazí. Môžete písať aj text, komponenty dokumentu LosOS (Callout, Columns, Steps, Badge a ďalšie) a <Button href=\"/apps\">Text</Button> pre tlačidlo.",
    de: "Schreib jeden Block in eine eigene Zeile, als <Name />. Ein Block, den du weglässt, wird nicht gezeigt. Du kannst auch Text schreiben, die Komponenten eines LosOS-Dokuments (Callout, Columns, Steps, Badge und die anderen) und <Button href=\"/apps\">Text</Button> für eine Schaltfläche.",
  },
  "pages.editor.flag": { en: "on or off", sk: "zapnuté alebo vypnuté", de: "an oder aus" },
  "pages.editor.history": { en: "History", sk: "História", de: "Verlauf" },
  "pages.editor.historyEmpty": {
    en: "No changes yet.",
    sk: "Zatiaľ bez zmien.",
    de: "Noch keine Änderungen.",
  },
  "pages.editor.historyBy": { en: "{by}, {at}", sk: "{by}, {at}", de: "{by}, {at}" },

  // ── What each block draws (blocks.json) ─────────────────────────────────
  "pages.block.BoxName": {
    en: "The box's name and whether it is answering.",
    sk: "Názov zariadenia a či odpovedá.",
    de: "Der Name der Box und ob sie antwortet.",
  },
  "pages.block.Applying": {
    en: "While the box applies a change: how far it got, and a game to play meanwhile. Shows nothing otherwise.",
    sk: "Kým zariadenie uplatňuje zmenu: ako ďaleko je a hra na krátenie čakania. Inak nič.",
    de: "Während die Box eine Änderung anwendet: wie weit sie ist, und ein Spiel für die Wartezeit. Sonst nichts.",
  },
  "pages.block.Apps": {
    en: "The apps on this box as tiles, with the storage strip under them.",
    sk: "Aplikácie na tomto zariadení ako dlaždice, pod nimi pás s úložiskom.",
    de: "Die Apps auf dieser Box als Kacheln, darunter der Speicherstreifen.",
  },
  "pages.block.Apps.hideStorage": {
    en: "Leave the storage strip out.",
    sk: "Vynechať pás s úložiskom.",
    de: "Den Speicherstreifen weglassen.",
  },
  "pages.block.Glance": {
    en: "The widgets: the ones LosOS has and the ones written by hand.",
    sk: "Widgety: tie od LosOS aj tie písané ručne.",
    de: "Die Widgets: die von LosOS und die von Hand geschriebenen.",
  },
  "pages.block.Disk": {
    en: "How full the disk is, and the reserve the box can add to it.",
    sk: "Ako je disk plný a rezerva, ktorú k nemu zariadenie môže pridať.",
    de: "Wie voll die Platte ist und die Reserve, die die Box hinzufügen kann.",
  },
});
