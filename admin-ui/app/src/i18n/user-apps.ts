import { defineMessages } from "./define";

/* The owner's own apps (src/apps/): the list on the Apps pane, an app's own
 * page, the editor that makes and changes one, and the app builder's
 * sentences (widgets/builder-panel.tsx, kind "app"). The builder's shared
 * sentences (the model, the licence, the faults) are under look.builder. */
export default defineMessages({
  // ── The list on the Apps pane ───────────────────────────────────────────
  "userApps.title": { en: "Your apps", sk: "Vaše aplikácie", de: "Deine Apps" },
  "userApps.caption": {
    en: "Apps you describe and the AI on this box writes. Each one runs on its own page and keeps its data on the box.",
    sk: "Aplikácie, ktoré opíšete a AI na tomto zariadení napíše. Každá beží na vlastnej stránke a údaje si ukladá na zariadení.",
    de: "Apps, die du beschreibst und die KI auf dieser Box schreibt. Jede läuft auf einer eigenen Seite und speichert ihre Daten auf der Box.",
  },
  "userApps.emptyTitle": { en: "No apps yet", sk: "Zatiaľ žiadne aplikácie", de: "Noch keine Apps" },
  "userApps.empty": {
    en: "Describe one, for example a shopping list or a chore chart, and the AI writes it.",
    sk: "Opíšte nejakú, napríklad nákupný zoznam alebo rozpis domácich prác, a AI ju napíše.",
    de: "Beschreibe eine, zum Beispiel eine Einkaufsliste oder einen Putzplan, und die KI schreibt sie.",
  },
  "userApps.make": { en: "Make an app", sk: "Vytvoriť aplikáciu", de: "App erstellen" },
  "userApps.open": { en: "Open", sk: "Otvoriť", de: "Öffnen" },
  "userApps.change": { en: "Change", sk: "Upraviť", de: "Ändern" },
  "userApps.delete": { en: "Delete", sk: "Odstrániť", de: "Löschen" },
  "userApps.retry": { en: "Try again", sk: "Skúsiť znova", de: "Erneut versuchen" },
  "userApps.listFailed": {
    en: "Could not read your apps from the box.",
    sk: "Vaše aplikácie sa zo zariadenia nepodarilo načítať.",
    de: "Deine Apps konnten nicht von der Box gelesen werden.",
  },
  "userApps.full": {
    en: "This box keeps at most {max} apps. Delete one to make another.",
    sk: "Toto zariadenie uchová najviac {max} aplikácií. Ak chcete vytvoriť ďalšiu, jednu odstráňte.",
    de: "Diese Box behält höchstens {max} Apps. Lösche eine, um eine weitere zu erstellen.",
  },
  "userApps.deleteTitle": {
    en: "Delete {name}?",
    sk: "Odstrániť {name}?",
    de: "{name} löschen?",
  },
  "userApps.deleteBody": {
    en: "The app and everything entered into it are deleted from the box. This cannot be undone.",
    sk: "Aplikácia a všetko, čo sa do nej zadalo, sa zo zariadenia odstráni. Nedá sa to vrátiť späť.",
    de: "Die App und alles, was darin eingegeben wurde, werden von der Box gelöscht. Das lässt sich nicht rückgängig machen.",
  },
  "userApps.cancel": { en: "Cancel", sk: "Zrušiť", de: "Abbrechen" },
  "userApps.deleted": { en: "{name} is deleted", sk: "{name} je odstránená", de: "{name} ist gelöscht" },
  "userApps.deleteFailed": {
    en: "The app was not deleted.",
    sk: "Aplikácia sa neodstránila.",
    de: "Die App wurde nicht gelöscht.",
  },

  // ── An app's own page ───────────────────────────────────────────────────
  "userApps.back": { en: "Your apps", sk: "Vaše aplikácie", de: "Deine Apps" },
  "userApps.loading": { en: "Opening the app…", sk: "Otváram aplikáciu…", de: "App wird geöffnet …" },
  "userApps.notFound": {
    en: "There is no such app on this box. It may have been deleted.",
    sk: "Takáto aplikácia na tomto zariadení nie je. Možno ju niekto odstránil.",
    de: "Diese App gibt es auf dieser Box nicht. Vielleicht wurde sie gelöscht.",
  },
  "userApps.loadFailed": {
    en: "Could not open the app.",
    sk: "Aplikáciu sa nepodarilo otvoriť.",
    de: "Die App konnte nicht geöffnet werden.",
  },
  "userApps.crashed": {
    en: "The app ran into a mistake: {message}",
    sk: "Aplikácia narazila na chybu: {message}",
    de: "Die App ist auf einen Fehler gestoßen: {message}",
  },
  "userApps.saveError": {
    en: "The box did not keep that: {message}",
    sk: "Zariadenie to neuložilo: {message}",
    de: "Die Box hat das nicht gespeichert: {message}",
  },

  // ── The editor ──────────────────────────────────────────────────────────
  "userApps.editor.newTitle": { en: "Make an app", sk: "Vytvoriť aplikáciu", de: "App erstellen" },
  "userApps.editor.editTitle": { en: "Change {name}", sk: "Upraviť {name}", de: "{name} ändern" },
  "userApps.editor.tab.ai": { en: "Build with AI", sk: "Vytvoriť s AI", de: "Mit KI bauen" },
  "userApps.editor.tab.code": { en: "Code", sk: "Kód", de: "Code" },
  "userApps.editor.field.name": { en: "Name", sk: "Názov", de: "Name" },
  "userApps.editor.field.summary": { en: "What it does", sk: "Čo robí", de: "Was sie tut" },
  "userApps.editor.codeLabel": {
    en: "Source of the app",
    sk: "Zdrojový kód aplikácie",
    de: "Quelltext der App",
  },
  "userApps.editor.codeHint": {
    en: "One React component called App, built from the LosOS components. The box checks it as you type.",
    sk: "Jeden komponent Reactu s názvom App, zložený z komponentov LosOS. Zariadenie ho kontroluje počas písania.",
    de: "Eine React-Komponente namens App aus den LosOS-Komponenten. Die Box prüft sie beim Tippen.",
  },
  "userApps.editor.preview": { en: "Preview", sk: "Náhľad", de: "Vorschau" },
  "userApps.editor.previewEmpty": {
    en: "The app shows here once it is written.",
    sk: "Aplikácia sa tu zobrazí, keď bude napísaná.",
    de: "Die App erscheint hier, sobald sie geschrieben ist.",
  },
  "userApps.editor.previewNote": {
    en: "The preview forgets what you enter in it. The saved app keeps its data on the box.",
    sk: "Náhľad zabudne, čo doň zadáte. Uložená aplikácia si údaje uchová na zariadení.",
    de: "Die Vorschau vergisst, was du darin eingibst. Die gespeicherte App behält ihre Daten auf der Box.",
  },
  "userApps.editor.problems": {
    en: "The box found mistakes in the code",
    sk: "Zariadenie našlo v kóde chyby",
    de: "Die Box hat Fehler im Code gefunden",
  },
  "userApps.editor.askFix": {
    en: "Ask the AI to fix it",
    sk: "Požiadať AI o opravu",
    de: "Die KI um eine Korrektur bitten",
  },
  "userApps.editor.fixPrompt": {
    en: "Fix this mistake: {problem}",
    sk: "Oprav túto chybu: {problem}",
    de: "Behebe diesen Fehler: {problem}",
  },
  "userApps.editor.save": { en: "Save", sk: "Uložiť", de: "Speichern" },
  "userApps.editor.cancel": { en: "Cancel", sk: "Zrušiť", de: "Abbrechen" },
  "userApps.editor.saved": { en: "{name} is saved", sk: "{name} je uložená", de: "{name} ist gespeichert" },
  "userApps.editor.savedNote": {
    en: "It is under Your apps on the Apps page.",
    sk: "Nájdete ju v časti Vaše aplikácie na stránke Aplikácie.",
    de: "Sie steht unter Deine Apps auf der Seite Apps.",
  },
  "userApps.editor.notSaved": {
    en: "The app was not saved.",
    sk: "Aplikácia sa neuložila.",
    de: "Die App wurde nicht gespeichert.",
  },
  "userApps.editor.needName": {
    en: "Give the app a name.",
    sk: "Pomenujte aplikáciu.",
    de: "Gib der App einen Namen.",
  },
  "userApps.editor.needCode": {
    en: "Build the app with the AI, or write its code, before you save it.",
    sk: "Pred uložením aplikáciu vytvorte s AI alebo napíšte jej kód.",
    de: "Bau die App mit der KI oder schreib ihren Code, bevor du sie speicherst.",
  },

  // ── The app builder (widgets/builder-panel.tsx, kind "app") ─────────────
  "userApps.builder.local": {
    en: "The AI is {model}, running on this box. What you write here stays on the box, and building is free. An app takes a few minutes, while the box checks it, tries it and has the AI fix what it finds. The model is small and makes mistakes, so try the app in the preview before you save it.",
    sk: "AI je {model} a beží na tomto zariadení. Čo sem napíšete, zostáva na zariadení, a tvorba je zadarmo. Aplikácia trvá niekoľko minút, kým ju zariadenie skontroluje, vyskúša a nechá AI opraviť, čo nájde. Model je malý a robí chyby, preto aplikáciu pred uložením vyskúšajte v náhľade.",
    de: "Die KI ist {model} und läuft auf dieser Box. Was du hier schreibst, bleibt auf der Box, und das Bauen ist kostenlos. Eine App dauert ein paar Minuten, während die Box sie prüft, ausprobiert und die KI korrigieren lässt, was sie findet. Das Modell ist klein und macht Fehler, probiere die App daher vor dem Speichern in der Vorschau aus.",
  },
  "userApps.builder.intro": {
    en: "Describe the app and the AI writes it. Try it in the preview, change what you like, then save it.",
    sk: "Opíšte aplikáciu a AI ju napíše. Vyskúšajte ju v náhľade, upravte, čo chcete, a potom ju uložte.",
    de: "Beschreibe die App, und die KI schreibt sie. Probiere sie in der Vorschau aus, ändere, was du möchtest, und speichere sie dann.",
  },
  "userApps.builder.field.prompt": {
    en: "What should the app do?",
    sk: "Čo má aplikácia robiť?",
    de: "Was soll die App tun?",
  },
  "userApps.builder.promptHint": {
    en: "For example: a shopping list where I tick off what I bought, and a button that clears the ticked ones.",
    sk: "Napríklad: nákupný zoznam, v ktorom si odškrtnem, čo som kúpil, a tlačidlo, ktoré odškrtnuté položky vymaže.",
    de: "Zum Beispiel: eine Einkaufsliste, in der ich abhake, was ich gekauft habe, und eine Schaltfläche, die das Abgehakte löscht.",
  },
  "userApps.builder.change": {
    en: "Change the app in the editor instead of starting over",
    sk: "Upraviť aplikáciu v editore namiesto začatia odznova",
    de: "Die App im Editor ändern, statt neu anzufangen",
  },
  "userApps.builder.writing": {
    en: "The AI is writing the app: {lines} lines so far. It carries on if you leave this page.",
    sk: "AI píše aplikáciu: zatiaľ {lines} riadkov. Pokračuje aj vtedy, keď túto stránku opustíte.",
    de: "Die KI schreibt die App: bisher {lines} Zeilen. Sie macht weiter, wenn du diese Seite verlässt.",
  },
  "userApps.builder.fixing": {
    en: "The box found a mistake in the app and the AI is fixing it: {lines} lines so far.",
    sk: "Zariadenie našlo v aplikácii chybu a AI ju opravuje: zatiaľ {lines} riadkov.",
    de: "Die Box hat einen Fehler in der App gefunden, und die KI behebt ihn: bisher {lines} Zeilen.",
  },
  "userApps.builder.problem.prompt": {
    en: "Describe the app first.",
    sk: "Najprv aplikáciu opíšte.",
    de: "Beschreibe zuerst die App.",
  },
  "userApps.builder.doneTitle": {
    en: "The app is written",
    sk: "Aplikácia je napísaná",
    de: "Die App ist geschrieben",
  },
  "userApps.builder.doneNote": {
    en: "Try it in the preview before you save it.",
    sk: "Pred uložením ju vyskúšajte v náhľade.",
    de: "Probiere sie vor dem Speichern in der Vorschau aus.",
  },
  "userApps.builder.done": {
    en: "Written and put in the preview.",
    sk: "Napísaná a vložená do náhľadu.",
    de: "Geschrieben und in die Vorschau gelegt.",
  },
  "userApps.builder.stopped": {
    en: "Stopped. Nothing was changed.",
    sk: "Zastavené. Nič sa nezmenilo.",
    de: "Angehalten. Es wurde nichts geändert.",
  },
  "userApps.builder.atLimit": {
    en: "The answer reached its length limit, so the app may be unfinished.",
    sk: "Odpoveď dosiahla limit dĺžky, takže aplikácia môže byť nedokončená.",
    de: "Die Antwort hat ihre Längengrenze erreicht, die App ist daher vielleicht unfertig.",
  },
  "userApps.builder.fault.noApp": {
    en: "The AI answered without writing an app. Try describing it differently.",
    sk: "AI odpovedala bez aplikácie. Skúste ju opísať inak.",
    de: "Die KI hat ohne App geantwortet. Beschreibe sie anders.",
  },
});
