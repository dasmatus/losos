import { defineMessages } from "./define";

/* "Report" on a Chat or Mail message (components/report-dialog.tsx), and
 * the handbook page it links, manual/pictures. */
export default defineMessages({
  "report.action": { en: "Report", sk: "Nahlásiť", de: "Melden" },
  "report.title": { en: "Report this message", sk: "Nahlásiť túto správu", de: "Diese Nachricht melden" },
  "report.intro": {
    en: "Some things belong with the police. Everything else can go to the people who look after this box.",
    sk: "Niektoré veci patria polícii. Všetko ostatné môže ísť ľuďom, ktorí sa starajú o tento box.",
    de: "Manches gehört zur Polizei. Alles andere kann an die Leute gehen, die sich um diese Box kümmern.",
  },
  "report.child.title": {
    en: "A sexual picture of a child",
    sk: "Sexuálny obrázok dieťaťa",
    de: "Ein sexuelles Bild eines Kindes",
  },
  "report.child.dont": {
    en: "Do not forward it, screenshot it, download it or save it to LosOS Cloud. Keeping a copy can be a crime even when you mean well. Write down who sent it and when.",
    sk: "Nepreposielajte ho, nerobte snímku obrazovky, nesťahujte ho ani ho neukladajte do LosOS Cloud. Uchovanie kópie môže byť trestné, aj keď to myslíte dobre. Zapíšte si, kto ho poslal a kedy.",
    de: "Leite es nicht weiter, mach keinen Screenshot, lade es nicht herunter und speichere es nicht in LosOS Cloud. Eine Kopie zu behalten kann strafbar sein, auch wenn du es gut meinst. Schreib dir auf, wer es wann geschickt hat.",
  },
  "report.child.danger": {
    en: "If a child is in danger now, call 112 in the EU, or your local emergency number.",
    sk: "Ak je dieťa práve v nebezpečenstve, volajte 112 alebo políciu na 158.",
    de: "Wenn ein Kind gerade in Gefahr ist, ruf die 110 oder die 112 an.",
  },
  "report.child.hotline": {
    en: "Report it to the police or to your country's hotline. INHOPE (inhope.org) lists the hotline for each country.",
    sk: "Nahláste to polícii alebo na Stopline.sk. Zoznam liniek pre ostatné krajiny je na inhope.org.",
    de: "Melde es der Polizei oder bei internet-beschwerdestelle.de oder jugendschutz.net. Meldestellen anderer Länder findest du auf inhope.org.",
  },
  "report.child.helpline": {
    en: "Children and teenagers can talk to someone for free on 116 111.",
    sk: "Deti a mladí sa môžu bezplatne s niekým porozprávať na čísle 116 111.",
    de: "Kinder und Jugendliche können kostenlos unter 116 111 mit jemandem sprechen.",
  },
  "report.child.where": {
    en: "Where to report, country by country",
    sk: "Kde nahlasovať, podľa krajiny",
    de: "Wo melden, nach Land",
  },
  "report.box.title": {
    en: "Tell this box's moderators",
    sk: "Povedať moderátorom tohto boxu",
    de: "Den Moderatoren dieser Box Bescheid geben",
  },
  "report.box.detail": {
    en: "They get a reference to the message, never its words or pictures, and can hide it or stop the sender.",
    sk: "Dostanú odkaz na správu, nikdy jej text ani obrázky, a môžu ju skryť alebo odosielateľa zastaviť.",
    de: "Sie bekommen einen Verweis auf die Nachricht, nie ihren Text oder ihre Bilder, und können sie verbergen oder den Absender stoppen.",
  },
  "report.box.reason": { en: "What is wrong", sk: "Čo je zle", de: "Was ist los" },
  "report.box.note": {
    en: "Anything they should know (optional)",
    sk: "Čokoľvek, čo by mali vedieť (nepovinné)",
    de: "Was sie wissen sollten (freiwillig)",
  },
  "report.box.none": {
    en: "This box has no moderators to report to yet. The links above still work.",
    sk: "Tento box zatiaľ nemá moderátorov, ktorým by sa dalo hlásiť. Odkazy vyššie fungujú.",
    de: "Diese Box hat noch keine Moderatoren für Meldungen. Die Links oben funktionieren trotzdem.",
  },
  "report.reason.child": {
    en: "A sexual picture of a child",
    sk: "Sexuálny obrázok dieťaťa",
    de: "Ein sexuelles Bild eines Kindes",
  },
  "report.reason.sexual": {
    en: "Unwanted sexual content",
    sk: "Nevyžiadaný sexuálny obsah",
    de: "Unerwünschte sexuelle Inhalte",
  },
  "report.reason.abuse": {
    en: "Threats or harassment",
    sk: "Vyhrážky alebo obťažovanie",
    de: "Drohungen oder Belästigung",
  },
  "report.reason.spam": { en: "Spam", sk: "Spam", de: "Spam" },
  "report.reason.other": { en: "Something else", sk: "Niečo iné", de: "Etwas anderes" },
  "report.send": { en: "Send to moderators", sk: "Poslať moderátorom", de: "An Moderatoren senden" },
  "report.close": { en: "Close", sk: "Zavrieť", de: "Schließen" },
  "report.sent": {
    en: "The moderators have your report",
    sk: "Moderátori dostali vaše hlásenie",
    de: "Die Moderatoren haben deine Meldung",
  },
  "report.failed": {
    en: "The report did not go",
    sk: "Hlásenie sa neodoslalo",
    de: "Die Meldung ging nicht raus",
  },
});
