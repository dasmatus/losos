---
title: Kalender
sidebar_position: 4.85
slug: /manual/calendar
---

# Kalender

![Eine Woche in LosOS Calendar: links deine Kalender und zwei Abos, ein Stundenplan und eine Steuerfrist aus den Abos und Review the box, ein Termin mit Videoanruf in LosOS Chat.](@site/docs/img/calendar-week-de.png)

LosOS Calendar bewahrt deine Termine und die Kalender auf, denen du folgst.
**Kalender** in der Seitenleiste öffnet ihn unter `/calendar`. Deine
Kalender gehören nur dir. Die anderen Personen auf der Box, auch
Administratoren, können sie nicht lesen.

## Wann es den Kalender gibt \{#when-calendar-is-there}

Der Kalender bewahrt seine Termine in [LosOS Cloud](losos-cloud) auf, also
gibt es ihn auf jeder Box, auf der LosOS Cloud läuft, ob die Box ihren
Speicher teilt oder nicht. Einladungen per [Mail](mail) und Anrufe in
[Chat](chat) brauchen diese Apps, die nur laufen, solange die Box mit einem
Mesh teilt, dem sie beigetreten ist. Ohne sie hat ein Termin keinen Anruf
und keine Einladung, alles andere bleibt gleich.

Der Ersatzschlüssel öffnet die Verwaltungsseiten, hat aber keinen Kalender.
Der Kalender bittet dich deshalb, dich mit deinem eigenen Konto anzumelden.

## Die Ansichten \{#the-views}

- **Woche** zeigt die Tage von Montag bis Sonntag nach Stunden, die
  ganztägigen Termine darüber und eine Linie bei der aktuellen Uhrzeit. Ein
  Doppelklick auf eine freie Stunde beginnt dort einen Termin.
- **Monat** zeigt sechs Wochen. Ein Tag mit mehr Terminen, als hineinpassen,
  zeigt, wie viele es noch sind; ein Klick auf den Tag öffnet seine Woche.
- **Liste** zeigt die nächsten sechs Wochen als Liste, Tag für Tag.

**Heute** und die Pfeile daneben blättern durch die Wochen oder Monate.

Die Spalte links listet **Deine Kalender** und **Abonnements**. Ein Klick
auf die Farbe eines Kalenders blendet seine Termine aus oder wieder ein. Die
Seite merkt sich, welche du ausgeblendet hast. Der Knopf **…** neben einem
Kalender öffnet seine Einstellungen.

Die Uhrzeiten stehen in der Zeitzone deines Browsers. Ein Termin aus einer
anderen Zone, etwa eine Einladung aus dem Ausland, behält seine eigene und
erscheint zur richtigen Zeit in deiner.

## Termine \{#events}

**Neuer Termin** öffnet den Editor. Nur der Titel ist Pflicht. Ein Termin
kann:

- den ganzen Tag dauern oder mehrere Tage;
- sich jeden Tag, jede Woche, jeden Monat oder jedes Jahr wiederholen, an
  gewählten Wochentagen und bis zu einem letzten Tag. Eine Wiederholung vom
  Handy, die der Editor nicht zeigen kann, etwa „der letzte Freitag im
  Monat“, bleibt, wie sie war, bis du eine andere wählst;
- **Gäste** haben, die du beim Tippen aus deinen [Kontakten](contacts) und
  den Personen auf der Box auswählst;
- ein **Videoanruf** in [Chat](chat) sein, Ende-zu-Ende-verschlüsselt;
- eine **Erinnerung** vor dem Beginn haben;
- einen Ort und Notizen haben.

Ein Klick auf einen Termin öffnet ihn. Der Termin zeigt seine Gäste und ob
jeder kommt, mit einem Knopf **Chat** für jeden Gast mit einer
Chat-Adresse. Ein Anruf hat **Anruf beitreten**, das ihn in Chat öffnet.
**Löschen** fragt bei einem wiederkehrenden Termin, ob nur dieser Tag oder
alle Tage gelöscht werden.

Hat ein Handy den Termin geändert, während du ihn bearbeitet hast, sagt das
Speichern das, statt die Änderung des Handys zu überschreiben. Schließ den
Editor und öffne den Termin erneut, um zu sehen, was sich geändert hat.

![Ein geöffneter Termin: Review the box, mit seinem Videoanruf in LosOS Chat und Anruf beitreten, dem Kalender, der Erinnerung, Matus als Organisator und Eva, die kommt, und unten Gästen schreiben und Einladungen senden.](@site/docs/img/calendar-event-de.png)

