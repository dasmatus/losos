import { defineMessages } from "./define";

/* Private details in Chat and Mail (lib/sensitive.ts,
 * components/details.tsx): card, IBAN, passport and ID card, birth and
 * Social Security numbers, blacked out where they are shown and asked
 * about before they are sent. */
export default defineMessages({
  "details.kind.card": { en: "Card number", sk: "Číslo karty", de: "Kartennummer" },
  "details.kind.iban": { en: "IBAN", sk: "Číslo IBAN", de: "IBAN" },
  "details.kind.document": {
    en: "Passport or ID card number",
    sk: "Číslo pasu alebo občianskeho preukazu",
    de: "Pass- oder Ausweisnummer",
  },
  "details.kind.birthNumber": { en: "Birth number", sk: "Rodné číslo", de: "Geburtsnummer" },
  "details.kind.ssn": { en: "Social Security number", sk: "Číslo sociálneho zabezpečenia (USA)", de: "US-Sozialversicherungsnummer" },
  "details.ending": { en: "{kind} ending {last}", sk: "{kind} končiace na {last}", de: "{kind}, endet auf {last}" },

  // ── black where shown ──
  "details.hidden": {
    en: "{kind}, hidden. Press to show it.",
    sk: "{kind}, skryté. Stlačte, ak ho chcete zobraziť.",
    de: "{kind}, verborgen. Drück darauf, um sie anzuzeigen.",
  },
  "details.shown": {
    en: "{kind}, shown. Press to hide it again.",
    sk: "{kind}, zobrazené. Stlačte, ak ho chcete znova skryť.",
    de: "{kind}, angezeigt. Drück darauf, um sie wieder zu verbergen.",
  },

  // ── the question before sending ──
  "details.send.title": {
    en: "This message has private details. Send it anyway?",
    sk: "Táto správa obsahuje súkromné údaje. Poslať ju aj tak?",
    de: "Diese Nachricht enthält private Daten. Trotzdem senden?",
  },
  "details.send.body": {
    en: "Anyone who gets the message can keep and pass these on. Once it is sent, you cannot take it back.",
    sk: "Ktokoľvek, kto správu dostane, si ich môže ponechať a poslať ďalej. Keď ju pošlete, už ju nevezmete späť.",
    de: "Wer die Nachricht bekommt, kann sie behalten und weitergeben. Wenn sie gesendet ist, kannst du sie nicht mehr zurückholen.",
  },
  "details.send.childBody": {
    en: "Never send these because someone online asked for them. Ask an adult you trust first.",
    sk: "Nikdy ich neposielajte len preto, že vás o ne niekto na internete požiadal. Najprv sa opýtajte dospelého, ktorému dôverujete.",
    de: "Schick so etwas nie, nur weil jemand im Internet danach fragt. Frag zuerst einen Erwachsenen, dem du vertraust.",
  },
  "details.send.private": {
    en: "The check ran on this device. Nobody is told about it, whatever you choose.",
    sk: "Kontrola prebehla v tomto zariadení. Nikto sa o nej nedozvie, nech sa rozhodnete akokoľvek.",
    de: "Die Prüfung lief auf diesem Gerät. Niemand erfährt davon, egal wie du dich entscheidest.",
  },
  "details.send.anyway": { en: "Send anyway", sk: "Napriek tomu poslať", de: "Trotzdem senden" },
  "details.send.cancel": { en: "Don't send", sk: "Neposielať", de: "Nicht senden" },

  // ── Settings › Your account ──
  "details.settings.warning": {
    en: "Black out private details",
    sk: "Zakryť súkromné údaje",
    de: "Private Daten schwärzen",
  },
  "details.settings.warningDetail": {
    en: "Card, IBAN, passport, ID card and birth numbers show as black bars in Chat and Mail until you press them, and you are asked before you send one. Checked on this device.",
    sk: "Čísla kariet, IBAN, pasov, občianskych preukazov a rodné čísla sa v Chate a Pošte zobrazia ako čierne pruhy, kým ich nestlačíte, a pred odoslaním sa vás opýta. Kontroluje sa v tomto zariadení.",
    de: "Karten-, IBAN-, Pass-, Ausweis- und Geburtsnummern erscheinen in Chat und Mail als schwarze Balken, bis du darauf drückst, und du wirst gefragt, bevor du eine sendest. Geprüft auf diesem Gerät.",
  },
  "details.settings.warningChild": {
    en: "Always on for an account marked as a child's.",
    sk: "Pri účte označenom ako detský je vždy zapnuté.",
    de: "Bei einem Konto, das als Kinderkonto markiert ist, immer an.",
  },
});
