---
title: Chat
sidebar_position: 4.6
slug: /manual/chat
---

# Chat

![Der Raum eines Repositorys in LosOS Chat, mit dem, was LosOS Git dort geschrieben hat.](@site/docs/img/chat-room.png)

LosOS Chat ist ein Messenger auf dem Gerät für alle, die ein Konto bei
LosOS Login haben. Ihr schreibt euch Direktnachrichten und in Räumen, teilt
Bilder und Dateien, antwortet in Threads und reagiert auf Nachrichten.
Direktnachrichten und neue Räume sind Ende-zu-Ende-verschlüsselt, also
speichert das Gerät nur, was es selbst nicht lesen kann.

Chat verwendet Matrix, einen offenen Standard für Nachrichten, und läuft auf
dem Gerät selbst. Nichts geht an einen fremden Dienst. Der Matrix-Server auf
dem Gerät ist Synapse. Ein Gerät, das Chat schon vor dem Wechsel zu Synapse
hatte, beginnt mit einem leeren Chat: Die alten Unterhaltungen bleiben im
geteilten Speicher, aber Chat liest sie nicht mehr.

## Wann Chat da ist

Chat gehört zu dem Teil des Geräts, den du mit dem Mesh teilst, und
speichert alles in dessen Speicher. Er läuft nur, solange das Gerät seinen
Speicher teilt und einem Mesh beigetreten ist. Dann erscheint **Chat** in der
Seitenleiste der Verwaltungsseiten.

Wenn du das Teilen beendest, hält Chat mit an. Der Eintrag in der
Seitenleiste verschwindet, und die Adresse des Chats führt zurück zu den
Verwaltungsseiten. Die Nachrichten bleiben im geteilten Speicher und kommen
zurück, wenn du wieder teilst.

## Anmelden

Öffne **Chat** in der Seitenleiste oder geh zu
`https://<name>.local/chat/`. Chat ist eine der [Admin-Seiten](admin-pages), mit
derselben oberen Leiste und Seitenleiste: links die Unterhaltungen, daneben
die geöffnete. Er verwendet deine Anmeldung bei LosOS Login.
Ein eigenes Chat-Passwort gibt es nicht.

Wenn du dich bei LosOS Login abmeldest, meldet sich auch Chat ab. Wenn eine
Administratorin oder ein Administrator jemanden unter **Einstellungen** >
**Benutzer** entfernt, werden die Chat-Geräte dieser Person sofort
abgemeldet.

Die Chat-Adresse aller ist `@<Anmeldename>:<name>.local`, zum Beispiel
`@eva:mattbox.local`.

## Sichere Nachrichten

Beim ersten Öffnen bittet Chat dich, sichere Nachrichten einzurichten.
Klick auf **Sichere Nachrichten einrichten** und dann auf
**Wiederherstellungsschlüssel erstellen**. Chat zeigt einen
Wiederherstellungsschlüssel aus zwölf Gruppen von Buchstaben und Ziffern.
Kopiere ihn oder lade ihn herunter, bewahre ihn an einem sicheren Ort
getrennt von diesem Computer auf und klick auf **Ich habe ihn gespeichert**.
Chat zeigt den Schlüssel nicht noch einmal.

Mit dem Schlüssel können deine anderen Geräte deine verschlüsselten
Nachrichten lesen. Wenn du Chat in einem anderen Browser oder auf einem
anderen Computer öffnest, zeigt er **Entsperre dieses Gerät, um
verschlüsselte Nachrichten zu lesen**. Klick darauf und gib den
Wiederherstellungsschlüssel ein, oder bestätige das neue Gerät von einem,
das du schon verwendest, indem du die Emoji vergleichst, die beide zeigen.

Ohne den Schlüssel und ohne ein anderes Gerät kann ein neues Gerät keine
Nachrichten lesen, die vor seiner Anmeldung gesendet wurden. Niemand kann
sie für dich wiederherstellen, auch nicht die Administration des Geräts.

Die Schaltfläche mit dem Schloss in der oberen Leiste öffnet jederzeit
**Sichere Nachrichten**. Dort stehen deine Geräte, und du kannst jedes
einzelne überprüfen oder abmelden.

## Jemandem schreiben

