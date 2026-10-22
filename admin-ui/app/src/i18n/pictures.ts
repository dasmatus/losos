import { defineMessages } from "./define";

/* Pictures in Chat and Mail (lib/pictures.ts, lib/nudity.ts): the blur on
 * pictures from other people, the nudity warning, the settings in Account
 * and the age mark in People. */
export default defineMessages({
  // ── a hidden picture ──
  "pictures.hidden.label": {
    en: "Picture from {name}, hidden. Press to show it.",
    sk: "Obrázok od {name}, skrytý. Stlačte, ak ho chcete zobraziť.",
    de: "Bild von {name}, verborgen. Drück darauf, um es anzuzeigen.",
  },
  "pictures.hidden.from": { en: "Picture from {name}", sk: "Obrázok od {name}", de: "Bild von {name}" },
  "pictures.hidden.press": { en: "Press to show", sk: "Stlačte na zobrazenie", de: "Zum Anzeigen drücken" },
  "pictures.hidden.trust": {
    en: "Always show pictures from {name}",
    sk: "Vždy zobrazovať obrázky od {name}",
    de: "Bilder von {name} immer anzeigen",
  },

  // ── the warning on a picture received ──
  "pictures.nudity.title": {
    en: "This picture may show nudity",
    sk: "Tento obrázok môže zobrazovať nahotu",
    de: "Dieses Bild zeigt vielleicht Nacktheit",
  },
  "pictures.nudity.body": {
    en: "The check on this device thinks so, and it can be wrong. Nothing was sent anywhere.",
    sk: "Kontrola v tomto zariadení si to myslí a môže sa mýliť. Nič sa nikam neposlalo.",
    de: "Die Prüfung auf diesem Gerät meint das, und sie kann sich irren. Nichts wurde irgendwohin geschickt.",
  },
  "pictures.nudity.childBody": {
    en: "You don't have to look. If a picture or the person who sent it makes you uncomfortable, talk to an adult you trust.",
    sk: "Nemusíte sa pozerať. Ak vám obrázok alebo ten, kto ho poslal, nie je príjemný, povedzte to dospelému, ktorému dôverujete.",
    de: "Du musst nicht hinsehen. Wenn dir ein Bild oder die Person, die es geschickt hat, unangenehm ist, sprich mit einem Erwachsenen, dem du vertraust.",
  },
  "pictures.nudity.show": { en: "Show anyway", sk: "Napriek tomu zobraziť", de: "Trotzdem anzeigen" },
  "pictures.nudity.checking": { en: "Checking the picture…", sk: "Obrázok sa kontroluje…", de: "Bild wird geprüft…" },

  // ── the question before sending ──
  "pictures.send.title": {
    en: "This picture may show nudity. Send it anyway?",
    sk: "Tento obrázok môže zobrazovať nahotu. Poslať ho aj tak?",
    de: "Dieses Bild zeigt vielleicht Nacktheit. Trotzdem senden?",
  },
  "pictures.send.body": {
    en: "Once it is sent, you cannot take it back from the other person's devices.",
    sk: "Keď ho pošlete, zo zariadení druhej osoby ho už nevezmete späť.",
    de: "Wenn es gesendet ist, kannst du es von den Geräten der anderen Person nicht mehr zurückholen.",
  },
  "pictures.send.childBody": {
    en: "Once it is sent, you cannot take it back. If someone asked you for a picture like this, talk to an adult you trust first.",
    sk: "Keď ho pošlete, už ho nevezmete späť. Ak vás niekto o takýto obrázok požiadal, povedzte to najprv dospelému, ktorému dôverujete.",
    de: "Wenn es gesendet ist, kannst du es nicht mehr zurückholen. Wenn dich jemand um so ein Bild gebeten hat, sprich zuerst mit einem Erwachsenen, dem du vertraust.",
  },
  "pictures.send.private": {
    en: "The check ran on this device. Nobody is told about it, whatever you choose.",
    sk: "Kontrola prebehla v tomto zariadení. Nikto sa o nej nedozvie, nech sa rozhodnete akokoľvek.",
    de: "Die Prüfung lief auf diesem Gerät. Niemand erfährt davon, egal wie du dich entscheidest.",
  },
  "pictures.send.anyway": { en: "Send anyway", sk: "Napriek tomu poslať", de: "Trotzdem senden" },
  "pictures.send.cancel": { en: "Don't send", sk: "Neposielať", de: "Nicht senden" },

  // ── Mail ──
  "pictures.mail.hidden": {
    en: "Pictures in this message are hidden.",
    sk: "Obrázky v tejto správe sú skryté.",
    de: "Bilder in dieser Nachricht sind verborgen.",
  },
  "pictures.mail.show": { en: "Show pictures", sk: "Zobraziť obrázky", de: "Bilder anzeigen" },

  // ── Settings › Account ──
  "pictures.settings.title": {
    en: "Pictures and private details",
    sk: "Obrázky a súkromné údaje",
    de: "Bilder und private Daten",
  },
  "pictures.settings.blur": { en: "Hide pictures from", sk: "Skryť obrázky od", de: "Bilder verbergen von" },
  "pictures.settings.blur.everyone": { en: "Everyone", sk: "Všetkých", de: "Allen" },
  "pictures.settings.blur.strangers": { en: "Strangers", sk: "Neznámych", de: "Fremden" },
  "pictures.settings.blur.nobody": { en: "No one", sk: "Nikoho", de: "Niemandem" },
  "pictures.settings.blur.everyone.detail": {
    en: "Every picture waits blurred until you press it, except pictures from people you chose to always show.",
    sk: "Každý obrázok čaká rozmazaný, kým ho nestlačíte, okrem obrázkov od ľudí, ktorých obrázky ste sa rozhodli vždy zobrazovať.",
    de: "Jedes Bild wartet verschwommen, bis du darauf drückst, außer Bilder von Personen, die du immer anzeigen lässt.",
  },
  "pictures.settings.blur.strangers.detail": {
    en: "Pictures from people on this box and in your contacts show at once. Everyone else's wait blurred.",
    sk: "Obrázky od ľudí na tomto boxe a vo vašich kontaktoch sa zobrazia hneď. Ostatné čakajú rozmazané.",
    de: "Bilder von Personen auf dieser Box und in deinen Kontakten erscheinen sofort. Alle anderen warten verschwommen.",
  },
  "pictures.settings.blur.nobody.detail": {
    en: "Every picture shows at once.",
    sk: "Každý obrázok sa zobrazí hneď.",
    de: "Jedes Bild erscheint sofort.",
  },
  "pictures.settings.warning": {
    en: "Warn me about nudity",
    sk: "Upozorniť ma na nahotu",
    de: "Vor Nacktheit warnen",
  },
  "pictures.settings.warningDetail": {
    en: "A small model on this device checks Chat pictures before you see or send them. Pictures and results stay on the device.",
    sk: "Malý model v tomto zariadení skontroluje obrázky v Chate predtým, ako ich uvidíte alebo pošlete. Obrázky ani výsledky zariadenie neopustia.",
    de: "Ein kleines Modell auf diesem Gerät prüft Chat-Bilder, bevor du sie siehst oder sendest. Bilder und Ergebnisse bleiben auf dem Gerät.",
  },
  "pictures.settings.warningChild": {
    en: "Always on for an account marked as a child's.",
    sk: "Pri účte označenom ako detský je vždy zapnuté.",
    de: "Bei einem Konto, das als Kinderkonto markiert ist, immer an.",
  },
  "pictures.settings.trusted": {
    en: "Always shown from",
    sk: "Vždy zobrazené od",
    de: "Immer angezeigt von",
  },
  "pictures.settings.untrust": {
    en: "Stop always showing pictures from {name}",
    sk: "Prestať vždy zobrazovať obrázky od {name}",
    de: "Bilder von {name} nicht mehr immer anzeigen",
  },
  "pictures.settings.age": { en: "Age", sk: "Vek", de: "Alter" },
  "pictures.settings.caption": {
    en: "Showing every picture at once needs an adult mark. Buy with Binance once under Market, or ask an administrator to mark you under Users.",
    sk: "Na okamžité zobrazovanie všetkých obrázkov potrebujete označenie dospelého. Raz nakúpte cez Binance v časti Trh alebo požiadajte správcu, aby vás označil v časti Používatelia.",
    de: "Damit jedes Bild sofort erscheint, brauchst du eine Markierung als erwachsen. Kauf einmal mit Binance unter Markt, oder bitte jemanden aus der Verwaltung, dich unter Benutzer zu markieren.",
  },
  "pictures.settings.captionAdult": {
    en: "You are marked as an adult, so you may show every picture at once.",
    sk: "Ste označení ako dospelý, takže môžete zobrazovať všetky obrázky hneď.",
    de: "Du bist als erwachsen markiert und kannst jedes Bild sofort anzeigen lassen.",
  },
  "pictures.settings.adultRequired": {
    en: "Showing every picture needs an adult mark first.",
    sk: "Na zobrazovanie všetkých obrázkov najprv potrebujete označenie dospelého.",
    de: "Um jedes Bild anzuzeigen, brauchst du zuerst eine Markierung als erwachsen.",
  },
  "pictures.settings.childAccount": {
    en: "This account is marked as a child's, so this stays on.",
    sk: "Tento účet je označený ako detský, takže toto zostáva zapnuté.",
    de: "Dieses Konto ist als Kinderkonto markiert, darum bleibt das an.",
  },
  "pictures.settings.failed": {
    en: "The setting did not change. Try again.",
    sk: "Nastavenie sa nezmenilo. Skúste to znova.",
    de: "Die Einstellung hat sich nicht geändert. Versuch es noch einmal.",
  },

  // ── the age mark ──
  "pictures.age.none": { en: "Not marked", sk: "Neoznačené", de: "Nicht markiert" },
  "pictures.age.child": {
    en: "Child, marked by an administrator",
    sk: "Dieťa, označil správca",
    de: "Kind, markiert von der Verwaltung",
  },
  "pictures.age.adultBinance": {
    en: "Adult, by a Binance purchase",
    sk: "Dospelý, podľa nákupu cez Binance",
    de: "Erwachsen, durch einen Kauf mit Binance",
  },
  "pictures.age.adultAdmin": {
    en: "Adult, marked by an administrator",
    sk: "Dospelý, označil správca",
    de: "Erwachsen, markiert von der Verwaltung",
  },
  "pictures.age.badgeAdult": { en: "Adult", sk: "Dospelý", de: "Erwachsen" },
  "pictures.age.badgeChild": { en: "Child", sk: "Dieťa", de: "Kind" },
  "pictures.age.edit": { en: "Age for pictures", sk: "Vek pre obrázky", de: "Alter für Bilder" },
  "pictures.age.dialogTitle": { en: "Age of {name}", sk: "Vek: {name}", de: "Alter von {name}" },
  "pictures.age.dialogBody": {
    en: "This decides what they may change about pictures in Chat and Mail. Nothing else on the box depends on it.",
    sk: "Toto určuje, čo môže daná osoba meniť pri obrázkoch v Chate a Pošte. Nič iné na boxe od toho nezávisí.",
    de: "Das bestimmt, was die Person bei Bildern in Chat und Mail ändern darf. Sonst hängt nichts auf der Box davon ab.",
  },
  "pictures.age.choice.adult": { en: "Adult", sk: "Dospelý", de: "Erwachsen" },
  "pictures.age.choice.child": { en: "Child", sk: "Dieťa", de: "Kind" },
  "pictures.age.choice.none": { en: "Not marked", sk: "Neoznačené", de: "Nicht markiert" },
  "pictures.age.choice.adult.detail": {
    en: "May choose to show every picture at once.",
    sk: "Môže si zvoliť okamžité zobrazovanie všetkých obrázkov.",
    de: "Darf wählen, dass jedes Bild sofort erscheint.",
  },
  "pictures.age.choice.child.detail": {
    en: "The nudity warning stays on, and pictures from strangers always wait blurred. A Binance purchase does not change this.",
    sk: "Upozornenie na nahotu zostáva zapnuté a obrázky od neznámych vždy čakajú rozmazané. Nákup cez Binance to nezmení.",
    de: "Die Nacktheitswarnung bleibt an, und Bilder von Fremden warten immer verschwommen. Ein Kauf mit Binance ändert das nicht.",
  },
  "pictures.age.choice.none.detail": {
    en: "Pictures from strangers wait blurred. A finished Binance purchase marks them as an adult.",
    sk: "Obrázky od neznámych čakajú rozmazané. Dokončený nákup cez Binance osobu označí ako dospelú.",
    de: "Bilder von Fremden warten verschwommen. Ein abgeschlossener Kauf mit Binance markiert die Person als erwachsen.",
  },
  "pictures.age.save": { en: "Save", sk: "Uložiť", de: "Speichern" },
  "pictures.age.failed": {
    en: "The age mark did not change",
    sk: "Označenie veku sa nezmenilo",
    de: "Die Altersmarkierung hat sich nicht geändert",
  },
});
