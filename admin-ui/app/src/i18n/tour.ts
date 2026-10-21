import { defineMessages } from "./define";

/* The first-run tour (src/tour/), shown once after setup and again from
 * the About pane. One title and one sentence per step: the scene above
 * them does the showing. The chat step is drawn only on a box that shares
 * its storage in a mesh. */
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
    en: "LosOS Cloud brings Files, Calendar, Contacts, Tasks, Notes and a Board, all on the Overview.",
    sk: "LosOS Cloud prináša Súbory, Kalendár, Kontakty, Úlohy, Poznámky a Tabuľu, všetko v Prehľade.",
    de: "LosOS Cloud bringt Dateien, Kalender, Kontakte, Aufgaben, Notizen und ein Board mit, alle in der Übersicht.",
  },
  "tour.together.title": {
    en: "Write together",
    sk: "Píšte spolu",
    de: "Gemeinsam schreiben",
  },
  "tour.together.body": {
    en: "Open a document in Office and edit it with others at the same time. Each of you sees where the others are typing.",
    sk: "Otvorte dokument v Office a upravujte ho s ďalšími naraz. Každý vidí, kde píšu ostatní.",
    de: "Öffne ein Dokument in Office und bearbeite es gleichzeitig mit anderen. Ihr seht, wo die anderen tippen.",
  },
  "tour.apps.title": { en: "Add more apps", sk: "Pridajte ďalšie aplikácie", de: "Weitere Apps hinzufügen" },
  "tour.apps.body": {
    en: "Search under Apps and install what you find. It runs on this box, next to your files.",
    sk: "Vyhľadajte ich v časti Aplikácie a nainštalujte. Pobežia na tomto zariadení, vedľa vašich súborov.",
    de: "Such unter Apps und installiere, was du findest. Es läuft auf dieser Box, neben deinen Dateien.",
  },
  "tour.byName.title": { en: "Get paid at this box's name", sk: "Platby na názov zariadenia", de: "Bezahlt werden über den Namen der Box" },
  "tour.byName.body": {
    en: "Under Network, bind a wallet to this box's names. Anyone can then pay the box by typing its name into their wallet.",
    sk: "V časti Sieť priraďte k názvom tohto zariadenia peňaženku. Ktokoľvek potom môže zariadeniu zaplatiť tak, že napíše jeho názov do svojej peňaženky.",
    de: "Unter Netzwerk verknüpfst du eine Wallet mit den Namen dieser Box. Dann kann jeder die Box bezahlen, indem er ihren Namen in seine Wallet tippt.",
  },
  "tour.builder.title": {
    en: "Describe a widget, get a widget",
    sk: "Opíšte widget a dostanete ho",
    de: "Beschreib ein Widget, und du bekommst es",
  },
  "tour.builder.body": {
    en: "Under Look, Build with AI writes a widget from one sentence. The AI on this box checks it and fixes what it finds.",
    sk: "V časti Vzhľad napíše Vytvoriť s AI widget z jednej vety. AI na tomto zariadení ho skontroluje a opraví, čo nájde.",
    de: "Unter Aussehen schreibt Mit KI bauen ein Widget aus einem Satz. Die KI auf dieser Box prüft es und behebt, was sie findet.",
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
  "tour.lend.title": { en: "Lend memory to AI models", sk: "Požičajte pamäť modelom AI", de: "Leih KI-Modellen Speicher" },
  "tour.lend.body": {
    en: "Under Market, lend some of this box's memory to AI models that run across boxes. You are paid for every token they write.",
    sk: "V časti Trh požičajte časť pamäte tohto zariadenia modelom AI, ktoré bežia na viacerých zariadeniach. Dostanete zaplatené za každý token, ktorý napíšu.",
    de: "Unter Markt leihst du einen Teil des Arbeitsspeichers dieser Box KI-Modellen, die über mehrere Boxen laufen. Du wirst für jeden Token bezahlt, den sie schreiben.",
  },
  "tour.chat.title": {
    en: "Chat and calls",
    sk: "Chat a hovory",
    de: "Chat und Anrufe",
  },
  "tour.chat.body": {
    en: "While this box shares its storage in the mesh, it has chat with end-to-end encrypted rooms and video calls. Type @ to name someone.",
    sk: "Kým toto zariadenie zdieľa úložisko v mesh, má chat so šifrovanými miestnosťami a videohovory. Napíšte @ a označte niekoho.",
    de: "Solange diese Box ihren Speicher im Mesh teilt, hat sie Chat mit Ende-zu-Ende-verschlüsselten Räumen und Videoanrufe. Tipp @, um jemanden zu nennen.",
  },
  "tour.profile.title": { en: "Put a face to your name", sk: "Pridajte k menu tvár", de: "Gib deinem Namen ein Gesicht" },
  "tour.profile.body": {
    en: "Add a picture and a banner under Your account. People on this box see them next to your name.",
    sk: "V časti Váš účet pridajte fotku a banner. Ľudia na tomto zariadení ich uvidia pri vašom mene.",
    de: "Füge unter Dein Konto ein Bild und ein Banner hinzu. Leute auf dieser Box sehen sie neben deinem Namen.",
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
  "tour.scene.docName": { en: "Plan.docx", sk: "Plán.docx", de: "Plan.docx" },
  "tour.scene.docHeading": { en: "The trip in May", sk: "Výlet v máji", de: "Die Reise im Mai" },
  "tour.scene.typedMine": { en: "We leave on Friday at eight.", sk: "Odchádzame v piatok o ôsmej.", de: "Wir fahren Freitag um acht." },
  "tour.scene.typedHers": { en: "I will book the train.", sk: "Vlak zarezervujem ja.", de: "Ich buche den Zug." },
  "tour.scene.togetherCaption": {
    en: "Office runs on this box. The document never leaves it.",
    sk: "Office beží na tomto zariadení. Dokument ho nikdy neopustí.",
    de: "Office läuft auf dieser Box. Das Dokument verlässt sie nie.",
  },
  "tour.scene.builderOnBox": {
    en: "The AI runs on this box",
    sk: "AI beží na tomto zariadení",
    de: "Die KI läuft auf dieser Box",
  },
  "tour.scene.prompt": {
    en: "A to-do list for the band",
    sk: "Zoznam úloh pre kapelu",
    de: "Eine To-do-Liste für die Band",
  },
  "tour.scene.build": { en: "Build", sk: "Vytvoriť", de: "Bauen" },
  "tour.scene.stepWrite": { en: "Wrote {file}", sk: "Napísal {file}", de: "{file} geschrieben" },
  "tour.scene.stepCheck": { en: "Found one problem", sk: "Našiel jeden problém", de: "Ein Problem gefunden" },
  "tour.scene.stepFix": { en: "Fixed it", sk: "Opravil ho", de: "Behoben" },
  "tour.scene.appTitle": { en: "Band to-dos", sk: "Úlohy kapely", de: "Band-To-dos" },
  "tour.scene.todo1": { en: "Book the hall", sk: "Zarezervovať sálu", de: "Saal buchen" },
  "tour.scene.todo2": { en: "Print the posters", sk: "Vytlačiť plagáty", de: "Plakate drucken" },
  "tour.scene.todo3": { en: "Tune the guitars", sk: "Naladiť gitary", de: "Gitarren stimmen" },
  "tour.scene.builderCaption": {
    en: "The widget runs in its own frame, closed off from the rest of this page.",
    sk: "Widget beží vo vlastnom rámci, oddelený od zvyšku tejto stránky.",
    de: "Das Widget läuft in einem eigenen Rahmen, abgeschottet vom Rest dieser Seite.",
  },
  "tour.scene.thisBoxEarned": { en: "What this box earned", sk: "Čo toto zariadenie zarobilo", de: "Was diese Box verdient hat" },
  "tour.scene.theirWallet": { en: "Someone's wallet", sk: "Niečia peňaženka", de: "Die Wallet von jemandem" },
  "tour.scene.payTo": { en: "Pay to", sk: "Zaplatiť komu", de: "Bezahlen an" },
  "tour.scene.aiModels": { en: "AI models", sk: "Modely AI", de: "KI-Modelle" },
  "tour.scene.price": {
    en: "{in} in, {out} out per million tokens",
    sk: "{in} za vstup, {out} za výstup na milión tokenov",
    de: "{in} rein, {out} raus pro Million Tokens",
  },
  "tour.scene.onBoxes": { en: "On {count} boxes", sk: "Na {count} zariadeniach", de: "Auf {count} Boxen" },
  "tour.scene.tokens": {
    en: { one: "{count} token", other: "{count} tokens" },
    sk: { one: "{count} token", few: "{count} tokeny", many: "{count} tokenu", other: "{count} tokenov" },
    de: { one: "{count} Token", other: "{count} Tokens" },
  },
  "tour.scene.paidByTheToken": { en: "Paid by the token", sk: "Platí sa za token", de: "Pro Token bezahlt" },
  "tour.scene.byName": { en: "Paid by name", sk: "Platby na názov", de: "Bezahlt per Name" },
  "tour.scene.balance": { en: "Balance", sk: "Zostatok", de: "Guthaben" },
  "tour.scene.received": { en: "Payment received", sk: "Platba prijatá", de: "Zahlung erhalten" },
  "tour.scene.toName": { en: "Sent to {name}", sk: "Poslaná na {name}", de: "Gesendet an {name}" },
  "tour.scene.chat": { en: "Chat", sk: "Chat", de: "Chat" },
  "tour.scene.roomBand": { en: "Band", sk: "Kapela", de: "Band" },
  "tour.scene.asked": { en: "Who books the hall?", sk: "Kto zarezervuje sálu?", de: "Wer bucht den Saal?" },
  "tour.scene.draft": { en: "can you?", sk: "zvládneš to?", de: "machst du das?" },
  "tour.scene.videoCall": { en: "Video call", sk: "Videohovor", de: "Videoanruf" },
  "tour.scene.send": { en: "Send", sk: "Poslať", de: "Senden" },
  "tour.scene.changeBanner": { en: "Change banner", sk: "Zmeniť banner", de: "Banner ändern" },
  "tour.scene.changePicture": { en: "Change picture", sk: "Zmeniť fotku", de: "Bild ändern" },
  "tour.scene.profileCaption": {
    en: "Your picture and banner show wherever your name does on this box.",
    sk: "Vaša fotka a banner sa zobrazia všade, kde je na tomto zariadení vaše meno.",
    de: "Dein Bild und Banner erscheinen überall, wo dein Name auf dieser Box steht.",
  },
});
