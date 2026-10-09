import { defineMessages } from "./define";

/* The Look pane (screens/settings/pane-look.tsx), the hand-written widget
 * editor and frame (widgets/hand-editor.tsx, widgets/hand-frame.tsx) and
 * the gallery's "by hand" section. The pane's own name and summary are with
 * the other panes in settings.ts. */

export default defineMessages({
  // ── Background ──────────────────────────────────────────────────────────
  "look.background.group": { en: "Background", sk: "Pozadie", de: "Hintergrund" },
  "look.background.none": { en: "Plain", sk: "Bez obrázka", de: "Schlicht" },
  "look.background.tide": { en: "Tide", sk: "Príliv", de: "Gezeiten" },
  "look.background.grid": { en: "Grid", sk: "Mriežka", de: "Raster" },
  "look.background.dusk": { en: "Dusk", sk: "Súmrak", de: "Abenddämmerung" },
  "look.background.own": { en: "Your own picture", sk: "Vlastný obrázok", de: "Eigenes Bild" },
  "look.background.choose": {
    en: "Choose a picture…",
    sk: "Vybrať obrázok…",
    de: "Bild auswählen …",
  },
  "look.background.replace": { en: "Replace…", sk: "Nahradiť…", de: "Ersetzen …" },
  "look.background.remove": {
    en: "Remove the picture",
    sk: "Odstrániť obrázok",
    de: "Bild entfernen",
  },
  "look.background.uploading": { en: "Uploading…", sk: "Nahráva sa…", de: "Wird hochgeladen …" },
  "look.background.caption": {
    en: "Takes effect at once, on every browser that opens this box; nothing rebuilds. A picture of your own can be a PNG, JPEG, GIF or WebP up to {mb} MB.",
    sk: "Prejaví sa hneď, v každom prehliadači, ktorý toto zariadenie otvorí; nič sa neprestavuje. Vlastný obrázok môže byť PNG, JPEG, GIF alebo WebP do {mb} MB.",
    de: "Gilt sofort, in jedem Browser, der diese Box öffnet; nichts wird neu gebaut. Ein eigenes Bild kann ein PNG, JPEG, GIF oder WebP bis {mb} MB sein.",
  },
  "look.veil.title": { en: "Veil", sk: "Závoj", de: "Schleier" },
  "look.veil.detail": {
    en: "How much of the page colour sits over the picture. Thinner shows more picture; thicker keeps text easy to read.",
    sk: "Koľko farby stránky leží nad obrázkom. Tenší ukáže viac obrázka; hustejší udrží text čitateľný.",
    de: "Wie viel Seitenfarbe über dem Bild liegt. Dünner zeigt mehr Bild; dichter hält den Text gut lesbar.",
  },
  "look.veil.label": {
    en: "Veil, {percent}%",
    sk: "Závoj, {percent} %",
    de: "Schleier, {percent} %",
  },

  // ── Toasts ──────────────────────────────────────────────────────────────
  "look.toast.pictureSet": { en: "Background set.", sk: "Pozadie nastavené.", de: "Hintergrund gesetzt." },
  "look.toast.pictureRemoved": {
    en: "Background removed.",
    sk: "Pozadie odstránené.",
    de: "Hintergrund entfernt.",
  },
  "look.toast.veilSaved": { en: "Veil saved.", sk: "Závoj uložený.", de: "Schleier gespeichert." },
  "look.toast.notSaved": {
    en: "That could not be saved.",
    sk: "To sa nepodarilo uložiť.",
    de: "Das konnte nicht gespeichert werden.",
  },
  "look.toast.badType": {
    en: "That is not a picture this box takes. Use a PNG, JPEG, GIF or WebP.",
    sk: "Toto nie je obrázok, ktorý zariadenie prijme. Použite PNG, JPEG, GIF alebo WebP.",
    de: "Das ist kein Bild, das diese Box annimmt. Nimm ein PNG, JPEG, GIF oder WebP.",
  },
  "look.toast.tooBig": {
    en: "That picture is larger than {mb} MB.",
    sk: "Tento obrázok je väčší ako {mb} MB.",
    de: "Dieses Bild ist größer als {mb} MB.",
  },

  // ── Widgets on the pane ─────────────────────────────────────────────────
  "look.widgets.group": {
    en: "Widgets written by hand",
    sk: "Widgety napísané ručne",
    de: "Von Hand geschriebene Widgets",
  },
  "look.widgets.empty": {
    en: "None yet. Write one here, or from the gallery on Overview.",
    sk: "Zatiaľ žiadne. Napíšte si ho tu alebo z galérie na Prehľade.",
    de: "Noch keins. Schreib eins hier, oder aus der Galerie in der Übersicht.",
  },
  "look.widgets.write": { en: "Write one", sk: "Napísať", de: "Eins schreiben" },
  "look.widgets.edit": { en: "Edit {name}", sk: "Upraviť {name}", de: "{name} bearbeiten" },
  "look.widgets.delete": { en: "Delete {name}", sk: "Odstrániť {name}", de: "{name} löschen" },
  "look.widgets.deleted": { en: "{name} deleted.", sk: "{name}: odstránené.", de: "{name} gelöscht." },
  "look.widgets.notDeleted": {
    en: "{name} could not be deleted.",
    sk: "{name} sa nepodarilo odstrániť.",
    de: "{name} konnte nicht gelöscht werden.",
  },
  "look.widgets.half": { en: "Half width", sk: "Polovičná šírka", de: "Halbe Breite" },
  "look.widgets.full": { en: "Full width", sk: "Plná šírka", de: "Volle Breite" },
  "look.widgets.caption": {
    en: "A widget written here is kept on this box and offered in the gallery on Overview, where you put it on the board. It runs in a sandbox of its own: it can draw, ask the box for its readings and fetch from the internet, and it cannot reach this page, your sign-in or the box's settings.",
    sk: "Widget napísaný tu zostáva na tomto zariadení a ponúka sa v galérii na Prehľade, kde si ho dáte na nástenku. Beží vo vlastnom sandboxe: môže kresliť, pýtať si od zariadenia jeho údaje a sťahovať z internetu, no nedosiahne na túto stránku, vaše prihlásenie ani nastavenia zariadenia.",
    de: "Ein hier geschriebenes Widget bleibt auf dieser Box und wird in der Galerie der Übersicht angeboten, wo du es auf die Pinnwand legst. Es läuft in einer eigenen Sandbox: es kann zeichnen, die Box nach ihren Messwerten fragen und aus dem Internet laden, aber es erreicht weder diese Seite, noch deine Anmeldung, noch die Einstellungen der Box.",
  },
  "look.widgets.count": {
    en: { one: "{count} of {max}", other: "{count} of {max}" },
    sk: { one: "{count} z {max}", few: "{count} z {max}", many: "{count} z {max}", other: "{count} z {max}" },
    de: { one: "{count} von {max}", other: "{count} von {max}" },
  },
  "look.notLoaded": {
    en: "The look could not be read from this box: {message}",
    sk: "Vzhľad sa nepodarilo načítať zo zariadenia: {message}",
    de: "Das Aussehen konnte nicht von dieser Box gelesen werden: {message}",
  },
  "look.tryAgain": { en: "Try again", sk: "Skúsiť znova", de: "Noch einmal" },

  // ── The editor ──────────────────────────────────────────────────────────
  "look.editor.title": { en: "Write a widget", sk: "Napísať widget", de: "Widget schreiben" },
  "look.editor.editTitle": { en: "Edit {name}", sk: "Upraviť {name}", de: "{name} bearbeiten" },
  "look.editor.description": {
    en: "HTML with its own style and script, in one file or a few. It is kept on this box and drawn in a sandbox of its own; the preview is drawn exactly as the board will draw it.",
    sk: "HTML s vlastným štýlom a skriptom, v jednom súbore alebo v niekoľkých. Zostáva na tomto zariadení a kreslí sa vo vlastnom sandboxe; náhľad je presne taký, ako ho vykreslí nástenka.",
    de: "HTML mit eigenem Style und Script, in einer Datei oder in mehreren. Es bleibt auf dieser Box und wird in einer eigenen Sandbox gezeichnet; die Vorschau ist genau das, was die Pinnwand zeigen wird.",
  },
  "look.editor.tab.write": { en: "Write", sk: "Písať", de: "Schreiben" },
  "look.editor.tab.help": { en: "How it works", sk: "Ako to funguje", de: "So funktioniert es" },
  "look.editor.tab.claude": { en: "Build with Claude", sk: "Vytvoriť s Claude", de: "Mit Claude bauen" },

  // ── Built by Claude (widgets/builder-panel.tsx) ─────────────────────────
  "look.builder.intro": {
    en: "Describe the widget and Claude writes it. It lands in the editor, where you can read it, try it in the preview and change it before you save.",
    sk: "Opíšte widget a Claude ho napíše. Objaví sa v editore, kde si ho môžete prečítať, vyskúšať v náhľade a upraviť ešte pred uložením.",
    de: "Beschreiben Sie das Widget, und Claude schreibt es. Es landet im Editor, wo Sie es lesen, in der Vorschau ausprobieren und vor dem Speichern ändern können.",
  },
  "look.builder.loading": { en: "Asking the edge…", sk: "Pýtam sa edge…", de: "Edge wird gefragt …" },
  "look.builder.loadFailed": {
    en: "Could not reach the widget builder on the edge.",
    sk: "Tvorca widgetov na edge nie je dostupný.",
    de: "Der Widget-Baukasten auf dem Edge ist nicht erreichbar.",
  },
  "look.builder.retry": { en: "Try again", sk: "Skúsiť znova", de: "Erneut versuchen" },
  "look.builder.unavailable": {
    en: "The edge this box uses does not offer the widget builder.",
    sk: "Edge, ktorý tento box používa, tvorcu widgetov neponúka.",
    de: "Der Edge dieser Box bietet den Widget-Baukasten nicht an.",
  },
  "look.builder.noOfficialEdge": {
    en: "The widget builder needs an official LosOS edge in reach, and there is none.",
    sk: "Tvorca widgetov potrebuje dostupný oficiálny edge LosOS, a žiadny tu nie je.",
    de: "Der Widget-Baukasten braucht einen erreichbaren offiziellen LosOS-Edge, und es gibt keinen.",
  },
  "look.builder.price": {
    en: "Claude {model}: {input} per million tokens read and {output} per million written. That is Anthropic's price plus {markup}%. One build costs at most {max}.",
    sk: "Claude {model}: {input} za milión prečítaných tokenov a {output} za milión napísaných. To je cena Anthropicu plus {markup} %. Jedna tvorba stojí najviac {max}.",
    de: "Claude {model}: {input} pro Million gelesener Tokens und {output} pro Million geschriebener. Das ist der Preis von Anthropic plus {markup} %. Ein Bau kostet höchstens {max}.",
  },
  "look.builder.balance": { en: "Balance: {amount}", sk: "Zostatok: {amount}", de: "Guthaben: {amount}" },
  "look.builder.topUp": { en: "Add {amount}", sk: "Dobiť {amount}", de: "{amount} aufladen" },
  "look.builder.topUpNote": {
    en: "Pay in the new tab. The balance changes once Stripe confirms the payment.",
    sk: "Zaplaťte v novej karte. Zostatok sa zmení, keď Stripe platbu potvrdí.",
    de: "Bezahlen Sie im neuen Tab. Das Guthaben ändert sich, sobald Stripe die Zahlung bestätigt.",
  },
  "look.builder.refresh": { en: "Refresh", sk: "Obnoviť", de: "Aktualisieren" },
  "look.builder.popupBlocked": {
    en: "The browser blocked the payment tab. Allow pop-ups for this page and try again.",
    sk: "Prehliadač zablokoval kartu s platbou. Povoľte pre túto stránku vyskakovacie okná a skúste znova.",
    de: "Der Browser hat den Zahlungs-Tab blockiert. Erlauben Sie Pop-ups für diese Seite und versuchen Sie es erneut.",
  },
  "look.builder.noCheckout": {
    en: "The edge did not return a Stripe payment page.",
    sk: "Edge nevrátil platobnú stránku Stripe.",
    de: "Der Edge hat keine Stripe-Zahlungsseite zurückgegeben.",
  },
  "look.builder.field.prompt": {
    en: "What should the widget show?",
    sk: "Čo má widget zobrazovať?",
    de: "Was soll das Widget zeigen?",
  },
  "look.builder.promptHint": {
    en: "For example: free disk space as a ring, green below 70% and red above 90%.",
    sk: "Napríklad: voľné miesto na disku ako kruh, zelený pod 70 % a červený nad 90 %.",
    de: "Zum Beispiel: freier Speicher als Ring, grün unter 70 % und rot über 90 %.",
  },
  "look.builder.change": {
    en: "Change the widget in the editor, all its files, instead of starting over",
    sk: "Upraviť widget v editore so všetkými súbormi namiesto začatia odznova",
    de: "Das Widget im Editor mit allen Dateien ändern, statt neu anzufangen",
  },
  "look.builder.build": { en: "Build", sk: "Vytvoriť", de: "Bauen" },
  "look.builder.needBalance": {
    en: "Add credit to build.",
    sk: "Na tvorbu dobite kredit.",
    de: "Laden Sie Guthaben auf, um zu bauen.",
  },
  "look.builder.runningShort": { en: "Building", sk: "Tvorí sa", de: "Wird gebaut" },
  "look.builder.running": {
    en: "Claude is writing the widget. This takes a minute or two, and it carries on if you close this window.",
    sk: "Claude píše widget. Trvá to minútu či dve a pokračuje aj vtedy, keď toto okno zavriete.",
    de: "Claude schreibt das Widget. Das dauert ein, zwei Minuten und läuft weiter, wenn Sie dieses Fenster schließen.",
  },
  "look.builder.startFailed": {
    en: "The build did not start.",
    sk: "Tvorba sa nespustila.",
    de: "Der Bau hat nicht begonnen.",
  },
  "look.builder.problem.prompt": {
    en: "Describe the widget first.",
    sk: "Najprv widget opíšte.",
    de: "Beschreiben Sie zuerst das Widget.",
  },
  "look.builder.problem.promptLong": {
    en: "Keep the description under {max} characters.",
    sk: "Popis môže mať najviac {max} znakov.",
    de: "Die Beschreibung darf höchstens {max} Zeichen lang sein.",
  },
  "look.builder.doneTitle": {
    en: "Claude wrote the widget",
    sk: "Claude napísal widget",
    de: "Claude hat das Widget geschrieben",
  },
  "look.builder.doneNote": {
    en: "It is in the editor. Read it before you save it.",
    sk: "Je v editore. Pred uložením si ho prečítajte.",
    de: "Es ist im Editor. Lesen Sie es, bevor Sie es speichern.",
  },
  "look.builder.done": {
    en: "Written and put in the editor. Charged {amount}.",
    sk: "Napísané a vložené do editora. Účtované {amount}.",
    de: "Geschrieben und in den Editor gelegt. Berechnet: {amount}.",
  },
  "look.builder.atLimit": {
    en: "The build reached its spending cap, so the widget may be unfinished.",
    sk: "Tvorba dosiahla limit výdavkov, takže widget môže byť nedokončený.",
    de: "Der Bau hat seine Ausgabengrenze erreicht, das Widget ist daher vielleicht unfertig.",
  },
  "look.builder.fault.noWidget": {
    en: "Claude finished without writing a widget. Charged {amount}. Try describing it differently.",
    sk: "Claude skončil bez widgetu. Účtované {amount}. Skúste ho opísať inak.",
    de: "Claude hat ohne Widget aufgehört. Berechnet: {amount}. Beschreiben Sie es anders.",
  },
  "look.builder.fault.unusable": {
    en: "Claude wrote something too large to be a widget. Charged {amount}.",
    sk: "Claude napísal niečo príliš veľké na widget. Účtované {amount}.",
    de: "Claude hat etwas geschrieben, das für ein Widget zu groß ist. Berechnet: {amount}.",
  },
  "look.builder.fault.upstream": {
    en: "The build could not be finished at Anthropic. Nothing was charged.",
    sk: "Tvorbu sa v Anthropicu nepodarilo dokončiť. Nič sa neúčtovalo.",
    de: "Der Bau konnte bei Anthropic nicht abgeschlossen werden. Es wurde nichts berechnet.",
  },
  "look.editor.field.name": { en: "Name", sk: "Názov", de: "Name" },
  "look.editor.field.span": { en: "Width", sk: "Šírka", de: "Breite" },
  "look.editor.field.source": { en: "Source", sk: "Zdroj", de: "Quelltext" },
  "look.editor.field.files": { en: "Files", sk: "Súbory", de: "Dateien" },
  "look.editor.fileLabel": {
    en: "Source of {name}",
    sk: "Zdroj súboru {name}",
    de: "Quelltext von {name}",
  },
  "look.editor.addFile": { en: "Add a file", sk: "Pridať súbor", de: "Datei hinzufügen" },
  "look.editor.newFile": { en: "New file name", sk: "Názov nového súboru", de: "Name der neuen Datei" },
  "look.editor.newFileAdd": { en: "Add", sk: "Pridať", de: "Hinzufügen" },
  "look.editor.removeFile": { en: "Remove {name}", sk: "Odstrániť {name}", de: "{name} entfernen" },
  "look.editor.completeHint": {
    en: "Suggestions appear as you type; Ctrl+Space asks for them. Esc and then Tab leaves the editor.",
    sk: "Návrhy sa zobrazujú počas písania; Ctrl+Medzerník ich vyvolá. Esc a potom Tab opustí editor.",
    de: "Vorschläge erscheinen beim Tippen; Strg+Leertaste ruft sie auf. Esc und dann Tab verlässt den Editor.",
  },
  "look.editor.pasted.title": {
    en: "Pasting something from the internet? Read it first.",
    sk: "Vkladáte niečo z internetu? Najprv si to prečítajte.",
    de: "Etwas aus dem Internet eingefügt? Erst lesen.",
  },
  "look.editor.pasted.body": {
    en: "A widget runs its script on every browser that opens this box. Keep only code you understand; anything that asks you to paste a key, a password or a link you do not recognise does not belong here.",
    sk: "Widget spúšťa svoj skript v každom prehliadači, ktorý otvorí tento box. Nechajte si len kód, ktorému rozumiete; čokoľvek, čo od vás žiada vložiť kľúč, heslo alebo odkaz, ktorý nepoznáte, sem nepatrí.",
    de: "Ein Widget führt sein Skript in jedem Browser aus, der diese Box öffnet. Behalte nur Code, den du verstehst; alles, was dich auffordert, einen Schlüssel, ein Passwort oder einen unbekannten Link einzufügen, gehört nicht hierher.",
  },
  "look.editor.sourceHint": {
    en: "{used} of {max} KiB, all files together",
    sk: "{used} z {max} KiB, všetky súbory spolu",
    de: "{used} von {max} KiB, alle Dateien zusammen",
  },
  "look.editor.preview": { en: "Preview", sk: "Náhľad", de: "Vorschau" },
  "look.editor.save": { en: "Save", sk: "Uložiť", de: "Speichern" },
  "look.editor.saveAndAdd": {
    en: "Save and put on the board",
    sk: "Uložiť a dať na nástenku",
    de: "Speichern und auf die Pinnwand",
  },
  "look.editor.saving": { en: "Saving…", sk: "Ukladá sa…", de: "Wird gespeichert …" },
  "look.editor.cancel": { en: "Cancel", sk: "Zrušiť", de: "Abbrechen" },
  "look.editor.problem.name": { en: "Give it a name.", sk: "Dajte mu názov.", de: "Gib ihm einen Namen." },
  "look.editor.problem.nameLong": {
    en: "The name is longer than {max} characters.",
    sk: "Názov je dlhší ako {max} znakov.",
    de: "Der Name ist länger als {max} Zeichen.",
  },
  "look.editor.problem.source": {
    en: "Write something in index.html first.",
    sk: "Najprv niečo napíšte do index.html.",
    de: "Schreib zuerst etwas in index.html.",
  },
  "look.editor.problem.sourceLong": {
    en: "The files are larger than {kb} KiB together.",
    sk: "Súbory majú spolu viac ako {kb} KiB.",
    de: "Die Dateien sind zusammen größer als {kb} KiB.",
  },
  "look.editor.problem.fileName": {
    en: "Use lowercase letters, digits, - and _, ending in one of: {kinds}.",
    sk: "Použite malé písmená, číslice, - a _, s príponou jednou z: {kinds}.",
    de: "Nimm Kleinbuchstaben, Ziffern, - und _, mit einer dieser Endungen: {kinds}.",
  },
  "look.editor.problem.fileTaken": {
    en: "There is already a file called {name}.",
    sk: "Súbor s názvom {name} už existuje.",
    de: "Es gibt schon eine Datei namens {name}.",
  },
  "look.editor.problem.tooManyFiles": {
    en: "A widget has at most {max} files.",
    sk: "Widget môže mať najviac {max} súborov.",
    de: "Ein Widget hat höchstens {max} Dateien.",
  },
  "look.editor.saved": { en: "{name} saved.", sk: "{name}: uložené.", de: "{name} gespeichert." },
  "look.editor.notSaved": {
    en: "The widget could not be saved.",
    sk: "Widget sa nepodarilo uložiť.",
    de: "Das Widget konnte nicht gespeichert werden.",
  },
  "look.editor.templateFoot": { en: "free on this box", sk: "voľné na tomto zariadení", de: "frei auf dieser Box" },
  "look.editor.help.sandbox": {
    en: "Your widget is a page of its own inside the tile. Write HTML, a <style> and a <script> as you would anywhere; the box's colours are there as CSS variables such as var(--ink), var(--muted), var(--accent) and var(--surface), and they follow the light and dark theme.",
    sk: "Váš widget je vlastná stránka vnútri dlaždice. Píšte HTML, <style> a <script> ako kdekoľvek inde; farby zariadenia sú k dispozícii ako premenné CSS, napríklad var(--ink), var(--muted), var(--accent) a var(--surface), a sledujú svetlú aj tmavú tému.",
    de: "Dein Widget ist eine eigene Seite in der Kachel. Schreib HTML, ein <style> und ein <script> wie überall sonst; die Farben der Box stehen als CSS-Variablen bereit, etwa var(--ink), var(--muted), var(--accent) und var(--surface), und folgen dem hellen und dunklen Thema.",
  },
  "look.editor.help.api": {
    en: "The box's readings come from losos.metric(name), which returns a promise. The names are {names}. losos.theme is \"light\" or \"dark\", losos.lang the language on screen, and losos.onTheme(fn) calls fn when the theme changes. The tile grows to fit what you draw.",
    sk: "Údaje zariadenia získate cez losos.metric(názov), ktorý vracia promise. Názvy sú {names}. losos.theme je „light“ alebo „dark“, losos.lang je jazyk na obrazovke a losos.onTheme(fn) zavolá fn pri zmene témy. Dlaždica sa prispôsobí tomu, čo nakreslíte.",
    de: "Die Messwerte der Box liefert losos.metric(name), das ein Promise zurückgibt. Die Namen sind {names}. losos.theme ist „light“ oder „dark“, losos.lang die Sprache auf dem Bildschirm, und losos.onTheme(fn) ruft fn beim Themenwechsel auf. Die Kachel wächst mit dem, was du zeichnest.",
  },
  "look.editor.help.files": {
    en: "index.html is what the tile shows. Other files are linked from it the usual way: <link rel=\"stylesheet\" href=\"style.css\">, <script src=\"app.js\"></script>, <img src=\"logo.svg\">, url(\"logo.svg\") in a stylesheet, and fetch(\"data.json\") from a script. Scripts share one page, so load them in order with several <script src> tags; import between files does not work.",
    sk: "index.html je to, čo dlaždica zobrazí. Ostatné súbory sa z neho odkazujú obvyklým spôsobom: <link rel=\"stylesheet\" href=\"style.css\">, <script src=\"app.js\"></script>, <img src=\"logo.svg\">, url(\"logo.svg\") v štýle a fetch(\"data.json\") zo skriptu. Skripty zdieľajú jednu stránku, preto ich načítajte v poradí viacerými značkami <script src>; import medzi súbormi nefunguje.",
    de: "index.html ist das, was die Kachel zeigt. Andere Dateien werden von dort wie gewohnt verlinkt: <link rel=\"stylesheet\" href=\"style.css\">, <script src=\"app.js\"></script>, <img src=\"logo.svg\">, url(\"logo.svg\") in einem Stylesheet und fetch(\"data.json\") aus einem Script. Scripts teilen sich eine Seite, also lade sie der Reihe nach mit mehreren <script src>-Tags; import zwischen Dateien funktioniert nicht.",
  },
  "look.editor.help.limits": {
    en: "The widget runs in a sandbox: it cannot see this page, your sign-in, or the box's settings, and it cannot change anything on the box. It may fetch from the internet. This box keeps up to {widgets} widgets, each with up to {files} files and {kb} KiB.",
    sk: "Widget beží v sandboxe: nevidí túto stránku, vaše prihlásenie ani nastavenia zariadenia a nemôže na zariadení nič zmeniť. Môže sťahovať z internetu. Zariadenie uchová až {widgets} widgetov, každý najviac s {files} súbormi a {kb} KiB.",
    de: "Das Widget läuft in einer Sandbox: es sieht weder diese Seite, noch deine Anmeldung, noch die Einstellungen der Box, und es kann nichts an der Box ändern. Aus dem Internet laden darf es. Diese Box behält bis zu {widgets} Widgets mit je bis zu {files} Dateien und {kb} KiB.",
  },
  "look.editor.help.example": { en: "An example", sk: "Príklad", de: "Ein Beispiel" },

  // ── The frame, on a tile ────────────────────────────────────────────────
  "look.frame.title": { en: "{name} (widget)", sk: "{name} (widget)", de: "{name} (Widget)" },
  "look.frame.gone": {
    en: "This widget is no longer on this box.",
    sk: "Tento widget už na zariadení nie je.",
    de: "Dieses Widget ist nicht mehr auf dieser Box.",
  },
  "look.frame.threw": {
    en: "The widget stopped: {message}",
    sk: "Widget sa zastavil: {message}",
    de: "Das Widget ist stehengeblieben: {message}",
  },
  "look.frame.waiting": {
    en: "Reading the box's look…",
    sk: "Načítava sa vzhľad zariadenia…",
    de: "Das Aussehen der Box wird gelesen …",
  },

  // ── The gallery ─────────────────────────────────────────────────────────
  "look.gallery.section": {
    en: "Written by hand on this box",
    sk: "Napísané ručne na tomto zariadení",
    de: "Von Hand geschrieben, auf dieser Box",
  },
  "look.gallery.hand": { en: "By hand", sk: "Ručne", de: "Von Hand" },
  "look.gallery.handBlurb": {
    en: "Write a widget as HTML, style and script of your own. It runs in a sandbox, can ask the box for its readings, and is kept on the box for every browser.",
    sk: "Napíšte widget ako vlastné HTML, štýl a skript. Beží v sandboxe, môže si od zariadenia pýtať údaje a zostáva na zariadení pre každý prehliadač.",
    de: "Schreib ein Widget als eigenes HTML, Style und Script. Es läuft in einer Sandbox, kann die Box nach Messwerten fragen und bleibt auf der Box, für jeden Browser.",
  },
  "look.gallery.writeOne": { en: "Write one", sk: "Napísať", de: "Eins schreiben" },
  "look.gallery.handName": { en: "{name}, by hand", sk: "{name}, ručne", de: "{name}, von Hand" },
});
