import { defineMessages } from "./define";

/* LosOS Contacts (src/contacts/), the address book at /contacts. Each
 * person's book is kept in Matrix on the box's chat server, so it exists
 * only on the shared side (sharing on, in a mesh), like Chat and Mail. */
export default defineMessages({
  "contacts.title": { en: "Contacts", sk: "Kontakty", de: "Kontakte" },
  "contacts.summary": {
    en: "Your address book, kept in LosOS Chat on this box. Only you can see it.",
    sk: "Váš adresár, uložený v LosOS Chat na tomto boxe. Vidíte ho len vy.",
    de: "Dein Adressbuch, gespeichert in LosOS Chat auf dieser Box. Nur du siehst es.",
  },
  "contacts.missing": {
    en: "There are no contacts on this box. Contacts runs only while the box shares its storage with a mesh it has joined.",
    sk: "Na tomto boxe nie sú kontakty. Kontakty bežia, len kým box zdieľa svoje úložisko so sieťou, ku ktorej sa pripojil.",
    de: "Auf dieser Box gibt es keine Kontakte. Kontakte laufen nur, solange die Box ihren Speicher mit einem Mesh teilt, dem sie beigetreten ist.",
  },
  "contacts.signIn.text": {
    en: "Sign in with your own account to see your contacts. The spare key has no address book.",
    sk: "Prihláste sa vlastným účtom, aby ste videli svoje kontakty. Náhradný kľúč nemá adresár.",
    de: "Melde dich mit deinem eigenen Konto an, um deine Kontakte zu sehen. Der Ersatzschlüssel hat kein Adressbuch.",
  },
  "contacts.signIn.button": { en: "Sign in", sk: "Prihlásiť sa", de: "Anmelden" },
  "contacts.failed": {
    en: "Contacts did not load: {detail}",
    sk: "Kontakty sa nenačítali: {detail}",
    de: "Kontakte wurden nicht geladen: {detail}",
  },
  "contacts.retry": { en: "Try again", sk: "Skúsiť znova", de: "Erneut versuchen" },

  // ── the list ──
  "contacts.new": { en: "New contact", sk: "Nový kontakt", de: "Neuer Kontakt" },
  "contacts.import": { en: "Import", sk: "Importovať", de: "Importieren" },
  "contacts.export": { en: "Export", sk: "Exportovať", de: "Exportieren" },
  "contacts.search": { en: "Search contacts", sk: "Hľadať kontakty", de: "Kontakte suchen" },
  "contacts.view.all": { en: "Your contacts", sk: "Vaše kontakty", de: "Deine Kontakte" },
  "contacts.view.box": { en: "On this box", sk: "Na tomto boxe", de: "Auf dieser Box" },
  "contacts.views": { en: "Address book", sk: "Adresár", de: "Adressbuch" },
  "contacts.empty": {
    en: "No contacts yet. Add one, or import a .vcf file from your phone or another address book.",
    sk: "Zatiaľ žiadne kontakty. Pridajte kontakt alebo importujte súbor .vcf z telefónu či iného adresára.",
    de: "Noch keine Kontakte. Füge einen hinzu oder importiere eine .vcf-Datei von deinem Handy oder aus einem anderen Adressbuch.",
  },
  "contacts.noResults": { en: "Nothing matches.", sk: "Nič sa nezhoduje.", de: "Nichts passt." },
  "contacts.boxEmpty": {
    en: "Nobody else has an account on this box yet.",
    sk: "Na tomto boxe zatiaľ nemá účet nikto iný.",
    de: "Auf dieser Box hat noch niemand sonst ein Konto.",
  },
  "contacts.pick": {
    en: "Pick someone to see how to reach them.",
    sk: "Vyberte niekoho a uvidíte, ako ho zastihnúť.",
    de: "Wähle jemanden aus, um zu sehen, wie du ihn erreichst.",
  },
  "contacts.back": { en: "Back to the list", sk: "Späť na zoznam", de: "Zurück zur Liste" },
  "contacts.onBox": { en: "Has an account on this box", sk: "Má účet na tomto boxe", de: "Hat ein Konto auf dieser Box" },

  // ── reaching someone ──
  "contacts.act.mail": { en: "Mail", sk: "Pošta", de: "Mail" },
  "contacts.act.chat": { en: "Chat", sk: "Chat", de: "Chat" },
  "contacts.act.call": { en: "Call", sk: "Hovor", de: "Anruf" },
  "contacts.act.video": { en: "Video", sk: "Video", de: "Video" },
  "contacts.act.addToBook": { en: "Add to contacts", sk: "Pridať do kontaktov", de: "Zu Kontakten hinzufügen" },
  "contacts.act.edit": { en: "Edit", sk: "Upraviť", de: "Bearbeiten" },
  "contacts.act.delete": { en: "Delete", sk: "Odstrániť", de: "Löschen" },
  "contacts.act.exportOne": { en: "Export as .vcf", sk: "Exportovať ako .vcf", de: "Als .vcf exportieren" },
  "contacts.act.copy": { en: "Copy", sk: "Kopírovať", de: "Kopieren" },
  "contacts.copied": { en: "Copied", sk: "Skopírované", de: "Kopiert" },

  // ── fields ──
  "contacts.field.name": { en: "Name", sk: "Meno", de: "Name" },
  "contacts.field.given": { en: "First name", sk: "Krstné meno", de: "Vorname" },
  "contacts.field.family": { en: "Last name", sk: "Priezvisko", de: "Nachname" },
  "contacts.field.nickname": { en: "Nickname", sk: "Prezývka", de: "Spitzname" },
  "contacts.field.org": { en: "Organisation", sk: "Organizácia", de: "Organisation" },
  "contacts.field.title": { en: "Job title", sk: "Pozícia", de: "Position" },
  "contacts.field.emails": { en: "Email", sk: "E-mail", de: "E-Mail" },
  "contacts.field.phones": { en: "Phone", sk: "Telefón", de: "Telefon" },
  "contacts.field.addresses": { en: "Address", sk: "Adresa", de: "Adresse" },
  "contacts.field.urls": { en: "Website", sk: "Web", de: "Website" },
  "contacts.field.birthday": { en: "Birthday", sk: "Narodeniny", de: "Geburtstag" },
  "contacts.field.note": { en: "Note", sk: "Poznámka", de: "Notiz" },
  "contacts.field.matrix": { en: "Chat (Matrix ID)", sk: "Chat (Matrix ID)", de: "Chat (Matrix-ID)" },
  "contacts.field.mesh": { en: "Mesh address", sk: "Adresa v sieti", de: "Mesh-Adresse" },
  "contacts.field.street": { en: "Street", sk: "Ulica", de: "Straße" },
  "contacts.field.locality": { en: "City", sk: "Mesto", de: "Ort" },
  "contacts.field.region": { en: "Region", sk: "Kraj", de: "Region" },
  "contacts.field.code": { en: "Postcode", sk: "PSČ", de: "PLZ" },
  "contacts.field.country": { en: "Country", sk: "Krajina", de: "Land" },
  "contacts.field.label": { en: "Kind", sk: "Druh", de: "Art" },
  "contacts.field.photo": { en: "Photo", sk: "Fotka", de: "Foto" },
  "contacts.matrixHint": {
    en: "Their chat address, like @eva:kitchenbox.local. Pick someone from this box or type it.",
    sk: "Adresa v chate, napríklad @eva:kitchenbox.local. Vyberte niekoho z tohto boxu alebo ju napíšte.",
    de: "Die Chat-Adresse, zum Beispiel @eva:kitchenbox.local. Wähle jemanden von dieser Box oder tippe sie ein.",
  },
  "contacts.meshHint": {
    en: "To share files with them on another box: name@box, like eva@kitchenbox.local.",
    sk: "Na zdieľanie súborov na inom boxe: meno@box, napríklad eva@kitchenbox.local.",
    de: "Um Dateien auf einer anderen Box mit ihnen zu teilen: name@box, zum Beispiel eva@kitchenbox.local.",
  },
  "contacts.birthdayHint": {
    en: "YYYY-MM-DD, or --MM-DD without the year",
    sk: "RRRR-MM-DD, alebo --MM-DD bez roku",
    de: "JJJJ-MM-TT, oder --MM-TT ohne Jahr",
  },
  "contacts.label.home": { en: "Home", sk: "Domov", de: "Privat" },
  "contacts.label.work": { en: "Work", sk: "Práca", de: "Arbeit" },
  "contacts.label.mobile": { en: "Mobile", sk: "Mobil", de: "Mobil" },
  "contacts.label.fax": { en: "Fax", sk: "Fax", de: "Fax" },
  "contacts.label.pager": { en: "Pager", sk: "Pager", de: "Pager" },
  "contacts.label.main": { en: "Main", sk: "Hlavné", de: "Haupt" },
  "contacts.label.other": { en: "Other", sk: "Iné", de: "Andere" },
  "contacts.label.none": { en: "No kind", sk: "Bez druhu", de: "Ohne Art" },
  "contacts.addRow": { en: "Add {what}", sk: "Pridať: {what}", de: "{what} hinzufügen" },
  "contacts.removeRow": { en: "Remove", sk: "Odobrať", de: "Entfernen" },

  // ── the editor ──
  "contacts.edit.newTitle": { en: "New contact", sk: "Nový kontakt", de: "Neuer Kontakt" },
  "contacts.edit.title": { en: "Edit contact", sk: "Upraviť kontakt", de: "Kontakt bearbeiten" },
  "contacts.edit.save": { en: "Save", sk: "Uložiť", de: "Speichern" },
  "contacts.edit.cancel": { en: "Cancel", sk: "Zrušiť", de: "Abbrechen" },
  "contacts.edit.photoAdd": { en: "Add a photo", sk: "Pridať fotku", de: "Foto hinzufügen" },
  "contacts.edit.photoChange": { en: "Change photo", sk: "Zmeniť fotku", de: "Foto ändern" },
  "contacts.edit.photoRemove": { en: "Remove photo", sk: "Odstrániť fotku", de: "Foto entfernen" },
  "contacts.edit.more": { en: "More fields", sk: "Ďalšie polia", de: "Weitere Felder" },
  "contacts.done.saved": { en: "Contact saved", sk: "Kontakt uložený", de: "Kontakt gespeichert" },
  "contacts.done.deleted": { en: "Contact deleted", sk: "Kontakt odstránený", de: "Kontakt gelöscht" },
  "contacts.confirmDelete": {
    en: "Delete {name}? This cannot be undone here.",
    sk: "Odstrániť kontakt {name}? Tu sa to nedá vrátiť.",
    de: "{name} löschen? Das lässt sich hier nicht rückgängig machen.",
  },

  // ── import and export ──
  "contacts.importTitle": { en: "Import contacts", sk: "Importovať kontakty", de: "Kontakte importieren" },
  "contacts.importLead": {
    en: "Pick a .vcf file: vCard 4.0, 3.0 or 2.1, with one contact or many. Contacts already in your book are updated, not added twice.",
    sk: "Vyberte súbor .vcf: vCard 4.0, 3.0 alebo 2.1, s jedným kontaktom alebo s mnohými. Kontakty, ktoré už máte, sa aktualizujú, nepridajú sa dvakrát.",
    de: "Wähle eine .vcf-Datei: vCard 4.0, 3.0 oder 2.1, mit einem Kontakt oder vielen. Kontakte, die schon in deinem Buch sind, werden aktualisiert, nicht doppelt hinzugefügt.",
  },
  "contacts.importDrop": {
    en: "Drop a .vcf file here, or choose one",
    sk: "Pustite sem súbor .vcf alebo ho vyberte",
    de: "Lege hier eine .vcf-Datei ab oder wähle eine aus",
  },
  "contacts.importChoose": { en: "Choose a file", sk: "Vybrať súbor", de: "Datei auswählen" },
  "contacts.importing": { en: "Importing {file}…", sk: "Importuje sa {file}…", de: "{file} wird importiert…" },
  "contacts.importDone": {
    en: "{added} added, {updated} updated",
    sk: "Pridané: {added}, aktualizované: {updated}",
    de: "{added} hinzugefügt, {updated} aktualisiert",
  },
  "contacts.importSkipped": {
    en: { one: "{count} card had no name, email or phone and was skipped.", other: "{count} cards had no name, email or phone and were skipped." },
    sk: { one: "{count} karta nemala meno, e-mail ani telefón a bola preskočená.", few: "{count} karty nemali meno, e-mail ani telefón a boli preskočené.", many: "{count} kariet nemalo meno, e-mail ani telefón a boli preskočené.", other: "{count} kariet nemalo meno, e-mail ani telefón a boli preskočené." },
    de: { one: "{count} Karte hatte weder Name, E-Mail noch Telefon und wurde übersprungen.", other: "{count} Karten hatten weder Name, E-Mail noch Telefon und wurden übersprungen." },
  },
  "contacts.importPhotos": {
    en: { one: "{count} photo could not be read; that contact came in without one.", other: "{count} photos could not be read; those contacts came in without one." },
    sk: { one: "{count} fotku sa nedalo prečítať; kontakt je bez fotky.", few: "{count} fotky sa nedali prečítať; tieto kontakty sú bez fotky.", many: "{count} fotiek sa nedalo prečítať; tieto kontakty sú bez fotky.", other: "{count} fotiek sa nedalo prečítať; tieto kontakty sú bez fotky." },
    de: { one: "{count} Foto konnte nicht gelesen werden; dieser Kontakt kam ohne Foto.", other: "{count} Fotos konnten nicht gelesen werden; diese Kontakte kamen ohne Foto." },
  },
  "contacts.importLeftOut": {
    en: { one: "{count} card was left out: an import takes at most 5000, and a book holds at most 10000.", other: "{count} cards were left out: an import takes at most 5000, and a book holds at most 10000." },
    sk: { one: "{count} karta sa vynechala: import vezme najviac 5000 a adresár drží najviac 10000.", few: "{count} karty sa vynechali: import vezme najviac 5000 a adresár drží najviac 10000.", many: "{count} kariet sa vynechalo: import vezme najviac 5000 a adresár drží najviac 10000.", other: "{count} kariet sa vynechalo: import vezme najviac 5000 a adresár drží najviac 10000." },
    de: { one: "{count} Karte wurde ausgelassen: ein Import nimmt höchstens 5000, ein Buch fasst höchstens 10000.", other: "{count} Karten wurden ausgelassen: ein Import nimmt höchstens 5000, ein Buch fasst höchstens 10000." },
  },
  "contacts.importClose": { en: "Done", sk: "Hotovo", de: "Fertig" },
  "contacts.exportTitle": { en: "Export contacts", sk: "Exportovať kontakty", de: "Kontakte exportieren" },
  "contacts.exportLead": {
    en: "Download your whole book as one .vcf file. vCard 3.0 imports on every phone; 4.0 is the newer format.",
    sk: "Stiahnite celý adresár ako jeden súbor .vcf. vCard 3.0 sa importuje na každom telefóne; 4.0 je novší formát.",
    de: "Lade dein ganzes Buch als eine .vcf-Datei herunter. vCard 3.0 lässt sich auf jedem Handy importieren; 4.0 ist das neuere Format.",
  },
  "contacts.export3": { en: "Download vCard 3.0", sk: "Stiahnuť vCard 3.0", de: "vCard 3.0 herunterladen" },
  "contacts.export4": { en: "Download vCard 4.0", sk: "Stiahnuť vCard 4.0", de: "vCard 4.0 herunterladen" },

  // ── people pickers in other apps ──
  "contacts.suggest.box": { en: "On this box", sk: "Na tomto boxe", de: "Auf dieser Box" },
  "contacts.suggest.contact": { en: "Contact", sk: "Kontakt", de: "Kontakt" },
  "contacts.suggest.label": { en: "Suggestions", sk: "Návrhy", de: "Vorschläge" },
});
