---
title: Chat
sidebar_position: 4.6
slug: /manual/chat
---

# Chat

Chat ist ein Messenger auf dem Gerät für alle, die ein Konto bei LosOS Login
haben. Man schreibt sich Direktnachrichten und in Räumen, teilt Bilder und
Dateien, antwortet in Threads und reagiert auf Nachrichten.
Direktnachrichten und neue Räume sind Ende-zu-Ende-verschlüsselt, also
speichert das Gerät nur, was es selbst nicht lesen kann.

Chat verwendet Matrix, einen offenen Standard für Nachrichten, und läuft auf
dem Gerät selbst. Nichts geht an einen fremden Dienst.

## Wann Chat da ist

Chat gehört zu dem Teil des Geräts, den Sie mit dem Mesh teilen, und
speichert alles in dessen Speicher. Er läuft nur, solange das Gerät seinen
Speicher teilt und einem Mesh beigetreten ist. Dann erscheint **Chat** in der
Seitenleiste der Verwaltungsseiten.

Wenn Sie das Teilen beenden, hält Chat mit an. Der Eintrag in der
Seitenleiste verschwindet, und die Adresse des Chats führt zurück zu den
Verwaltungsseiten. Die Nachrichten bleiben im geteilten Speicher und kommen
zurück, wenn Sie wieder teilen.

## Anmelden

Öffnen Sie **Chat** in der Seitenleiste oder gehen Sie zu
`https://<name>.local/chat/`. Chat verwendet Ihre Anmeldung bei LosOS Login.
Ein eigenes Chat-Passwort gibt es nicht.

Wenn Sie sich bei LosOS Login abmelden, meldet sich auch Chat ab. Wenn eine
Administratorin oder ein Administrator jemanden unter **Einstellungen** >
**Benutzer** entfernt, werden die Chat-Geräte dieser Person sofort
abgemeldet.

Die Chat-Adresse aller ist `@<Anmeldename>:<name>.local`, zum Beispiel
`@eva:mattbox.local`.

## Sichere Nachrichten

Beim ersten Öffnen bittet Chat Sie, sichere Nachrichten einzurichten.
Klicken Sie auf **Sichere Nachrichten einrichten** und dann auf
**Wiederherstellungsschlüssel erstellen**. Chat zeigt einen
Wiederherstellungsschlüssel aus zwölf Gruppen von Buchstaben und Ziffern.
Kopieren Sie ihn oder laden Sie ihn herunter, bewahren Sie ihn an einem
sicheren Ort getrennt von diesem Computer auf und klicken Sie auf **Ich habe
ihn gespeichert**. Chat zeigt den Schlüssel nicht noch einmal.

Mit dem Schlüssel können Ihre anderen Geräte Ihre verschlüsselten
Nachrichten lesen. Wenn Sie Chat in einem anderen Browser oder auf einem
anderen Computer öffnen, zeigt er **Entsperren Sie dieses Gerät, um
verschlüsselte Nachrichten zu lesen**. Klicken Sie darauf und geben Sie den
Wiederherstellungsschlüssel ein, oder bestätigen Sie das neue Gerät von
einem, das Sie schon verwenden, indem Sie die Emoji vergleichen, die beide
zeigen.

Ohne den Schlüssel und ohne ein anderes Gerät kann ein neues Gerät keine
Nachrichten lesen, die vor seiner Anmeldung gesendet wurden. Niemand kann
sie für Sie wiederherstellen, auch nicht die Administration des Geräts.

Die Schaltfläche mit dem Schloss über der Liste der Unterhaltungen öffnet
jederzeit **Sichere Nachrichten**. Dort stehen Ihre Geräte, und Sie können
jedes einzelne überprüfen oder abmelden.

## Jemandem schreiben

- **Eine Direktnachricht:** Klicken Sie auf **Jemandem schreiben** und wählen
  Sie eine Person. In der Liste stehen alle mit einem Konto bei LosOS Login
  auf dem Gerät.
- **Ein Raum:** Klicken Sie auf **Neuer Raum**, geben Sie ihm einen Namen und
  wählen Sie, ob er auf dem Gerät öffentlich ist oder nur auf Einladung.
  Personen laden Sie über die Kopfzeile des Raums ein.
- **Bereiche** fassen Räume zusammen. Einen neuen erstellen Sie mit der
  Plus-Schaltfläche am linken Rand, Räume fügen Sie in den Einstellungen des
  jeweiligen Raums hinzu.
- **Räume zum Beitreten finden** listet die öffentlichen Räume auf dem Gerät.

## In einer Unterhaltung

Fahren Sie mit dem Zeiger über eine Nachricht, um darauf zu reagieren, zu
antworten, im Thread zu antworten, sie zu bearbeiten oder zu löschen. Die
Thread-Schaltfläche in der Kopfzeile des Raums listet seine Threads.

Bilder, Videos und Dateien hängen Sie mit der Büroklammer an, durch
Einfügen oder durch Hineinziehen. In einem verschlüsselten Raum werden sie
verschlüsselt, bevor sie Ihren Browser verlassen. Das Mikrofon nimmt eine
Sprachnachricht auf.

Eine kleine Reihe von Gesichtern unter einer Nachricht zeigt, wer bis dorthin
gelesen hat.

## Andere Matrix-Apps

Andere Matrix-Apps wie Element können sich nicht beim Chat des Geräts
anmelden, denn Chat hat keine Passwörter und meldet nur über LosOS Login an.
Verwenden Sie die Chat-Seite in einem Browser auf einem beliebigen Gerät im
Netzwerk des Geräts.

Chat spricht nur mit dem Gerät. Die Verbindung zu anderen Matrix-Servern ist
aus, bis eine Administratorin oder ein Administrator
`losos.chat.federation.enable` unter **Einstellungen** > **Erweitert**
einschaltet. Dafür braucht es eine eigene Domain, die andere Server
erreichen.