- **Eine Direktnachricht:** Klick auf **Jemandem schreiben** und wähle eine
  Person. In der Liste stehen alle mit einem Konto bei LosOS Login auf dem
  Gerät und alle deine [Kontakte](contacts) mit einer Chat-Adresse. Der Knopf
  **Chat** auf der Seite eines Kontakts öffnet dieselbe Unterhaltung.
- **Ein Raum:** Klick auf **Neuer Raum**, gib ihm einen Namen und wähle, ob
  er auf dem Gerät öffentlich ist oder nur auf Einladung. Personen lädst du
  über die Kopfzeile des Raums ein.
- **Bereiche** fassen Räume zusammen. Sie sind die Reihe von Schaltflächen
  über den Unterhaltungen, neben der Suche; einen neuen erstellst du mit
  **Neuer Bereich**, Räume fügst du in den Einstellungen des jeweiligen Raums
  hinzu.
- **Räume zum Beitreten finden** listet die öffentlichen Räume auf dem Gerät.

## In einer Unterhaltung

Fahr mit dem Zeiger über eine Nachricht, um darauf zu antworten, im Thread
zu antworten, sie zu bearbeiten oder zu löschen. Die schnellen Reaktionen
erscheinen in einer Leiste darunter: Drück eine, um sie hinzuzufügen, und
noch einmal, um sie zurückzunehmen. Ein
Rechtsklick auf die Nachricht bietet dasselbe und dazu das Kopieren ihres
Texts. Die
Thread-Schaltfläche in der Kopfzeile des Raums listet seine Threads.

Bilder, Videos und Dateien hängst du mit der Büroklammer an, durch Einfügen
oder durch Hineinziehen. In einem verschlüsselten Raum werden sie
verschlüsselt, bevor sie deinen Browser verlassen. Das Mikrofon nimmt eine
Sprachnachricht auf.

Hat das Gerät LosOS Cloud, schickt die Wolken-Schaltfläche neben der
Büroklammer Dateien direkt aus deiner Cloud. Eine Datei, die dir jemand
schickt, hat auf ihrer Karte eine Wolken-Schaltfläche und im Rechtsklickmenü
**In LosOS Cloud speichern**: Sie landet entschlüsselt im Ordner **Chat**
deiner Cloud, und die Schaltfläche öffnet sie danach dort. Schickst du in
LosOS Cloud eine Datei in den Chat, fragt der Chat, in welche Unterhaltung
sie geht.

Eine kleine Reihe von Gesichtern unter einer Nachricht zeigt, wer bis dorthin
gelesen hat.

![Eine Unterhaltung mit Eva in LosOS Chat: Reaktionen unter den Nachrichten, eine Antwort in einem Thread und ein Bild, darunter die Büroklammer, die Cloud-Taste und das Mikrofon.](@site/docs/img/chat-de.png)

## Andere Matrix-Apps

Andere Matrix-Apps wie Element können sich nicht beim Chat des Geräts
anmelden, denn Chat hat keine Passwörter und meldet nur über LosOS Login an.
Verwende die Chat-Seite in einem Browser auf einem beliebigen Gerät im
Netzwerk des Geräts.

## Andere LosOS-Boxen \{#other-losos-boxes}

Der Chat erreicht die LosOS-Boxen in deinem Netz. Schreib von
`mattbox.local` an `@eva:kitchenbox.local`, und die Nachricht geht von einer
Box zur anderen, ohne das Netz zu verlassen. Eine Box mit einem
`.local`-Namen antwortet anderen Chat-Servern nur aus dem Netz, in dem sie
steht.

![Derselbe Raum im Chat einer zweiten Box, von kitchenbox.local aus beigetreten.](@site/docs/img/chat-other-box.png)

Um auch mit Matrix-Servern im Internet zu sprechen, braucht das Gerät eine
eigene Domain, die sie erreichen. `losos.chat.federation.enable` unter
**Einstellungen** > **Erweitert** schaltet die Verbindung zu anderen
Servern ganz aus.

Ein Repository in [LosOS Git](losos-git#a-chat-room-for-a-repository) kann
hier einen Raum haben, in den LosOS Git schreibt, was darin passiert. Leute
von anderen Boxen melden sich bei LosOS Git auch mit ihrer Chat-Adresse an.
