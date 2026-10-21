---
title: Kontakte
sidebar_position: 4.8
slug: /manual/contacts
---

# Kontakte

LosOS Contacts ist ein Adressbuch für jede Person auf der Box. **Kontakte**
in der Seitenleiste öffnet es unter `/contacts`. Dein Adressbuch gehört nur
dir. Die anderen Personen auf der Box, auch Administratoren, können es
weder über die Verwaltungsseiten noch über LosOS Cloud lesen.

## Wann es Kontakte gibt \{#when-contacts-is-there}

Kontakte bewahrt deine Kontakte in [LosOS Cloud](losos-cloud) auf, also gibt
es Kontakte auf jeder Box, auf der LosOS Cloud läuft, ob die Box ihren
Speicher teilt oder nicht. [Mail](mail) und [Chat](chat) laufen nur,
solange die Box mit einem Mesh teilt, dem sie beigetreten ist. Ist das
Teilen aus, hat die Seite einer Person keinen Knopf **Mail**, **Chat**,
**Anruf** oder **Video**, und das Adressbuch bleibt, wie es ist.

Der Ersatzschlüssel öffnet die Verwaltungsseiten, hat aber kein Adressbuch.
Kontakte bittet dich deshalb, dich mit deinem eigenen Konto anzumelden.

## Wo das Adressbuch liegt \{#where-the-book-is-kept}

Deine Kontakte sind die Adressbücher deines Kontos in LosOS Cloud, eine
vCard pro Kontakt. Kontakte zeigt die Personen aus all deinen
Adressbüchern, auch aus einem, das ein Handy oder Thunderbird angelegt hat.
Ein Kontakt, den du hier hinzufügst, kommt in dein erstes Adressbuch,
**Contacts**. Wenn du einen Kontakt bearbeitest, ändert Kontakte die Felder,
die es zeigt, und lässt den Rest der Karte so, wie dein Handy ihn
geschrieben hat, etwa Gruppen, Klingeltöne und Verwandte. Fotos bleiben in
der Karte, und die Seite zeigt sie als kleines quadratisches Bild.

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

Chat, Anruf und Video gibt es, wenn auf der Box der Chat läuft, und sie brauchen die Chat-Adresse des Kontakts, zum Beispiel
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

## Auf dem Handy und dem Computer \{#on-your-phone-and-computer}

Handys und Adressbücher auf dem Computer gleichen sich über CardDAV mit
denselben Adressbüchern ab. Richte sie ein wie unter
[Kalender und Kontakte](losos-cloud#calendars-and-contacts). Ein Kontakt,
den du auf dem Handy hinzufügst, erscheint in Kontakte, und eine Änderung in
Kontakte kommt beim nächsten Abgleich aufs Handy. Import und Export bleiben
für einen einmaligen Umzug von woanders.
