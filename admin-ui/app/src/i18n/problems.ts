import { defineMessages } from "./define";

/* The Problems pane (screens/settings/pane-problems.tsx): what the box
 * noticed going wrong, each report readable in full before it leaves the
 * box. The pane's name, summary and search words live under
 * settings.panes.problems.* with the other panes'. */

export default defineMessages({
  "problems.loading": {
    en: "Looking through the box's log…",
    sk: "Prezerám záznam zariadenia…",
    de: "Sehe das Protokoll der Box durch…",
  },
  "problems.unavailable": {
    en: "This box does not watch for problems yet. It will after its next update.",
    sk: "Toto zariadenie zatiaľ nesleduje problémy. Bude po najbližšej aktualizácii.",
    de: "Diese Box achtet noch nicht auf Probleme. Nach ihrem nächsten Update schon.",
  },
  "problems.failed": {
    en: "Could not load the problems.",
    sk: "Problémy sa nepodarilo načítať.",
    de: "Die Probleme konnten nicht geladen werden.",
  },
  "problems.retry": { en: "Try again", sk: "Skúsiť znova", de: "Erneut versuchen" },

  // ── Where reports go ────────────────────────────────────────────────────
  "problems.reports.title": { en: "Reports", sk: "Hlásenia", de: "Berichte" },
  "problems.reports.on": {
    en: "Reports go to {repository} on GitHub",
    sk: "Hlásenia idú do {repository} na GitHube",
    de: "Berichte gehen an {repository} auf GitHub",
  },
  "problems.reports.onDetail": {
    en: "Only when you read one and send it. You sign in to GitHub for each report, so it carries your name.",
    sk: "Iba keď si ho prečítate a odošlete. Pri každom hlásení sa prihlásite na GitHub, takže nesie vaše meno.",
    de: "Nur wenn du einen liest und abschickst. Du meldest dich für jeden Bericht bei GitHub an, also trägt er deinen Namen.",
  },
  "problems.reports.off": {
    en: "Reports stay on this box",
    sk: "Hlásenia zostávajú v tomto zariadení",
    de: "Berichte bleiben auf dieser Box",
  },
  "problems.reports.offDetail": {
    en: "This build has no place to send them. You can still read each one and copy it.",
    sk: "Táto verzia ich nemá kam poslať. Každé si však môžete prečítať a skopírovať.",
    de: "Dieser Build hat keinen Ort, an den er sie schicken kann. Du kannst trotzdem jeden lesen und kopieren.",
  },
  "problems.reports.caption": {
    en: "The box notices services that stop with an error, programs that crash and programs stopped for lack of memory. Nothing leaves the box until you send it.",
    sk: "Zariadenie si všíma služby, ktoré skončia chybou, programy, ktoré spadnú, a programy zastavené pre nedostatok pamäte. Nič neopustí zariadenie, kým to neodošlete.",
    de: "Die Box bemerkt Dienste, die mit einem Fehler enden, Programme, die abstürzen, und Programme, die wegen Speichermangels beendet werden. Nichts verlässt die Box, bevor du es abschickst.",
  },
  "problems.reports.more": { en: "How reports work", sk: "Ako fungujú hlásenia", de: "So funktionieren Berichte" },
  "problems.badge.askFirst": { en: "Asks first", sk: "Najprv sa opýta", de: "Fragt zuerst" },
  "problems.badge.localOnly": { en: "On this box", sk: "V tomto zariadení", de: "Auf dieser Box" },

  // ── The list ────────────────────────────────────────────────────────────
  "problems.list.title": { en: "What went wrong", sk: "Čo sa pokazilo", de: "Was schiefging" },
  "problems.none": { en: "Nothing has gone wrong", sk: "Nič sa nepokazilo", de: "Nichts ist schiefgegangen" },
  "problems.noneDetail": {
    en: "The box looks through its log every minute.",
    sk: "Zariadenie prezerá svoj záznam každú minútu.",
    de: "Die Box sieht ihr Protokoll jede Minute durch.",
  },
  "problems.kind.crashed": {
    en: "{subject} crashed ({signal})",
    sk: "{subject} spadol ({signal})",
    de: "{subject} ist abgestürzt ({signal})",
  },
  "problems.kind.outOfMemory": {
    en: "{subject} was stopped: the box ran out of memory",
    sk: "{subject} bol zastavený: zariadeniu došla pamäť",
    de: "{subject} wurde beendet: der Box ging der Speicher aus",
  },
  "problems.kind.failed": {
    en: "{subject} stopped with an error ({result})",
    sk: "{subject} skončil chybou ({result})",
    de: "{subject} endete mit einem Fehler ({result})",
  },
  "problems.once": { en: "{at}", sk: "{at}", de: "{at}" },
  "problems.times": {
    en: { one: "{count} time, {first} to {last}", other: "{count} times, {first} to {last}" },
    sk: {
      one: "{count}-krát, {first} až {last}",
      few: "{count}-krát, {first} až {last}",
      many: "{count}-krát, {first} až {last}",
      other: "{count}-krát, {first} až {last}",
    },
    de: { one: "{count}-mal, {first} bis {last}", other: "{count}-mal, {first} bis {last}" },
  },
  "problems.read": { en: "Read the report", sk: "Prečítať hlásenie", de: "Bericht lesen" },
  "problems.readAndSend": { en: "Read and send", sk: "Prečítať a odoslať", de: "Lesen und senden" },
  "problems.whatToDo": { en: "What to do", sk: "Čo robiť", de: "Was tun" },
  "problems.dismiss": { en: "Dismiss", sk: "Skryť", de: "Ausblenden" },
  "problems.dismissFailed": {
    en: "Could not dismiss the problem.",
    sk: "Problém sa nepodarilo skryť.",
    de: "Das Problem konnte nicht ausgeblendet werden.",
  },
  "problems.badge.failed": { en: "Error", sk: "Chyba", de: "Fehler" },
  "problems.badge.crashed": { en: "Crash", sk: "Pád", de: "Absturz" },
  "problems.badge.outOfMemory": { en: "Out of memory", sk: "Málo pamäte", de: "Kein Speicher" },
  "problems.badge.documented": {
    en: "In the handbook: {title}",
    sk: "V príručke: {title}",
    de: "Im Handbuch: {title}",
  },
  "problems.sent": { en: "Reported", sk: "Nahlásené", de: "Gemeldet" },
  "problems.sentExisting": {
    en: "Added to an open report",
    sk: "Pridané k otvorenému hláseniu",
    de: "Zu einem offenen Bericht hinzugefügt",
  },
  "problems.dismissed.title": { en: "Dismissed", sk: "Skryté", de: "Ausgeblendet" },
  "problems.dismissed.caption": {
    en: "A dismissed problem comes back to the list if it happens again.",
    sk: "Skrytý problém sa vráti do zoznamu, ak sa zopakuje.",
    de: "Ein ausgeblendetes Problem kommt zurück in die Liste, wenn es wieder passiert.",
  },

  // ── The report ──────────────────────────────────────────────────────────
  "problems.report.title": { en: "Report", sk: "Hlásenie", de: "Bericht" },
  "problems.report.public": {
    en: "This is the whole text {repository} would get, in public. Addresses, the box's name and the names of the people on it are already taken out.",
    sk: "Toto je celý text, ktorý by verejne dostal {repository}. Adresy, názov zariadenia a mená ľudí na ňom sú už odstránené.",
    de: "Das ist der ganze Text, den {repository} öffentlich bekäme. Adressen, der Name der Box und die Namen der Leute darauf sind schon entfernt.",
  },
  "problems.report.localOnly": {
    en: "This is what the box wrote down. Addresses, the box's name and the names of the people on it are taken out.",
    sk: "Toto si zariadenie zapísalo. Adresy, názov zariadenia a mená ľudí na ňom sú odstránené.",
    de: "Das hat die Box aufgeschrieben. Adressen, der Name der Box und die Namen der Leute darauf sind entfernt.",
  },
  "problems.report.writing": {
    en: "Writing the report from the log…",
    sk: "Píšem hlásenie zo záznamu…",
    de: "Schreibe den Bericht aus dem Protokoll…",
  },
  "problems.report.failed": {
    en: "Could not write the report.",
    sk: "Hlásenie sa nepodarilo napísať.",
    de: "Der Bericht konnte nicht geschrieben werden.",
  },
  "problems.report.note": {
    en: "What were you doing when it happened? (optional, sent with the report)",
    sk: "Čo ste robili, keď sa to stalo? (nepovinné, odošle sa s hlásením)",
    de: "Was hast du gemacht, als es passierte? (optional, wird mit dem Bericht gesendet)",
  },
  "problems.report.notePlaceholder": {
    en: "I had just installed an app…",
    sk: "Práve som nainštaloval aplikáciu…",
    de: "Ich hatte gerade eine App installiert…",
  },
  "problems.report.send": { en: "Send on GitHub", sk: "Odoslať na GitHub", de: "Auf GitHub senden" },
  "problems.report.sendAgain": { en: "Try sending again", sk: "Skúsiť odoslať znova", de: "Erneut senden" },
  "problems.report.stop": { en: "Stop", sk: "Zastaviť", de: "Abbrechen" },
  "problems.report.close": { en: "Close", sk: "Zavrieť", de: "Schließen" },

  // ── Sending ─────────────────────────────────────────────────────────────
  "problems.send.starting": {
    en: "Asking GitHub for a code…",
    sk: "Žiadam GitHub o kód…",
    de: "Frage GitHub nach einem Code…",
  },
  "problems.send.enterCode": {
    en: "Open GitHub, sign in and enter this code. The report is sent as you.",
    sk: "Otvorte GitHub, prihláste sa a zadajte tento kód. Hlásenie sa odošle pod vaším menom.",
    de: "Öffne GitHub, melde dich an und gib diesen Code ein. Der Bericht wird in deinem Namen gesendet.",
  },
  "problems.send.open": { en: "Open {address}", sk: "Otvoriť {address}", de: "{address} öffnen" },
  "problems.send.waiting": {
    en: "Waiting for the code to be entered…",
    sk: "Čakám na zadanie kódu…",
    de: "Warte auf die Eingabe des Codes…",
  },
  "problems.send.opened": { en: "Sent. The report is at", sk: "Odoslané. Hlásenie je na", de: "Gesendet. Der Bericht ist unter" },
  "problems.send.addedTo": {
    en: "Someone had reported this already. Your report was added to",
    sk: "Toto už niekto nahlásil. Vaše hlásenie bolo pridané k",
    de: "Das hatte schon jemand gemeldet. Dein Bericht wurde hinzugefügt zu",
  },
  "problems.send.cancelled": {
    en: "Stopped. Nothing was sent.",
    sk: "Zastavené. Nič sa neodoslalo.",
    de: "Abgebrochen. Nichts wurde gesendet.",
  },
});
