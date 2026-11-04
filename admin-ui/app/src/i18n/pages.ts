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
  "pages.page.settings.problems": { en: "Problems", sk: "Problémy", de: "Probleme" },
  "pages.page.settings.logs": { en: "Logs", sk: "Záznamy", de: "Protokolle" },
  "pages.page.settings.network": { en: "Network", sk: "Sieť", de: "Netzwerk" },
  "pages.page.settings.hardware": { en: "Hardware", sk: "Hardvér", de: "Hardware" },
  "pages.page.settings.security": { en: "Security", sk: "Zabezpečenie", de: "Sicherheit" },
  "pages.page.settings.statistics": { en: "Statistics", sk: "Štatistiky", de: "Statistik" },
  "pages.page.settings.about": { en: "About", sk: "O zariadení", de: "Über" },
  "pages.page.settings.reset": { en: "Reset", sk: "Reset", de: "Zurücksetzen" },
  "pages.page.settings.look": { en: "Look", sk: "Vzhľad", de: "Aussehen" },
  "pages.page.settings.mesh": { en: "Mesh", sk: "Mesh", de: "Mesh" },
  "pages.page.settings.market": { en: "Market", sk: "Trh", de: "Markt" },
  "pages.page.settings.machines": { en: "Machines", sk: "Stroje", de: "Maschinen" },
  "pages.page.settings.apps": { en: "Apps", sk: "Aplikácie", de: "Apps" },
  "pages.page.settings.credentials": { en: "Credentials", sk: "Prihlasovacie údaje", de: "Zugangsdaten" },
  "pages.page.settings.access": { en: "Access tokens", sk: "Prístupové tokeny", de: "Zugriffstokens" },
  "pages.page.settings.account": { en: "Your account", sk: "Váš účet", de: "Dein Konto" },
  "pages.page.settings.users": { en: "Users", sk: "Používatelia", de: "Benutzer" },
  "pages.page.settings.moderation": { en: "Moderation", sk: "Moderovanie", de: "Moderation" },
  "pages.page.settings.advanced": { en: "Advanced", sk: "Rozšírené", de: "Erweitert" },
  "pages.page.settings.history": { en: "History", sk: "História", de: "Historie" },
  "pages.page.settings.backup": { en: "Backup", sk: "Záloha", de: "Sicherung" },
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
  "pages.block.overview.BoxName": {
    en: "The box's name and whether it is answering.",
    sk: "Názov zariadenia a či odpovedá.",
    de: "Der Name der Box und ob sie antwortet.",
  },
  "pages.block.overview.Applying": {
    en: "While the box applies a change: how far it got, and a game to play meanwhile. Shows nothing otherwise.",
    sk: "Kým zariadenie uplatňuje zmenu: ako ďaleko je a hra na krátenie čakania. Inak nič.",
    de: "Während die Box eine Änderung anwendet: wie weit sie ist, und ein Spiel für die Wartezeit. Sonst nichts.",
  },
  "pages.block.overview.Apps": {
    en: "The apps on this box as tiles, with the storage strip under them.",
    sk: "Aplikácie na tomto zariadení ako dlaždice, pod nimi pás s úložiskom.",
    de: "Die Apps auf dieser Box als Kacheln, darunter der Speicherstreifen.",
  },
  "pages.block.overview.Apps.hideStorage": {
    en: "Leave the storage strip out.",
    sk: "Vynechať pás s úložiskom.",
    de: "Den Speicherstreifen weglassen.",
  },
  "pages.block.overview.Glance": {
    en: "The widgets: the ones LosOS has and the ones written by hand.",
    sk: "Widgety: tie od LosOS aj tie písané ručne.",
    de: "Die Widgets: die von LosOS und die von Hand geschriebenen.",
  },
  "pages.block.settings.storage.Disk": {
    en: "How full the disk is, and the reserve the box can add to it.",
    sk: "Ako je disk plný a rezerva, ktorú k nemu zariadenie môže pridať.",
    de: "Wie voll die Platte ist und die Reserve, die die Box hinzufügen kann.",
  },
  "pages.block.settings.network.Name": {
    en: "The box's name and the address it is reached at on this network.",
    sk: "Názov zariadenia a adresa, na ktorej je v tejto sieti dostupné.",
    de: "Der Name der Box und die Adresse, unter der sie in diesem Netz erreichbar ist.",
  },
  "pages.block.settings.network.Reaching": {
    en: "Whether the box can be reached from outside this network.",
    sk: "Či je zariadenie dostupné aj mimo tejto siete.",
    de: "Ob die Box auch von außerhalb dieses Netzes erreichbar ist.",
  },
  "pages.block.settings.network.Windows": {
    en: "Whether the box shows up under Network in File Explorer. Shows nothing on a box without it.",
    sk: "Či sa zariadenie zobrazí v Prieskumníkovi pod Sieťou. Na zariadení bez tejto možnosti nič.",
    de: "Ob die Box im Explorer unter Netzwerk erscheint. Auf einer Box ohne diese Möglichkeit nichts.",
  },
  "pages.block.settings.network.Hidden": {
    en: "The Tor and I2P addresses. Shows nothing on a box without them.",
    sk: "Adresy v sieťach Tor a I2P. Na zariadení bez nich nič.",
    de: "Die Tor- und I2P-Adressen. Auf einer Box ohne sie nichts.",
  },
  "pages.block.settings.network.Domains": {
    en: "The box's own domain names.",
    sk: "Vlastné domény zariadenia.",
    de: "Die eigenen Domains der Box.",
  },
  "pages.block.settings.hardware.Graphics": {
    en: "The switch for the graphics chip.",
    sk: "Prepínač grafického čipu.",
    de: "Der Schalter für den Grafikchip.",
  },
  "pages.block.settings.security.Protections": {
    en: "The extra protections, each with what it costs, and the note on a box without a TPM.",
    sk: "Ďalšie ochrany, každá s tým, čo stojí, a upozornenie na zariadení bez TPM.",
    de: "Die zusätzlichen Schutzmaßnahmen, jede mit ihrem Preis, und der Hinweis auf einer Box ohne TPM.",
  },
  "pages.block.settings.statistics.Counts": {
    en: "What was counted on the apps' ports, as charts.",
    sk: "Čo sa napočítalo na portoch aplikácií, v grafoch.",
    de: "Was an den Ports der Apps gezählt wurde, als Diagramme.",
  },
  "pages.block.settings.about.ThisBox": {
    en: "The box's name, its addresses and how its storage is used.",
    sk: "Názov zariadenia, jeho adresy a ako sa používa jeho úložisko.",
    de: "Der Name der Box, ihre Adressen und wie ihr Speicher genutzt wird.",
  },
  "pages.block.settings.about.Handbook": {
    en: "The link to the handbook.",
    sk: "Odkaz na príručku.",
    de: "Der Link zum Handbuch.",
  },
  "pages.block.settings.about.Tour": {
    en: "The link that shows the tour again.",
    sk: "Odkaz, ktorý znova spustí prehliadku.",
    de: "Der Link, der die Tour noch einmal zeigt.",
  },
  "pages.block.settings.about.SpareTime": {
    en: "Whether the box is on a mesh and when it lends its time.",
    sk: "Či je zariadenie v sieti a kedy požičiava svoj čas.",
    de: "Ob die Box in einem Mesh ist und wann sie ihre Zeit verleiht.",
  },
  "pages.block.settings.about.Updates": {
    en: "When the box updates and restarts.",
    sk: "Kedy sa zariadenie aktualizuje a reštartuje.",
    de: "Wann sich die Box aktualisiert und neu startet.",
  },
  "pages.block.settings.reset.StartOver": {
    en: "The button that resets the settings, and its confirmation.",
    sk: "Tlačidlo, ktoré obnoví nastavenia, a jeho potvrdenie.",
    de: "Der Knopf, der die Einstellungen zurücksetzt, und seine Bestätigung.",
  },
  "pages.block.settings.reset.Erase": {
    en: "Erasing the box, with a backup first.",
    sk: "Vymazanie zariadenia, najprv so zálohou.",
    de: "Das Löschen der Box, zuerst mit einer Sicherung.",
  },
  "pages.block.settings.look.NotLoaded": {
    en: "Why the look did not load. Shows nothing once it has.",
    sk: "Prečo sa vzhľad nenačítal. Keď sa načíta, nič.",
    de: "Warum das Aussehen nicht geladen wurde. Sobald es geladen ist, nichts.",
  },
  "pages.block.settings.look.Background": {
    en: "The background picture.",
    sk: "Obrázok na pozadí.",
    de: "Das Hintergrundbild.",
  },
  "pages.block.settings.look.Widgets": {
    en: "The widgets written by hand.",
    sk: "Widgety písané ručne.",
    de: "Die von Hand geschriebenen Widgets.",
  },
  "pages.block.settings.mesh.Edge": {
    en: "The edges in reach and the one this box uses.",
    sk: "Edge v dosahu a ten, ktorý zariadenie používa.",
    de: "Die erreichbaren Edges und das, das diese Box nutzt.",
  },
  "pages.block.settings.mesh.Cluster": {
    en: "The mesh this box is in.",
    sk: "Sieť, v ktorej je toto zariadenie.",
    de: "Das Mesh, in dem diese Box ist.",
  },
  "pages.block.settings.mesh.LocalShare": {
    en: "Sharing with the gateways of this network only.",
    sk: "Zdieľanie len s bránami tejto siete.",
    de: "Teilen nur mit den Gateways dieses Netzes.",
  },
  "pages.block.settings.mesh.Manage": {
    en: "Who may manage this box from the edge.",
    sk: "Kto môže toto zariadenie spravovať z edge.",
    de: "Wer diese Box vom Edge aus verwalten darf.",
  },
  "pages.block.settings.mesh.Join": {
    en: "The switch that joins the mesh.",
    sk: "Prepínač, ktorý pripojí zariadenie k sieti.",
    de: "Der Schalter, der dem Mesh beitritt.",
  },
  "pages.block.settings.mesh.SpareTime": {
    en: "When the box lends its processor to the mesh.",
    sk: "Kedy zariadenie požičiava svoj procesor sieti.",
    de: "Wann die Box dem Mesh ihren Prozessor leiht.",
  },
  "pages.block.settings.market.Sharing": {
    en: "The switch that shares the disk with the mesh.",
    sk: "Prepínač, ktorý zdieľa disk so sieťou.",
    de: "Der Schalter, der die Platte mit dem Mesh teilt.",
  },
  "pages.block.settings.market.Lend": {
    en: "Lending this box to AI models on the mesh.",
    sk: "Požičiavanie tohto zariadenia AI modelom v sieti.",
    de: "Das Verleihen dieser Box an KI-Modelle im Mesh.",
  },
  "pages.block.settings.market.Status": {
    en: "While the market loads, or when it cannot be reached: why. Shows nothing once it has answered.",
    sk: "Kým sa trh načítava alebo keď je nedostupný: prečo. Keď odpovie, nič.",
    de: "Während der Markt lädt oder wenn er nicht erreichbar ist: warum. Sobald er antwortet, nichts.",
  },
  "pages.block.settings.market.Yours": {
    en: "What this box has bought.",
    sk: "Čo toto zariadenie kúpilo.",
    de: "Was diese Box gekauft hat.",
  },
  "pages.block.settings.market.Buy": {
    en: "What is on offer.",
    sk: "Čo je v ponuke.",
    de: "Was angeboten wird.",
  },
  "pages.block.settings.market.AiModels": {
    en: "AI models paid for in USDC. Shows nothing on an edge without them.",
    sk: "AI modely platené v USDC. Na edge bez nich nič.",
    de: "KI-Modelle, bezahlt in USDC. Auf einem Edge ohne sie nichts.",
  },
  "pages.block.settings.market.Sell": {
    en: "What this box offers for sale.",
    sk: "Čo toto zariadenie ponúka na predaj.",
    de: "Was diese Box zum Verkauf anbietet.",
  },
  "pages.block.settings.market.Earnings": {
    en: "What this box has earned. Shows nothing before it has.",
    sk: "Čo toto zariadenie zarobilo. Kým nič nezarobí, nič.",
    de: "Was diese Box verdient hat. Vorher nichts.",
  },
  "pages.block.settings.machines.Status": {
    en: "While the machines load, or when they are not offered: why. Shows nothing once they have.",
    sk: "Kým sa stroje načítavajú alebo keď nie sú v ponuke: prečo. Potom nič.",
    de: "Während die Maschinen laden oder wenn sie nicht angeboten werden: warum. Danach nichts.",
  },
  "pages.block.settings.machines.Start": {
    en: "Starting a new machine.",
    sk: "Spustenie nového stroja.",
    de: "Eine neue Maschine starten.",
  },
  "pages.block.settings.machines.Mine": {
    en: "The machines this box runs on the mesh.",
    sk: "Stroje, ktoré toto zariadenie beží v sieti.",
    de: "Die Maschinen, die diese Box im Mesh betreibt.",
  },
  "pages.block.settings.machines.Images": {
    en: "The disk images to start machines from.",
    sk: "Obrazy diskov, z ktorých sa spúšťajú stroje.",
    de: "Die Plattenabbilder, aus denen Maschinen starten.",
  },
  "pages.block.settings.machines.Host": {
    en: "The machines this box hosts for others.",
    sk: "Stroje, ktoré toto zariadenie hostí pre iných.",
    de: "Die Maschinen, die diese Box für andere beherbergt.",
  },
  "pages.block.settings.apps.Sites": {
    en: "LosOS Cloud's and LosOS Git's settings, each folded.",
    sk: "Nastavenia LosOS Cloud a LosOS Git, každé zložené.",
    de: "Die Einstellungen von LosOS Cloud und LosOS Git, jede eingeklappt.",
  },
  "pages.block.settings.apps.Yours": {
    en: "The apps built on this box.",
    sk: "Aplikácie vytvorené na tomto zariadení.",
    de: "Die auf dieser Box gebauten Apps.",
  },
  "pages.block.settings.apps.Box": {
    en: "The apps installed on this box, and finding more.",
    sk: "Aplikácie nainštalované na tomto zariadení a hľadanie ďalších.",
    de: "Die auf dieser Box installierten Apps und die Suche nach weiteren.",
  },
  "pages.block.settings.credentials.Status": {
    en: "While the pane loads, or when it cannot: why. Shows nothing once it has.",
    sk: "Kým sa časť načítava alebo keď sa nedá: prečo. Keď sa načíta, nič.",
    de: "Während der Bereich lädt oder wenn er es nicht kann: warum. Sobald er geladen ist, nichts.",
  },
  "pages.block.settings.credentials.Seal": {
    en: "What the box's secrets are sealed to.",
    sk: "K čomu sú zapečatené tajomstvá zariadenia.",
    de: "Woran die Geheimnisse der Box versiegelt sind.",
  },
  "pages.block.settings.credentials.Apps": {
    en: "The secrets of each app, and making new ones.",
    sk: "Tajomstvá každej aplikácie a vytváranie nových.",
    de: "Die Geheimnisse jeder App und das Erzeugen neuer.",
  },
  "pages.block.settings.access.Status": {
    en: "While the pane loads, or when it cannot: why. Shows nothing once it has.",
    sk: "Kým sa časť načítava alebo keď sa nedá: prečo. Keď sa načíta, nič.",
    de: "Während der Bereich lädt oder wenn er es nicht kann: warum. Sobald er geladen ist, nichts.",
  },
  "pages.block.settings.access.HostKey": {
    en: "The box's own key, to check it by.",
    sk: "Vlastný kľúč zariadenia, podľa ktorého ho overíte.",
    de: "Der eigene Schlüssel der Box, um sie daran zu prüfen.",
  },
  "pages.block.settings.access.Keys": {
    en: "The keys that may sign calls to this box.",
    sk: "Kľúče, ktoré môžu podpisovať volania na toto zariadenie.",
    de: "Die Schlüssel, die Aufrufe an diese Box signieren dürfen.",
  },
  "pages.block.settings.access.Tokens": {
    en: "The access tokens made so far.",
    sk: "Doteraz vytvorené prístupové tokeny.",
    de: "Die bisher erstellten Zugriffstoken.",
  },
  "pages.block.settings.access.NewToken": {
    en: "Making a new access token.",
    sk: "Vytvorenie nového prístupového tokenu.",
    de: "Ein neues Zugriffstoken erstellen.",
  },
  "pages.block.settings.account.Status": {
    en: "While the pane loads, or when it cannot: why. Shows nothing once it has.",
    sk: "Kým sa časť načítava alebo keď sa nedá: prečo. Keď sa načíta, nič.",
    de: "Während der Bereich lädt oder wenn er es nicht kann: warum. Sobald er geladen ist, nichts.",
  },
  "pages.block.settings.account.Held": {
    en: "Why the account is held, while it is. Shows nothing otherwise.",
    sk: "Prečo je účet zadržaný, kým je. Inak nič.",
    de: "Warum das Konto gesperrt ist, solange es das ist. Sonst nichts.",
  },
  "pages.block.settings.account.Profile": {
    en: "The name and the picture others see.",
    sk: "Meno a obrázok, ktoré vidia ostatní.",
    de: "Der Name und das Bild, die andere sehen.",
  },
  "pages.block.settings.account.Contact": {
    en: "The account's details, such as its email address.",
    sk: "Údaje účtu, napríklad e-mailová adresa.",
    de: "Die Angaben des Kontos, etwa die E-Mail-Adresse.",
  },
  "pages.block.settings.account.Password": {
    en: "Changing the password.",
    sk: "Zmena hesla.",
    de: "Das Passwort ändern.",
  },
  "pages.block.settings.account.Factors": {
    en: "The second steps of signing in.",
    sk: "Druhé kroky prihlásenia.",
    de: "Die zweiten Schritte der Anmeldung.",
  },
  "pages.block.settings.account.Pictures": {
    en: "The pictures kept for the account.",
    sk: "Obrázky uložené pre účet.",
    de: "Die für das Konto gespeicherten Bilder.",
  },
  "pages.block.settings.account.Windows": {
    en: "Signing in to the files from Windows.",
    sk: "Prihlásenie k súborom z Windows.",
    de: "Die Anmeldung an den Dateien von Windows aus.",
  },
  "pages.block.settings.users.Status": {
    en: "While the pane loads, or when it cannot: why. Shows nothing once it has.",
    sk: "Kým sa časť načítava alebo keď sa nedá: prečo. Keď sa načíta, nič.",
    de: "Während der Bereich lädt oder wenn er es nicht kann: warum. Sobald er geladen ist, nichts.",
  },
  "pages.block.settings.users.People": {
    en: "The people who use this box.",
    sk: "Ľudia, ktorí používajú toto zariadenie.",
    de: "Die Menschen, die diese Box nutzen.",
  },
  "pages.block.settings.users.Roles": {
    en: "What each role may do.",
    sk: "Čo smie každá rola.",
    de: "Was jede Rolle darf.",
  },
  "pages.block.settings.users.CustomRoles": {
    en: "The roles made on this box. Shows nothing where there are none.",
    sk: "Roly vytvorené na tomto zariadení. Kde nie sú, nič.",
    de: "Die auf dieser Box erstellten Rollen. Wo es keine gibt, nichts.",
  },
  "pages.block.settings.users.Rules": {
    en: "The rules for new people. Shows nothing where there are none.",
    sk: "Pravidlá pre nových ľudí. Kde nie sú, nič.",
    de: "Die Regeln für neue Menschen. Wo es keine gibt, nichts.",
  },
  "pages.block.settings.users.NewPerson": {
    en: "Adding a person.",
    sk: "Pridanie osoby.",
    de: "Eine Person hinzufügen.",
  },
  "pages.block.settings.moderation.Status": {
    en: "While the pane loads, or when it cannot: why. Shows nothing once it has.",
    sk: "Kým sa časť načítava alebo keď sa nedá: prečo. Keď sa načíta, nič.",
    de: "Während der Bereich lädt oder wenn er es nicht kann: warum. Sobald er geladen ist, nichts.",
  },
  "pages.block.settings.moderation.Reports": {
    en: "What people have reported.",
    sk: "Čo ľudia nahlásili.",
    de: "Was Menschen gemeldet haben.",
  },
  "pages.block.settings.moderation.People": {
    en: "The people held or warned.",
    sk: "Ľudia zadržaní alebo varovaní.",
    de: "Die gesperrten oder verwarnten Menschen.",
  },
  "pages.block.settings.moderation.Log": {
    en: "What was done, and by whom.",
    sk: "Čo sa urobilo a kto to urobil.",
    de: "Was getan wurde, und von wem.",
  },
  "pages.block.settings.advanced.Search": {
    en: "The search over every option, and how many there are.",
    sk: "Hľadanie vo všetkých možnostiach a ich počet.",
    de: "Die Suche über alle Optionen, und wie viele es sind.",
  },
  "pages.block.settings.advanced.Nothing": {
    en: "What a search that finds nothing says.",
    sk: "Čo povie hľadanie, ktoré nič nenájde.",
    de: "Was eine Suche sagt, die nichts findet.",
  },
  "pages.block.settings.advanced.Options": {
    en: "Every option, by group, and the question before a dangerous change.",
    sk: "Všetky možnosti po skupinách a otázka pred nebezpečnou zmenou.",
    de: "Alle Optionen nach Gruppen, und die Frage vor einer gefährlichen Änderung.",
  },
  "pages.block.settings.advanced.Strays": {
    en: "Lines in the settings file that no option names. Shows nothing where there are none.",
    sk: "Riadky v súbore nastavení, ktoré nepomenúva žiadna možnosť. Kde nie sú, nič.",
    de: "Zeilen in der Einstellungsdatei, die keine Option nennt. Wo es keine gibt, nichts.",
  },
  "pages.block.settings.advanced.About": {
    en: "Where the options are kept and what is left out.",
    sk: "Kde sú možnosti uložené a čo je vynechané.",
    de: "Wo die Optionen liegen und was ausgelassen ist.",
  },
  "pages.block.settings.history.Repository": {
    en: "The Git repository the settings are kept in.",
    sk: "Repozitár Git, v ktorom sú nastavenia.",
    de: "Das Git-Repository, in dem die Einstellungen liegen.",
  },
  "pages.block.settings.history.Clone": {
    en: "How to clone it, and syncing now. Shows nothing without a repository.",
    sk: "Ako ho naklonovať a synchronizovať hneď. Bez repozitára nič.",
    de: "Wie man es klont, und jetzt abgleichen. Ohne Repository nichts.",
  },
  "pages.block.settings.history.Update": {
    en: "Where the box takes its updates from.",
    sk: "Odkiaľ zariadenie berie aktualizácie.",
    de: "Woher die Box ihre Updates bezieht.",
  },
  "pages.block.settings.history.Commits": {
    en: "The latest changes.",
    sk: "Posledné zmeny.",
    de: "Die letzten Änderungen.",
  },
  "pages.block.settings.backup.Waiting": {
    en: "While a change is applied: that backups wait for it. Shows nothing otherwise.",
    sk: "Kým sa uplatňuje zmena: že zálohy na ňu čakajú. Inak nič.",
    de: "Während eine Änderung angewendet wird: dass Sicherungen darauf warten. Sonst nichts.",
  },
  "pages.block.settings.backup.Target": {
    en: "Where backups go.",
    sk: "Kam idú zálohy.",
    de: "Wohin die Sicherungen gehen.",
  },
  "pages.block.settings.backup.Code": {
    en: "The recovery code the backups are encrypted with.",
    sk: "Kód obnovy, ktorým sú zálohy zašifrované.",
    de: "Der Wiederherstellungscode, mit dem die Sicherungen verschlüsselt sind.",
  },
  "pages.block.settings.backup.Backups": {
    en: "The backups made so far, and making one now.",
    sk: "Doteraz vytvorené zálohy a vytvorenie zálohy hneď.",
    de: "Die bisherigen Sicherungen, und jetzt eine erstellen.",
  },
  "pages.block.settings.backup.Restore": {
    en: "Restoring from a backup.",
    sk: "Obnova zo zálohy.",
    de: "Aus einer Sicherung wiederherstellen.",
  },
  "pages.block.settings.problems.Status": {
    en: "Loading, and what shows when the problems cannot be read.",
    sk: "Načítanie a to, čo sa zobrazí, keď sa problémy nedajú prečítať.",
    de: "Das Laden und was erscheint, wenn die Probleme nicht lesbar sind.",
  },
  "pages.block.settings.problems.Reporting": {
    en: "Whether reports can be sent, and where they go.",
    sk: "Či sa hlásenia dajú odoslať a kam idú.",
    de: "Ob Berichte gesendet werden können und wohin sie gehen.",
  },
  "pages.block.settings.problems.List": {
    en: "The problems the box found.",
    sk: "Problémy, ktoré zariadenie našlo.",
    de: "Die Probleme, die die Box gefunden hat.",
  },
  "pages.block.settings.problems.Dismissed": {
    en: "The problems put aside.",
    sk: "Odložené problémy.",
    de: "Die beiseitegelegten Probleme.",
  },
  "pages.block.settings.problems.Report": {
    en: "The report you read before you send it.",
    sk: "Hlásenie, ktoré si prečítate pred odoslaním.",
    de: "Der Bericht, den du vor dem Senden liest.",
  },
  "pages.block.settings.logs.Filters": {
    en: "The service, level, time and text to show.",
    sk: "Služba, úroveň, čas a text, ktoré sa majú zobraziť.",
    de: "Dienst, Stufe, Zeitraum und Text, die gezeigt werden.",
  },
  "pages.block.settings.logs.Notices": {
    en: "What went wrong while reading or downloading.",
    sk: "Čo sa pokazilo pri čítaní alebo sťahovaní.",
    de: "Was beim Lesen oder Herunterladen schiefging.",
  },
  "pages.block.settings.logs.Lines": {
    en: "The lines themselves.",
    sk: "Samotné riadky.",
    de: "Die Zeilen selbst.",
  },
  "pages.block.settings.logs.Hidden": {
    en: "The note on what is hidden.",
    sk: "Poznámka o tom, čo je skryté.",
    de: "Der Hinweis darauf, was ausgeblendet ist.",
  },
});
