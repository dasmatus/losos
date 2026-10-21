import { defineMessages } from "./define";

/* What the apps hand each other (lib/intents.ts, lib/cloud-files.ts): the
 * Cloud picker, saving into Cloud, the share sheet, opening a file in the
 * app that edits it, and asking the box's AI about something. */
export default defineMessages({
  "together.cancel": { en: "Cancel", sk: "Zrušiť", de: "Abbrechen" },
  "together.close": { en: "Close", sk: "Zavrieť", de: "Schließen" },

  "together.picker.title": { en: "Choose from LosOS Cloud", sk: "Vybrať z LosOS Cloud", de: "Aus LosOS Cloud wählen" },
  "together.picker.home": { en: "Files", sk: "Súbory", de: "Dateien" },
  "together.picker.up": { en: "Up one folder", sk: "O priečinok vyššie", de: "Einen Ordner höher" },
  "together.picker.loading": { en: "Loading the folder", sk: "Načítava sa priečinok", de: "Ordner wird geladen" },
  "together.picker.failed": {
    en: "LosOS Cloud did not answer. Try again in a moment.",
    sk: "LosOS Cloud neodpovedal. Skúste to o chvíľu znova.",
    de: "LosOS Cloud hat nicht geantwortet. Versuch es gleich noch einmal.",
  },
  "together.picker.empty": { en: "This folder is empty.", sk: "Tento priečinok je prázdny.", de: "Dieser Ordner ist leer." },
  "together.picker.picked": {
    en: { one: "{count} file chosen", other: "{count} files chosen" },
    sk: { one: "Vybraný {count} súbor", few: "Vybrané {count} súbory", many: "Vybraných {count} súborov", other: "Vybraných {count} súborov" },
    de: { one: "{count} Datei gewählt", other: "{count} Dateien gewählt" },
  },
  "together.picker.attach": { en: "Attach", sk: "Priložiť", de: "Anhängen" },

  "together.save.action": { en: "Save to LosOS Cloud", sk: "Uložiť do LosOS Cloud", de: "In LosOS Cloud speichern" },
  "together.save.saving": { en: "Saving…", sk: "Ukladá sa…", de: "Wird gespeichert…" },
  "together.save.done": { en: "Saved to LosOS Cloud", sk: "Uložené do LosOS Cloud", de: "In LosOS Cloud gespeichert" },
  "together.save.where": { en: "In the {folder} folder", sk: "V priečinku {folder}", de: "Im Ordner {folder}" },
  "together.save.failed": {
    en: "Could not save to LosOS Cloud",
    sk: "Do LosOS Cloud sa nepodarilo uložiť",
    de: "Speichern in LosOS Cloud fehlgeschlagen",
  },
  "together.show": { en: "Show in LosOS Cloud", sk: "Ukázať v LosOS Cloud", de: "In LosOS Cloud zeigen" },
  "together.open": { en: "Open", sk: "Otvoriť", de: "Öffnen" },
  "together.opening": { en: "Opening {name}", sk: "Otvára sa {name}", de: "{name} wird geöffnet" },

  "together.send.mail": { en: "Send by mail", sk: "Poslať e-mailom", de: "Per Mail senden" },
  "together.send.chat": { en: "Send in chat", sk: "Poslať v chate", de: "Im Chat senden" },
  "together.send.share": { en: "Send to…", sk: "Poslať…", de: "Senden an…" },
  "together.share.title": { en: "Send {name}", sk: "Poslať {name}", de: "{name} senden" },
  "together.share.many": {
    en: { one: "Send {count} file", other: "Send {count} files" },
    sk: { one: "Poslať {count} súbor", few: "Poslať {count} súbory", many: "Poslať {count} súborov", other: "Poslať {count} súborov" },
    de: { one: "{count} Datei senden", other: "{count} Dateien senden" },
  },
  "together.share.body": {
    en: "Pick where it goes. Mail and chat send a copy; the file stays in your LosOS Cloud.",
    sk: "Vyberte, kam to pôjde. E-mail a chat pošlú kópiu, súbor zostane vo vašom LosOS Cloud.",
    de: "Wähle, wohin es geht. Mail und Chat senden eine Kopie, die Datei bleibt in deiner LosOS Cloud.",
  },
  "together.share.mailHint": { en: "As an attachment", sk: "Ako príloha", de: "Als Anhang" },
  "together.share.chatHint": { en: "Into a room you pick", sk: "Do miestnosti, ktorú vyberiete", de: "In einen Raum deiner Wahl" },
  "together.share.linkHint": { en: "Opens for people on this box", sk: "Otvorí sa ľuďom na tomto zariadení", de: "Öffnet sich für Leute auf dieser Box" },
  "together.share.copyLink": { en: "Copy link", sk: "Kopírovať odkaz", de: "Link kopieren" },
  "together.share.copied": { en: "Link copied", sk: "Odkaz skopírovaný", de: "Link kopiert" },

  "together.mail.chat": { en: "Chat", sk: "Chat", de: "Chat" },
  "together.mail.call": { en: "Call", sk: "Zavolať", de: "Anrufen" },
  "together.mail.attachFile": { en: "Attach from this computer", sk: "Priložiť z tohto počítača", de: "Von diesem Computer anhängen" },
  "together.mail.attachCloud": { en: "Attach from LosOS Cloud", sk: "Priložiť z LosOS Cloud", de: "Aus LosOS Cloud anhängen" },
  "together.mail.remove": { en: "Remove {name}", sk: "Odstrániť {name}", de: "{name} entfernen" },
  "together.mail.tooBig": {
    en: "Attachments go up to {size} in all",
    sk: "Prílohy môžu mať spolu najviac {size}",
    de: "Anhänge dürfen zusammen höchstens {size} groß sein",
  },
  "together.mail.reading": { en: "Reading {name} from LosOS Cloud", sk: "Načítava sa {name} z LosOS Cloud", de: "{name} wird aus LosOS Cloud gelesen" },

  "together.ai.ask": { en: "Ask AI", sk: "Opýtať sa AI", de: "KI fragen" },
  "together.ai.askAbout": { en: "Ask AI about this", sk: "Opýtať sa AI na toto", de: "Die KI dazu fragen" },
  "together.ai.summarise": { en: "Summarise", sk: "Zhrnúť", de: "Zusammenfassen" },
  "together.ai.title": { en: "Ask the box’s AI", sk: "Opýtať sa AI zariadenia", de: "Die KI der Box fragen" },
  "together.ai.about": { en: "About {title}", sk: "O {title}", de: "Zu {title}" },
  "together.ai.private": {
    en: "Runs on this box. What you ask and what it reads stay here.",
    sk: "Beží na tomto zariadení. Otázka aj to, čo číta, zostávajú tu.",
    de: "Läuft auf dieser Box. Deine Frage und was sie liest, bleiben hier.",
  },
  "together.ai.question": { en: "Your question", sk: "Vaša otázka", de: "Deine Frage" },
  "together.ai.placeholder": { en: "What should it tell you?", sk: "Čo vám má povedať?", de: "Was soll sie dir sagen?" },
  "together.ai.summary": {
    en: "Summarise this in a few sentences.",
    sk: "Zhrň to v niekoľkých vetách.",
    de: "Fass das in ein paar Sätzen zusammen.",
  },
  "together.ai.send": { en: "Ask", sk: "Opýtať sa", de: "Fragen" },
  "together.ai.stop": { en: "Stop", sk: "Zastaviť", de: "Stoppen" },
  "together.ai.again": { en: "Ask something else", sk: "Opýtať sa niečo iné", de: "Etwas anderes fragen" },
  "together.ai.thinking": { en: "Reading and thinking…", sk: "Číta a premýšľa…", de: "Liest und denkt nach…" },
  "together.ai.waiting": {
    en: "Waiting for the box’s AI: it answers one question at a time.",
    sk: "Čaká sa na AI zariadenia: odpovedá na jednu otázku naraz.",
    de: "Wartet auf die KI der Box: Sie beantwortet eine Frage nach der anderen.",
  },
  "together.ai.cut": {
    en: "Only the first part was read; the rest did not fit.",
    sk: "Prečítala sa len prvá časť, zvyšok sa nezmestil.",
    de: "Nur der erste Teil wurde gelesen, der Rest passte nicht hinein.",
  },
  "together.ai.failed": {
    en: "The box’s AI could not answer",
    sk: "AI zariadenia nedokázala odpovedať",
    de: "Die KI der Box konnte nicht antworten",
  },
  "together.ai.copy": { en: "Copy the answer", sk: "Kopírovať odpoveď", de: "Antwort kopieren" },
  "together.ai.saveNote": { en: "Save as a note", sk: "Uložiť ako poznámku", de: "Als Notiz speichern" },
  "together.ai.reply": { en: "Use as a reply", sk: "Použiť ako odpoveď", de: "Als Antwort verwenden" },
  "together.ai.notesFolder": { en: "Notes", sk: "Poznámky", de: "Notizen" },
});