## Einladungen \{#invitations}

Speicherst du einen Termin mit neuen Gästen, bietet der Kalender an, jedem
von ihnen eine Einladung zu schicken. Die Einladung ist eine gewöhnliche
Nachricht mit einem `.ics`-Anhang, also liest sie jede Kalender-App, auf
der Box und außerhalb. **Einladungen senden** schickt sie bei einem Termin
noch einmal, und **Gästen schreiben** öffnet eine Nachricht an alle.

Eine Einladung, die dich in Mail erreicht, zeigt über der Nachricht, worum
es geht, mit **Ja**, **Vielleicht** und **Nein**. Deine Antwort trägt den
Termin in deinen Kalender ein und schickt sie per Mail an den, der dich
eingeladen hat. Kommt die Antwort dort an, zeigt seine Kopie des Termins
sie, sobald er die Nachricht öffnet. Eine Absage funktioniert genauso: Wer
sie öffnet, sieht den Termin im eigenen Kalender als abgesagt.

## Erinnerungen \{#reminders}

Eine Erinnerung erscheint auf der Verwaltungsseite, die du gerade offen
hast: der Name des Termins, wann er beginnt, und **Anruf beitreten** bei
einem Anruf oder **Öffnen** bei allem anderen. Hast du im Browser
Benachrichtigungen erlaubt, erscheint sie auch als Benachrichtigung,
solange der Tab im Hintergrund ist. Eine Erinnerung erscheint nur, solange
irgendwo eine Seite der Box offen ist.

## Abonnements \{#subscriptions}

**Abonnieren** folgt einem Kalender, den jemand anderes veröffentlicht,
etwa einer Liste von Steuerfristen oder einem Stundenplan aus EduPage. Füge
seine Adresse ein, die mit `webcal://`, `https://` oder `http://` beginnt,
wähle, wie oft er gelesen wird, und eine Farbe. Der Kalender liest ihn
einmal, bevor er ihn behält.

- Ein Abonnement wird von selbst wieder gelesen, jede Stunde, alle sechs
  Stunden, jeden Tag oder jede Woche, wie du es gewählt hast. **Jetzt
  lesen** in seinen Einstellungen liest es sofort. Termine, die der
  Herausgeber hinzugefügt, geändert oder entfernt hat, ändern sich mit.
- Ein Abonnement ist schreibgeschützt. Seine Termine lassen sich weder hier
  noch auf dem Handy bearbeiten; nur der Herausgeber ändert sie.
- Die Box liest die Adresse in einem eigenen Dienst, der keinen Zugriff auf
  deine Dateien hat und die Dienste nicht erreicht, die nur innerhalb der
  Box lauschen.
- Schlägt das Lesen fehl, sagen die Einstellungen des Kalenders, warum und
  wann es zuletzt geklappt hat. Die Termine vom letzten erfolgreichen Lesen
  bleiben.

Eine Box, auf der Nextcloud lief, behält ihre Abonnements: Der Umzug zu
LosOS Cloud bringt jedes davon als Abonnement mit.

## Importieren \{#importing}

**Importieren** nimmt eine `.ics`-Datei, etwa eine, die eine Kalender-App
exportiert hat, in einen deiner Kalender auf oder in einen neuen, der nach
der Datei heißt. Ein Termin, der schon im Kalender ist, wird aktualisiert
statt ein zweites Mal hinzugefügt.

## Einen Kalender veröffentlichen \{#publishing-a-calendar}

**Als Abo veröffentlichen** in den Einstellungen eines Kalenders gibt ihm
eine Adresse, die jede Kalender-App abonnieren kann, etwa
`webcal://mattbox.local/cloud/feed/….ics`. Wer die Adresse hat, kann den
Kalender ohne Anmeldung lesen, also gib sie nur Leuten, die ihn sehen
sollen. Schaltest du das Veröffentlichen aus, funktioniert die Adresse
nicht mehr. Schaltest du es wieder ein, gibt es eine neue Adresse.

## Auf Handy und Computer \{#on-your-phone-and-computer}

Handys und Kalender-Apps auf dem Computer synchronisieren über CalDAV mit
denselben Kalendern. Richte sie ein wie unter
[Kalender und Kontakte](losos-cloud#calendars-and-contacts). Ein Termin, den
du auf dem Handy anlegst, erscheint im Kalender, und einer, den du im
Kalender änderst, erreicht das Handy bei der nächsten Synchronisierung.
Abonnements kommen auch aufs Handy, schreibgeschützt.
