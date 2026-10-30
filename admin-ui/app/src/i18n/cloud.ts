import { defineMessages } from "./define";

/* LosOS Cloud's web app (src/cloud/), served at /cloud/: the places, the
 * file list and what can be done with it, uploads, trash, devices and the
 * page the desktop and phone apps open to sign in. */

export default defineMessages({
  "cloud.cancel": { en: "Cancel", sk: "Zrušiť", de: "Abbrechen" },
  "cloud.close": { en: "Close", sk: "Zavrieť", de: "Schließen" },
  "cloud.download": { en: "Download", sk: "Stiahnuť", de: "Herunterladen" },
  "cloud.crumbs": { en: "Folder path", sk: "Cesta k priečinku", de: "Ordnerpfad" },

  // ── Places ────────────────────────────────────────────────────────────
  "cloud.nav.files": { en: "Files", sk: "Súbory", de: "Dateien" },
  "cloud.nav.recent": { en: "Recent", sk: "Nedávne", de: "Zuletzt" },
  "cloud.nav.favorites": { en: "Favorites", sk: "Obľúbené", de: "Favoriten" },
  "cloud.nav.photos": { en: "Photos", sk: "Fotky", de: "Fotos" },
  "cloud.nav.trash": { en: "Trash", sk: "Kôš", de: "Papierkorb" },
  "cloud.nav.devices": { en: "Devices", sk: "Zariadenia", de: "Geräte" },
  "cloud.usage.label": { en: "Storage used", sk: "Využité miesto", de: "Belegter Speicher" },
  "cloud.usage.text": {
    en: "{used} used, {free} free",
    sk: "Využité {used}, voľné {free}",
    de: "{used} belegt, {free} frei",
  },
  "cloud.notFound.title": {
    en: "There is nothing at this address.",
    sk: "Na tejto adrese nič nie je.",
    de: "Unter dieser Adresse gibt es nichts.",
  },

  // ── Search ────────────────────────────────────────────────────────────
  "cloud.search.title": { en: "Search", sk: "Hľadanie", de: "Suche" },
  "cloud.search.for": { en: "Results for \"{text}\"", sk: "Výsledky pre „{text}“", de: "Ergebnisse für „{text}“" },
  "cloud.search.emptyTitle": { en: "Nothing found", sk: "Nič sa nenašlo", de: "Nichts gefunden" },
  "cloud.search.emptyBody": {
    en: "No file or folder name contains \"{text}\".",
    sk: "Žiadny súbor ani priečinok nemá v názve „{text}“.",
    de: "Kein Datei- oder Ordnername enthält „{text}“.",
  },

  // ── The list ──────────────────────────────────────────────────────────
  "cloud.col.name": { en: "Name", sk: "Názov", de: "Name" },
  "cloud.col.folder": { en: "Folder", sk: "Priečinok", de: "Ordner" },
  "cloud.col.size": { en: "Size", sk: "Veľkosť", de: "Größe" },
  "cloud.col.modified": { en: "Modified", sk: "Zmenené", de: "Geändert" },
  "cloud.selectAll": { en: "Select all", sk: "Vybrať všetko", de: "Alle auswählen" },
  "cloud.select": { en: "Select {name}", sk: "Vybrať {name}", de: "{name} auswählen" },
  "cloud.when.today": { en: "Today {time}", sk: "Dnes {time}", de: "Heute {time}" },
  "cloud.when.yesterday": { en: "Yesterday {time}", sk: "Včera {time}", de: "Gestern {time}" },
  "cloud.favorite.add": { en: "Add to favorites", sk: "Pridať do obľúbených", de: "Zu Favoriten hinzufügen" },
  "cloud.favorite.remove": { en: "Remove from favorites", sk: "Odobrať z obľúbených", de: "Aus Favoriten entfernen" },

  // ── A folder ──────────────────────────────────────────────────────────
  "cloud.folder.new": { en: "New folder", sk: "Nový priečinok", de: "Neuer Ordner" },
  "cloud.folder.newTitle": { en: "New folder", sk: "Nový priečinok", de: "Neuer Ordner" },
  "cloud.folder.create": { en: "Create", sk: "Vytvoriť", de: "Erstellen" },
  "cloud.folder.emptyTitle": { en: "This folder is empty", sk: "Tento priečinok je prázdny", de: "Dieser Ordner ist leer" },
  "cloud.folder.emptyBody": {
    en: "Drop files here, or upload them from your computer.",
    sk: "Pretiahnite sem súbory alebo ich nahrajte z počítača.",
    de: "Zieh Dateien hierher oder lade sie vom Computer hoch.",
  },
  "cloud.folder.missing": {
    en: "This folder is not there any more.",
    sk: "Tento priečinok už neexistuje.",
    de: "Diesen Ordner gibt es nicht mehr.",
  },
  "cloud.folder.home": { en: "Go to your files", sk: "Prejsť na súbory", de: "Zu deinen Dateien" },
  "cloud.name.label": { en: "Name", sk: "Názov", de: "Name" },

  // ── Uploads ───────────────────────────────────────────────────────────
  "cloud.upload.files": { en: "Upload", sk: "Nahrať", de: "Hochladen" },
  "cloud.upload.folder": { en: "Upload folder", sk: "Nahrať priečinok", de: "Ordner hochladen" },
  "cloud.upload.drop": { en: "Drop to upload to {folder}", sk: "Pustite a nahrá sa do {folder}", de: "Loslassen zum Hochladen nach {folder}" },
  "cloud.upload.panel": { en: "Uploads", sk: "Nahrávanie", de: "Uploads" },
  "cloud.upload.sending": {
    en: { one: "Uploading {count} file", other: "Uploading {count} files" },
    sk: { one: "Nahráva sa {count} súbor", few: "Nahrávajú sa {count} súbory", many: "Nahráva sa {count} súborov", other: "Nahráva sa {count} súborov" },
    de: { one: "{count} Datei wird hochgeladen", other: "{count} Dateien werden hochgeladen" },
  },
  "cloud.upload.finished": { en: "Uploads finished", sk: "Nahrávanie skončilo", de: "Uploads abgeschlossen" },
  "cloud.upload.waiting": { en: "Waiting", sk: "Čaká", de: "Wartet" },
  "cloud.upload.done": { en: "Uploaded", sk: "Nahrané", de: "Hochgeladen" },
  "cloud.upload.cancel": { en: "Cancel {name}", sk: "Zrušiť {name}", de: "{name} abbrechen" },
  "cloud.upload.retry": { en: "Try again", sk: "Skúsiť znova", de: "Erneut versuchen" },
  "cloud.upload.exists": {
    en: "A file with this name is already there.",
    sk: "Súbor s týmto názvom tu už je.",
    de: "Eine Datei mit diesem Namen ist schon da.",
  },
  "cloud.upload.replace": { en: "Replace", sk: "Nahradiť", de: "Ersetzen" },
  "cloud.upload.keepBoth": { en: "Keep both", sk: "Ponechať oba", de: "Beide behalten" },
  "cloud.upload.skip": { en: "Skip", sk: "Preskočiť", de: "Überspringen" },

  // ── What can be done with the picked rows ─────────────────────────────
  "cloud.selection.label": { en: "Selected items", sk: "Vybrané položky", de: "Ausgewählte Elemente" },
  "cloud.selection.clear": { en: "Clear the selection", sk: "Zrušiť výber", de: "Auswahl aufheben" },
  "cloud.selection.count": {
    en: { one: "{count} selected", other: "{count} selected" },
    sk: { one: "{count} vybraná", few: "{count} vybrané", many: "{count} vybraných", other: "{count} vybraných" },
    de: { one: "{count} ausgewählt", other: "{count} ausgewählt" },
  },
  "cloud.rename.action": { en: "Rename", sk: "Premenovať", de: "Umbenennen" },
  "cloud.rename.title": { en: "Rename {name}", sk: "Premenovať {name}", de: "{name} umbenennen" },
  "cloud.rename.submit": { en: "Rename", sk: "Premenovať", de: "Umbenennen" },
  "cloud.move.action": { en: "Move", sk: "Presunúť", de: "Verschieben" },
  "cloud.move.title": {
    en: { one: "Move {count} item to", other: "Move {count} items to" },
    sk: { one: "Presunúť {count} položku do", few: "Presunúť {count} položky do", many: "Presunúť {count} položiek do", other: "Presunúť {count} položiek do" },
    de: { one: "{count} Element verschieben nach", other: "{count} Elemente verschieben nach" },
  },
  "cloud.move.here": { en: "Move here", sk: "Presunúť sem", de: "Hierher verschieben" },
  "cloud.move.noFolders": { en: "No folders in here.", sk: "Nie sú tu žiadne priečinky.", de: "Hier sind keine Ordner." },
  "cloud.move.done": {
    en: { one: "Moved {count} item to {folder}", other: "Moved {count} items to {folder}" },
    sk: { one: "{count} položka presunutá do {folder}", few: "{count} položky presunuté do {folder}", many: "{count} položiek presunutých do {folder}", other: "{count} položiek presunutých do {folder}" },
    de: { one: "{count} Element nach {folder} verschoben", other: "{count} Elemente nach {folder} verschoben" },
  },
  "cloud.move.failed": { en: "{name} was not moved", sk: "{name} sa nepresunul", de: "{name} wurde nicht verschoben" },
  "cloud.copy.action": { en: "Copy", sk: "Kopírovať", de: "Kopieren" },
  "cloud.copy.title": {
    en: { one: "Copy {count} item to", other: "Copy {count} items to" },
    sk: { one: "Kopírovať {count} položku do", few: "Kopírovať {count} položky do", many: "Kopírovať {count} položiek do", other: "Kopírovať {count} položiek do" },
    de: { one: "{count} Element kopieren nach", other: "{count} Elemente kopieren nach" },
  },
  "cloud.copy.here": { en: "Copy here", sk: "Kopírovať sem", de: "Hierher kopieren" },
  "cloud.copy.done": {
    en: { one: "Copied {count} item to {folder}", other: "Copied {count} items to {folder}" },
    sk: { one: "{count} položka skopírovaná do {folder}", few: "{count} položky skopírované do {folder}", many: "{count} položiek skopírovaných do {folder}", other: "{count} položiek skopírovaných do {folder}" },
    de: { one: "{count} Element nach {folder} kopiert", other: "{count} Elemente nach {folder} kopiert" },
  },
  "cloud.copy.failed": { en: "{name} was not copied", sk: "{name} sa neskopíroval", de: "{name} wurde nicht kopiert" },
  "cloud.delete.action": { en: "Delete", sk: "Odstrániť", de: "Löschen" },
  "cloud.delete.done": {
    en: { one: "Moved {count} item to the trash", other: "Moved {count} items to the trash" },
    sk: { one: "{count} položka presunutá do koša", few: "{count} položky presunuté do koša", many: "{count} položiek presunutých do koša", other: "{count} položiek presunutých do koša" },
    de: { one: "{count} Element in den Papierkorb verschoben", other: "{count} Elemente in den Papierkorb verschoben" },
  },
  "cloud.delete.where": {
    en: "Trash in the sidebar brings it back.",
    sk: "Kôš v bočnom paneli ho vráti späť.",
    de: "Über den Papierkorb in der Seitenleiste holst du es zurück.",
  },
  "cloud.delete.failed": { en: "Not everything was deleted", sk: "Nie všetko sa odstránilo", de: "Nicht alles wurde gelöscht" },

  // ── Versions ──────────────────────────────────────────────────────────
  "cloud.versions.action": { en: "Versions", sk: "Verzie", de: "Versionen" },
  "cloud.versions.title": { en: "Versions of {name}", sk: "Verzie súboru {name}", de: "Versionen von {name}" },
  "cloud.versions.summary": {
    en: "Each time the file changes, LosOS Cloud keeps the content from before here, up to the 20 newest.",
    sk: "Pri každej zmene súboru sa sem odloží predchádzajúci obsah. Uchováva sa 20 najnovších.",
    de: "Bei jeder Änderung wird der vorherige Inhalt hier aufbewahrt. Die 20 neuesten bleiben.",
  },
  "cloud.versions.current": { en: "Current version", sk: "Aktuálna verzia", de: "Aktuelle Version" },
  "cloud.versions.none": { en: "No earlier versions yet.", sk: "Zatiaľ žiadne staršie verzie.", de: "Noch keine früheren Versionen." },
  "cloud.versions.restore": { en: "Restore", sk: "Obnoviť", de: "Wiederherstellen" },
  "cloud.versions.restored": { en: "Restored the version from {when}", sk: "Obnovená verzia z {when}", de: "Version vom {when} wiederhergestellt" },
  "cloud.versions.failed": { en: "The version was not restored", sk: "Verzia sa neobnovila", de: "Die Version wurde nicht wiederhergestellt" },

  // ── Empty places ──────────────────────────────────────────────────────
  "cloud.recent.emptyTitle": { en: "Nothing changed yet", sk: "Zatiaľ sa nič nezmenilo", de: "Noch nichts geändert" },
  "cloud.recent.emptyBody": {
    en: "Files you add or change show up here, newest first.",
    sk: "Súbory, ktoré pridáte alebo zmeníte, sa tu zobrazia od najnovších.",
    de: "Dateien, die du hinzufügst oder änderst, erscheinen hier, die neuesten zuerst.",
  },
  "cloud.favorites.emptyTitle": { en: "No favorites yet", sk: "Zatiaľ žiadne obľúbené", de: "Noch keine Favoriten" },
  "cloud.favorites.emptyBody": {
    en: "Star a file or folder in its row and it stays here.",
    sk: "Označte súbor alebo priečinok hviezdičkou v jeho riadku a zostane tu.",
    de: "Markiere eine Datei oder einen Ordner in seiner Zeile mit dem Stern, dann bleibt er hier.",
  },
  "cloud.photos.emptyTitle": { en: "No photos yet", sk: "Zatiaľ žiadne fotky", de: "Noch keine Fotos" },
  "cloud.photos.emptyBody": {
    en: "Pictures in any folder show up here, by month. The phone app can upload new ones as you take them.",
    sk: "Obrázky z ľubovoľného priečinka sa tu zobrazia podľa mesiacov. Aplikácia v telefóne môže nové nahrávať hneď po odfotení.",
    de: "Bilder aus jedem Ordner erscheinen hier, nach Monaten. Die Handy-App kann neue gleich nach der Aufnahme hochladen.",
  },
  "cloud.photos.previous": { en: "Previous photo", sk: "Predchádzajúca fotka", de: "Vorheriges Foto" },
  "cloud.photos.next": { en: "Next photo", sk: "Ďalšia fotka", de: "Nächstes Foto" },
  "cloud.photos.groupBy": { en: "Group by", sk: "Zoskupiť podľa", de: "Gruppieren nach" },
  "cloud.photos.by.date": { en: "Date", sk: "Dátumu", de: "Datum" },
  "cloud.photos.by.person": { en: "Person", sk: "Osoby", de: "Person" },
  "cloud.photos.by.camera": { en: "Camera", sk: "Fotoaparátu", de: "Kamera" },
  "cloud.photos.by.place": { en: "Place", sk: "Miesta", de: "Ort" },
  "cloud.photos.people": { en: "People", sk: "Ľudia", de: "Personen" },
  "cloud.photos.addName": { en: "Add a name", sk: "Pridať meno", de: "Namen hinzufügen" },
  "cloud.photos.unnamed": { en: "Someone", sk: "Niekto", de: "Jemand" },
  "cloud.photos.allPhotos": { en: "All photos", sk: "Všetky fotky", de: "Alle Fotos" },
  "cloud.photos.name": { en: "Name", sk: "Meno", de: "Name" },
  "cloud.photos.namePlaceholder": { en: "Who is this?", sk: "Kto je to?", de: "Wer ist das?" },
  "cloud.photos.nameHint": {
    en: "Give two people the same name and they become one.",
    sk: "Ak dvom osobám dáte rovnaké meno, stanú sa jednou.",
    de: "Gibst du zwei Personen denselben Namen, werden sie zu einer.",
  },
  "cloud.photos.count": {
    en: { one: "{count} photo", other: "{count} photos" },
    sk: { one: "{count} fotka", few: "{count} fotky", many: "{count} fotiek", other: "{count} fotiek" },
    de: { one: "{count} Foto", other: "{count} Fotos" },
  },
  "cloud.photos.noOne": { en: "No one", sk: "Nikto", de: "Niemand" },
  "cloud.photos.noCamera": { en: "Unknown camera", sk: "Neznámy fotoaparát", de: "Unbekannte Kamera" },
  "cloud.photos.noPlace": { en: "No location", sk: "Bez polohy", de: "Ohne Ort" },
  "cloud.photos.reading": {
    en: "Reading your photos on this box: {done} of {total}",
    sk: "Vaše fotky sa čítajú na tomto zariadení: {done} z {total}",
    de: "Deine Fotos werden auf dieser Box gelesen: {done} von {total}",
  },
  "cloud.photos.private": {
    en: "Faces and places are found on this box and stay on it. Only you see them.",
    sk: "Tváre a miesta sa hľadajú na tomto zariadení a zostávajú na ňom. Vidíte ich len vy.",
    de: "Gesichter und Orte werden auf dieser Box erkannt und bleiben dort. Nur du siehst sie.",
  },
  "cloud.photos.details": { en: "Details", sk: "Podrobnosti", de: "Details" },
  "cloud.photos.taken": { en: "Taken", sk: "Odfotené", de: "Aufgenommen" },
  "cloud.photos.changed": { en: "Changed", sk: "Zmenené", de: "Geändert" },
  "cloud.photos.camera": { en: "Camera", sk: "Fotoaparát", de: "Kamera" },
  "cloud.photos.lens": { en: "Lens", sk: "Objektív", de: "Objektiv" },
  "cloud.photos.settings": { en: "Settings", sk: "Nastavenia", de: "Einstellungen" },
  "cloud.photos.size": { en: "Size", sk: "Veľkosť", de: "Größe" },
  "cloud.photos.place": { en: "Place", sk: "Miesto", de: "Ort" },
  "cloud.photos.altitude": { en: "{m} m above sea level", sk: "{m} m nad morom", de: "{m} m über dem Meer" },
  "cloud.photos.inPhoto": { en: "In this photo", sk: "Na fotke", de: "Auf diesem Foto" },
  "cloud.photos.allTags": {
    en: { one: "{count} tag from the camera", other: "All {count} tags from the camera" },
    sk: { one: "{count} údaj z fotoaparátu", few: "Všetky {count} údaje z fotoaparátu", many: "Všetkých {count} údajov z fotoaparátu", other: "Všetkých {count} údajov z fotoaparátu" },
    de: { one: "{count} Angabe der Kamera", other: "Alle {count} Angaben der Kamera" },
  },
  "cloud.photos.group.picture": { en: "Picture", sk: "Obrázok", de: "Bild" },
  "cloud.photos.group.camera": { en: "Camera", sk: "Fotoaparát", de: "Kamera" },
  "cloud.photos.group.place": { en: "Place", sk: "Miesto", de: "Ort" },
  "cloud.photos.group.other": { en: "Other", sk: "Ostatné", de: "Sonstiges" },
  "cloud.photos.notRead": {
    en: "This photo has not been read yet.",
    sk: "Táto fotka ešte nebola prečítaná.",
    de: "Dieses Foto wurde noch nicht gelesen.",
  },
  "cloud.photos.noTags": {
    en: "The camera wrote nothing else about this photo.",
    sk: "Fotoaparát o tejto fotke nič ďalšie nezapísal.",
    de: "Die Kamera hat zu diesem Foto nichts weiter gespeichert.",
  },

  // ── Trash ─────────────────────────────────────────────────────────────
  "cloud.trash.summary": {
    en: { one: "Deleted files stay here for {days} day, then go for good.", other: "Deleted files stay here for {days} days, then go for good." },
    sk: { one: "Odstránené súbory tu zostanú {days} deň, potom zmiznú natrvalo.", few: "Odstránené súbory tu zostanú {days} dni, potom zmiznú natrvalo.", many: "Odstránené súbory tu zostanú {days} dní, potom zmiznú natrvalo.", other: "Odstránené súbory tu zostanú {days} dní, potom zmiznú natrvalo." },
    de: { one: "Gelöschte Dateien bleiben {days} Tag hier und sind dann endgültig weg.", other: "Gelöschte Dateien bleiben {days} Tage hier und sind dann endgültig weg." },
  },
  "cloud.trash.from": { en: "Was in", sk: "Bolo v", de: "War in" },
  "cloud.trash.deleted": { en: "Deleted", sk: "Odstránené", de: "Gelöscht" },
  "cloud.trash.actions": { en: "Actions", sk: "Akcie", de: "Aktionen" },
  "cloud.trash.restore": { en: "Restore", sk: "Obnoviť", de: "Wiederherstellen" },
  "cloud.trash.restored": { en: "Restored {name}", sk: "{name} obnovené", de: "{name} wiederhergestellt" },
  "cloud.trash.restoreFailed": { en: "{name} was not restored", sk: "{name} sa neobnovilo", de: "{name} wurde nicht wiederhergestellt" },
  "cloud.trash.purge": { en: "Delete {name} for good", sk: "Natrvalo odstrániť {name}", de: "{name} endgültig löschen" },
  "cloud.trash.purgeTitle": { en: "Delete {name} for good?", sk: "Natrvalo odstrániť {name}?", de: "{name} endgültig löschen?" },
  "cloud.trash.purgeBody": {
    en: "It cannot be brought back afterwards.",
    sk: "Potom sa už nedá vrátiť.",
    de: "Danach lässt es sich nicht mehr zurückholen.",
  },
  "cloud.trash.purgeConfirm": { en: "Delete for good", sk: "Odstrániť natrvalo", de: "Endgültig löschen" },
  "cloud.trash.purgeFailed": { en: "It was not deleted", sk: "Neodstránilo sa", de: "Es wurde nicht gelöscht" },
  "cloud.trash.empty": { en: "Empty the trash", sk: "Vyprázdniť kôš", de: "Papierkorb leeren" },
  "cloud.trash.emptyConfirmTitle": {
    en: { one: "Delete {count} item for good?", other: "Delete {count} items for good?" },
    sk: { one: "Natrvalo odstrániť {count} položku?", few: "Natrvalo odstrániť {count} položky?", many: "Natrvalo odstrániť {count} položiek?", other: "Natrvalo odstrániť {count} položiek?" },
    de: { one: "{count} Element endgültig löschen?", other: "{count} Elemente endgültig löschen?" },
  },
  "cloud.trash.emptyTitle": { en: "The trash is empty", sk: "Kôš je prázdny", de: "Der Papierkorb ist leer" },
  "cloud.trash.emptyBody": {
    en: "Files you delete wait here before they go.",
    sk: "Súbory, ktoré odstránite, tu počkajú, kým zmiznú.",
    de: "Gelöschte Dateien warten hier, bevor sie verschwinden.",
  },

  // ── Devices ───────────────────────────────────────────────────────────
  "cloud.devices.addTitle": { en: "Sync a computer or phone", sk: "Synchronizovať počítač alebo telefón", de: "Computer oder Handy synchronisieren" },
  "cloud.devices.addBody": {
    en: "Install the Nextcloud app on the computer or phone and give it this server address. It opens a page here to sign in.",
    sk: "Nainštalujte do počítača alebo telefónu aplikáciu Nextcloud a zadajte jej túto adresu servera. Na prihlásenie otvorí stránku tu.",
    de: "Installiere die Nextcloud-App auf dem Computer oder Handy und gib diese Serveradresse an. Zum Anmelden öffnet sie hier eine Seite.",
  },
  "cloud.devices.server": { en: "Server address", sk: "Adresa servera", de: "Serveradresse" },
  "cloud.devices.webdav": { en: "WebDAV address, for a file manager", sk: "Adresa WebDAV pre správcu súborov", de: "WebDAV-Adresse für einen Dateimanager" },
  "cloud.devices.copy": { en: "Copy the {what}", sk: "Kopírovať: {what}", de: "{what} kopieren" },
  "cloud.devices.copied": { en: "Copied", sk: "Skopírované", de: "Kopiert" },
  "cloud.devices.listTitle": { en: "Signed in", sk: "Prihlásené", de: "Angemeldet" },
  "cloud.devices.none": {
    en: "No computer or phone syncs yet.",
    sk: "Zatiaľ sa nesynchronizuje žiadny počítač ani telefón.",
    de: "Noch synchronisiert kein Computer und kein Handy.",
  },
  "cloud.devices.lastUsed": { en: "Last synced {when}", sk: "Naposledy synchronizované {when}", de: "Zuletzt synchronisiert {when}" },
  "cloud.devices.added": { en: "Added {when}", sk: "Pridané {when}", de: "Hinzugefügt {when}" },
  "cloud.devices.revoke": { en: "Sign out", sk: "Odhlásiť", de: "Abmelden" },
  "cloud.devices.revokeTitle": { en: "Sign out {name}?", sk: "Odhlásiť {name}?", de: "{name} abmelden?" },
  "cloud.devices.revokeBody": {
    en: "It stops syncing at once. To sync again it signs in anew.",
    sk: "Hneď prestane synchronizovať. Ak má znova synchronizovať, musí sa prihlásiť nanovo.",
    de: "Die Synchronisierung endet sofort. Um wieder zu synchronisieren, meldet es sich neu an.",
  },
  "cloud.devices.revoked": { en: "Signed out {name}", sk: "{name} odhlásené", de: "{name} abgemeldet" },
  "cloud.devices.revokeFailed": { en: "It was not signed out", sk: "Neodhlásilo sa", de: "Es wurde nicht abgemeldet" },

  // ── The apps' sign-in page ────────────────────────────────────────────
  "cloud.connect.title": { en: "Connect an app", sk: "Pripojiť aplikáciu", de: "App verbinden" },
  "cloud.connect.body": {
    en: "{client} asks to sync the files of {name}.",
    sk: "{client} žiada o synchronizáciu súborov používateľa {name}.",
    de: "{client} möchte die Dateien von {name} synchronisieren.",
  },
  "cloud.connect.note": {
    en: "It gets a password of its own, which Devices can take back at any time.",
    sk: "Dostane vlastné heslo, ktoré mu môžete kedykoľvek odobrať v časti Zariadenia.",
    de: "Sie bekommt ein eigenes Passwort, das du unter Geräte jederzeit zurücknehmen kannst.",
  },
  "cloud.connect.allow": { en: "Allow", sk: "Povoliť", de: "Erlauben" },
  "cloud.connect.deny": { en: "Not now", sk: "Teraz nie", de: "Jetzt nicht" },
  "cloud.connect.doneTitle": { en: "Connected", sk: "Pripojené", de: "Verbunden" },
  "cloud.connect.doneBody": {
    en: "Go back to the app; it carries on by itself. This tab can be closed.",
    sk: "Vráťte sa do aplikácie, pokračuje sama. Túto kartu môžete zavrieť.",
    de: "Kehr zur App zurück, sie macht von selbst weiter. Dieser Tab kann geschlossen werden.",
  },
  "cloud.connect.declinedTitle": { en: "Not connected", sk: "Nepripojené", de: "Nicht verbunden" },
  "cloud.connect.declinedBody": {
    en: "The app got nothing. This tab can be closed.",
    sk: "Aplikácia nedostala nič. Túto kartu môžete zavrieť.",
    de: "Die App hat nichts bekommen. Dieser Tab kann geschlossen werden.",
  },
  "cloud.connect.goneTitle": { en: "This link has run out", sk: "Tento odkaz už vypršal", de: "Dieser Link ist abgelaufen" },
  "cloud.connect.goneBody": {
    en: "Start the sign-in again from the app.",
    sk: "Spustite prihlásenie v aplikácii znova.",
    de: "Starte die Anmeldung in der App erneut.",
  },
  "cloud.connect.errorTitle": { en: "Something went wrong", sk: "Niečo sa pokazilo", de: "Etwas ist schiefgelaufen" },

  // ── Refusals ──────────────────────────────────────────────────────────
  "cloud.error.exists": {
    en: "Something with this name is already there.",
    sk: "Niečo s týmto názvom tu už je.",
    de: "Etwas mit diesem Namen ist schon da.",
  },
  "cloud.error.conflict": {
    en: "The folder it goes into is not there any more.",
    sk: "Priečinok, do ktorého to patrí, už neexistuje.",
    de: "Der Zielordner existiert nicht mehr.",
  },
  "cloud.error.badName": {
    en: "A name cannot contain / or be . or ..",
    sk: "Názov nemôže obsahovať / ani byť . alebo ..",
    de: "Ein Name darf kein / enthalten und nicht . oder .. sein.",
  },
  "cloud.error.loop": {
    en: "A folder cannot go inside itself.",
    sk: "Priečinok nemôže ísť sám do seba.",
    de: "Ein Ordner kann nicht in sich selbst.",
  },
  "cloud.error.full": { en: "The box is out of room.", sk: "Na zariadení už nie je miesto.", de: "Auf der Box ist kein Platz mehr." },
  "cloud.error.notFound": {
    en: "It is not there any more.",
    sk: "Už to tam nie je.",
    de: "Es ist nicht mehr da.",
  },
  "cloud.error.network": {
    en: "The box did not answer. Check the connection.",
    sk: "Zariadenie neodpovedá. Skontrolujte pripojenie.",
    de: "Die Box antwortet nicht. Prüf die Verbindung.",
  },
  "cloud.error.other": { en: "The box said: {message}", sk: "Zariadenie odpovedalo: {message}", de: "Die Box meldete: {message}" },
});
