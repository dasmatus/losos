import { defineMessages } from "./define";

/* LosOS Mail (src/mail/), the mail screen at /mail. It exists only on the
 * shared side (sharing on, in a mesh), and its mail stays between the people
 * on the box. */
export default defineMessages({
  "mail.title": { en: "Mail", sk: "Pošta", de: "Mail" },
  "mail.summary": {
    en: "Your address is {address}. Mail goes between the people on this box.",
    sk: "Vaša adresa je {address}. Pošta chodí medzi ľuďmi na tomto boxe.",
    de: "Deine Adresse ist {address}. Mail geht zwischen den Personen auf dieser Box.",
  },
  "mail.missing": {
    en: "There is no mail on this box. Mail runs only while the box shares its storage with a mesh it has joined.",
    sk: "Na tomto boxe nie je pošta. Pošta beží, len kým box zdieľa svoje úložisko so sieťou, ku ktorej sa pripojil.",
    de: "Auf dieser Box gibt es keine Mail. Mail läuft nur, solange die Box ihren Speicher mit einem Mesh teilt, dem sie beigetreten ist.",
  },
  "mail.failed": { en: "Mail did not load: {detail}", sk: "Pošta sa nenačítala: {detail}", de: "Mail wurde nicht geladen: {detail}" },
  "mail.retry": { en: "Try again", sk: "Skúsiť znova", de: "Erneut versuchen" },
  "mail.signIn.text": {
    en: "Sign in with your own account to read your mail. The spare key has no mailbox.",
    sk: "Prihláste sa vlastným účtom, aby ste si prečítali poštu. Náhradný kľúč nemá schránku.",
    de: "Melde dich mit deinem eigenen Konto an, um deine Mail zu lesen. Der Ersatzschlüssel hat kein Postfach.",
  },
  "mail.signIn.button": { en: "Sign in", sk: "Prihlásiť sa", de: "Anmelden" },

  // ── folders ──
  "mail.folders": { en: "Folders", sk: "Priečinky", de: "Ordner" },
  "mail.folder.inbox": { en: "Inbox", sk: "Doručená pošta", de: "Posteingang" },
  "mail.folder.drafts": { en: "Drafts", sk: "Koncepty", de: "Entwürfe" },
  "mail.folder.sent": { en: "Sent", sk: "Odoslaná pošta", de: "Gesendet" },
  "mail.folder.archive": { en: "Archive", sk: "Archív", de: "Archiv" },
  "mail.folder.junk": { en: "Junk", sk: "Nevyžiadaná pošta", de: "Spam" },
  "mail.folder.trash": { en: "Bin", sk: "Kôš", de: "Papierkorb" },

  // ── the list ──
  "mail.search": { en: "Search {folder}", sk: "Hľadať v {folder}", de: "{folder} durchsuchen" },
  "mail.searchLabel": { en: "Search mail", sk: "Hľadať v pošte", de: "Mail durchsuchen" },
  "mail.results": { en: "Results for “{query}”", sk: "Výsledky pre „{query}“", de: "Ergebnisse für „{query}“" },
  "mail.empty": { en: "Nothing here.", sk: "Nič tu nie je.", de: "Hier ist nichts." },
  "mail.noResults": { en: "No message matches.", sk: "Žiadna správa sa nezhoduje.", de: "Keine Nachricht passt." },
  "mail.more": { en: "Show more", sk: "Zobraziť viac", de: "Mehr anzeigen" },
  "mail.unread": { en: "Unread", sk: "Neprečítané", de: "Ungelesen" },
  "mail.flagged": { en: "Flagged", sk: "Označené", de: "Markiert" },
  "mail.attachment": { en: "Has an attachment", sk: "Má prílohu", de: "Hat einen Anhang" },
  "mail.noSubject": { en: "(no subject)", sk: "(bez predmetu)", de: "(kein Betreff)" },
  "mail.nobody": { en: "(nobody)", sk: "(nikto)", de: "(niemand)" },
  "mail.toPrefix": { en: "To:", sk: "Komu:", de: "An:" },

  // ── the reader ──
  "mail.pick": { en: "Pick a message to read it.", sk: "Vyberte správu, ktorú chcete čítať.", de: "Wähle eine Nachricht zum Lesen." },
  "mail.back": { en: "Back to the list", sk: "Späť na zoznam", de: "Zurück zur Liste" },
  "mail.from": { en: "From", sk: "Od", de: "Von" },
  "mail.to": { en: "To", sk: "Komu", de: "An" },
  "mail.cc": { en: "Cc", sk: "Kópia", de: "Cc" },
  "mail.reply": { en: "Reply", sk: "Odpovedať", de: "Antworten" },
  "mail.replyAll": { en: "Reply all", sk: "Odpovedať všetkým", de: "Allen antworten" },
  "mail.forward": { en: "Forward", sk: "Preposlať", de: "Weiterleiten" },
  "mail.flag": { en: "Flag", sk: "Označiť", de: "Markieren" },
  "mail.unflag": { en: "Remove the flag", sk: "Zrušiť označenie", de: "Markierung entfernen" },
  "mail.markUnread": { en: "Mark as unread", sk: "Označiť ako neprečítané", de: "Als ungelesen markieren" },
  "mail.markRead": { en: "Mark as read", sk: "Označiť ako prečítané", de: "Als gelesen markieren" },
  "mail.archive": { en: "Archive", sk: "Archivovať", de: "Archivieren" },
  "mail.delete": { en: "Delete", sk: "Odstrániť", de: "Löschen" },
  "mail.moveTo": { en: "Move to…", sk: "Presunúť do…", de: "Verschieben nach…" },
  "mail.showHtml": { en: "Show formatting", sk: "Zobraziť formátovanie", de: "Formatierung anzeigen" },
  "mail.showText": { en: "Show plain text", sk: "Zobraziť obyčajný text", de: "Nur Text anzeigen" },
  "mail.htmlTitle": { en: "The message, as its sender formatted it", sk: "Správa tak, ako ju odosielateľ naformátoval", de: "Die Nachricht, wie der Absender sie formatiert hat" },
  "mail.attachments": { en: "Attachments", sk: "Prílohy", de: "Anhänge" },
  "mail.actionFailed": { en: "That did not work", sk: "To sa nepodarilo", de: "Das hat nicht geklappt" },

  "mail.done.archived": { en: "Archived", sk: "Archivované", de: "Archiviert" },
  "mail.done.binned": { en: "Moved to the bin", sk: "Presunuté do koša", de: "In den Papierkorb verschoben" },
  "mail.done.deleted": { en: "Deleted", sk: "Odstránené", de: "Gelöscht" },
  "mail.done.flagged": { en: "Flagged", sk: "Označené", de: "Markiert" },
  "mail.done.unflagged": { en: "Flag removed", sk: "Označenie zrušené", de: "Markierung entfernt" },
  "mail.done.moved": { en: "Moved", sk: "Presunuté", de: "Verschoben" },
  "mail.done.unread": { en: "Marked as unread", sk: "Označené ako neprečítané", de: "Als ungelesen markiert" },
  "mail.done.read": { en: "Marked as read", sk: "Označené ako prečítané", de: "Als gelesen markiert" },
  "mail.done.sent": { en: "Sent", sk: "Odoslané", de: "Gesendet" },

  // ── writing ──
  "mail.compose.new": { en: "Write", sk: "Napísať", de: "Schreiben" },
  "mail.compose.title": { en: "New message", sk: "Nová správa", de: "Neue Nachricht" },
  "mail.compose.reply": { en: "Reply", sk: "Odpoveď", de: "Antwort" },
  "mail.compose.from": { en: "From {address}", sk: "Od {address}", de: "Von {address}" },
  "mail.compose.addCc": { en: "Add Cc", sk: "Pridať kópiu", de: "Cc hinzufügen" },
  "mail.compose.subject": { en: "Subject", sk: "Predmet", de: "Betreff" },
  "mail.compose.body": { en: "Message", sk: "Správa", de: "Nachricht" },
  "mail.compose.local": {
    en: "Mail goes to people on this box only.",
    sk: "Pošta ide len ľuďom na tomto boxe.",
    de: "Mail geht nur an Personen auf dieser Box.",
  },
  "mail.compose.discard": { en: "Discard", sk: "Zahodiť", de: "Verwerfen" },
  "mail.compose.send": { en: "Send", sk: "Odoslať", de: "Senden" },
  "mail.compose.re": { en: "Re:", sk: "Re:", de: "Re:" },
  "mail.compose.fwd": { en: "Fwd:", sk: "Fwd:", de: "Fwd:" },
  "mail.compose.wrote": { en: "On {when}, {who} wrote:", sk: "{when} {who} napísal(a):", de: "Am {when} schrieb {who}:" },
});
