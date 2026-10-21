---
title: Kontakte
sidebar_position: 4.8
slug: /manual/contacts
---

# Kontakte

LosOS Contacts ist ein Adressbuch für jede Person auf der Box. **Kontakte**
in der Seitenleiste öffnet es unter `/contacts`. Dein Adressbuch gehört nur
dir. Die anderen Personen auf der Box, auch Administratoren, können es
weder über die Verwaltungsseiten noch über den Chat lesen.

## Wann es Kontakte gibt \{#when-contacts-is-there}

Kontakte bewahrt jedes Adressbuch auf dem Chat-Server der Box auf, also gibt
es Kontakte genau dann, wenn es den [Chat](chat) gibt: solange die Box ihren
Speicher mit einem Mesh teilt, dem sie beigetreten ist. Ist das Teilen aus,
steht **Kontakte** nicht in der Seitenleiste und `/contacts` öffnet sich
nicht. Die Adressbücher bleiben auf der Festplatte und sind wieder da, wie
sie waren, sobald die Box wieder teilt.

Der Ersatzschlüssel öffnet die Verwaltungsseiten, hat aber kein Adressbuch.
Kontakte bittet dich deshalb, dich mit deinem eigenen Konto anzumelden.

## Wo das Adressbuch liegt \{#where-the-book-is-kept}

Jedes Adressbuch ist ein privater Raum auf dem Chat-Server der Box, der
entsteht, wenn du Kontakte zum ersten Mal öffnest. Jeder Kontakt ist ein
Eintrag in diesem Raum, sein Foto wird auf ein kleines quadratisches Bild
verkleinert. Der Raum verlässt die Box nie. Er ist als nicht geteilt mit
anderen Chat-Servern markiert, niemand sonst ist eingeladen, und der Chat
zeigt ihn nicht unter deinen Unterhaltungen.

## Das Adressbuch \{#the-book}

- **Deine Kontakte** zeigt die Personen, die du hinzugefügt oder importiert
  hast, nach Namen, mit dem Anfangsbuchstaben als Überschrift. **Suchen**
  findet Namen, Spitznamen, Organisationen, E-Mail-Adressen,
  Telefonnummern und Chat-Adressen, mit oder ohne Akzente.
- **Auf dieser Box** zeigt alle anderen mit einem Konto auf der Box, damit
  du sie erreichst, ohne sie erst hinzuzufügen. **Zu Kontakten hinzufügen**
  übernimmt jemanden in dein Adressbuch.
- Ein Klick auf eine Person öffnet ihre Seite: E-Mail-Adressen,
  Telefonnummern, Chat-Adresse, Mesh-Adresse, Postanschriften, Geburtstag,
  Websites und Notizen. Jede Zeile hat einen Knopf **Kopieren**.
- Mit einem Rechtsklick auf eine Person in der Liste bekommst du dieselben
  Aktionen, ohne die Seite zu öffnen; siehe
  [das Rechtsklick-Menü](admin-pages#the-right-click-menu).

## Jemanden erreichen \{#reaching-someone}

Die Seite einer Person hat einen Knopf für jeden Weg, sie zu erreichen:

- **Mail** öffnet eine neue Nachricht an ihre erste Adresse. Den Knopf gibt
  es, wenn auf der Box [Mail](mail) läuft und der Kontakt eine
  E-Mail-Adresse hat.
- **Chat** öffnet deine Direktnachrichten mit ihr oder beginnt sie.
- **Anruf** und **Video** öffnen diese Unterhaltung und starten einen
  [Anruf](calls).

Chat, Anruf und Video brauchen die Chat-Adresse des Kontakts, zum Beispiel
`@eva:mattbox.local`. Alle auf dieser Box haben eine; bei einem Kontakt
trägst du sie unter **Chat (Matrix-ID)** ein.

Auch die anderen Apps nutzen das Adressbuch. Die Felder **An** und **Cc** in
Mail und **Jemandem schreiben** im Chat schlagen beim Tippen deine Kontakte
vor, neben den Personen auf der Box. Wer auf der Box ist und zugleich in
deinem Adressbuch steht, erscheint einmal, als dein Kontakt.

## Hinzufügen und bearbeiten \{#adding-and-editing}

**Neuer Kontakt** öffnet den Editor. Nur der Name ist Pflicht. **Weitere
Felder** ergänzt Spitzname, Position, Postanschriften, Websites, Geburtstag,
Mesh-Adresse (`name@box`, um Dateien mit jemandem auf einer anderen Box zu
teilen) und eine Notiz. Jede E-Mail-Adresse und Telefonnummer kannst du als
privat, Arbeit, mobil und so weiter kennzeichnen. **Foto hinzufügen** nimmt
ein Bild von deinem Computer.

## Importieren und exportieren \{#importing-and-exporting}

**Importieren** nimmt eine `.vcf`-Datei von deinem Handy oder aus einem
anderen Adressbuch: vCard 4.0, 3.0 oder 2.1, mit einem Kontakt oder vielen.
Fotos in der Datei kommen mit. Ein Kontakt, der schon in deinem Adressbuch
steht, wird aktualisiert statt ein zweites Mal angelegt. Es entscheidet die
eigene Kennung der Datei, wenn sie eine hat, sonst der Name zusammen mit der
ersten E-Mail-Adresse oder Telefonnummer.

**Exportieren** lädt dein ganzes Adressbuch als eine `.vcf`-Datei herunter,
in vCard 3.0 (das jedes Handy liest) oder 4.0. Die Seite einer Person und
das Rechtsklick-Menü exportieren auch einen einzelnen Kontakt.

## Was Kontakte noch nicht kann \{#what-contacts-does-not-do-yet}

Kontakte hat kein CardDAV, ein Handy oder ein Adressbuch auf dem Computer
kann sich also nicht direkt damit abgleichen. Kontakte bringst du mit einem
Export auf der einen und einem Import auf der anderen Seite hinüber.
