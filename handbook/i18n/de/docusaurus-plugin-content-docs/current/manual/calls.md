---
title: Anrufe
sidebar_position: 11
slug: /manual/calls
---

# Anrufe

Leute auf der Box können sich aus LosOS Chat anrufen, mit Sprache oder mit
Video, in einer Direktnachricht oder in einem Raum mit bis zu acht Leuten.
Der Anruf läuft über die Box, und die Box kann ihn weder sehen noch hören.
Jedes Bild und jedes Stück Ton verschlüsselt der Browser, der es sendet,
und erst die Browser, die es empfangen, entschlüsseln es.

MatrixRTC koordiniert die Anrufteilnahme im Raum, den Ablauf der Mitgliedschaften
und die Klingelbenachrichtigungen. Jede Mitgliedschaft veröffentlicht LosOS'
eigenen SFU-Transport, nicht LiveKit. Audio und Video laufen weiterhin über
`losos-calls`; die Verschlüsselung der Medienframes im Browser und der
Schlüsselaustausch über Olm bleiben LosOS-spezifisch.

## Wann es Anrufe gibt \{#when-calls-are-there}

Anrufe gehören zum Chat und brauchen deshalb dasselbe wie der Chat: Die Box
teilt ihren Speicher und ist einem Mesh beigetreten. Außerdem brauchen sie
**Einstellungen → Erweitert → `losos.calls.enable`**, das an ist, solange
du es nicht ausschaltest. Ist eines davon aus, haben Räume keine
Anruf-Knöpfe und die Box startet nichts für Anrufe. Schaltest du es wieder
ein, sind die Anrufe nach dem nächsten Anwenden zurück.

## Einen Anruf starten \{#starting-a-call}

Öffne einen Raum oder eine Direktnachricht. Im Kopf sind zwei Knöpfe,
**Sprachanruf** und **Videoanruf**. Dein Browser fragt einmal nach dem
Mikrofon, und nach der Kamera, wenn du mit Video startest.

Ein Anruf braucht keine Kamera. Öffnet der Browser keine, weil keine da ist
oder du sie nicht erlaubt hast, bist du mit ausgeschalteter Kamera dabei,
und eine Meldung sagt das; die anderen hören dich wie gewohnt. Ohne
Mikrofon hörst du nur zu, und der Mikrofonknopf fragt erneut danach.

Alle im Raum bekommen in der Ecke ihrer Chat-Seite eine Karte, die eine
Minute lang klingelt. **Annehmen** tritt so bei, wie der Anruf gestartet
wurde. Bei einem Videoanruf tritt **Ohne Video annehmen** nur
mit dem Mikrofon bei. **Ablehnen** blendet die Karte nur aus.

Die Zeitleiste des Raums zeigt, dass ein Anruf begonnen hat. Solange er
läuft, hat die Zeile einen Knopf **Beitreten**, damit auch zurückkommen
kann, wer das Klingeln verpasst oder den Anruf verlassen hat. Wenn die
letzte Person geht, zeigt die Zeitleiste, dass der Anruf beendet ist.

## Während eines Anrufs \{#during-a-call}

Jede Person hat eine Kachel. Die Kachel zeigt ihre Kamera, oder ihr Bild,
solange ihre Kamera aus ist. Die Knöpfe unten schalten dein Mikrofon und
deine Kamera aus und ein, teilen deinen Bildschirm, wechseln deinen
Hintergrund, laden einen Gast ein, öffnen den Podcast-Modus und verlassen
den Anruf. Die Kamera auszuschalten hält sie an, ihr Licht geht aus, und
bis du sie wieder einschaltest, wird nichts gesendet.

Deine Stimme geht mit 128 kb/s über den ganzen hörbaren Bereich hinaus,
mit Echo- und Rauschunterdrückung des Browsers. Deine Kamera sendet
gleichmäßig 30 Bilder pro Sekunde. Reicht die Verbindung nicht, senkt sie
zuerst die Schärfe und hält die Bewegung flüssig; ein geteilter Bildschirm
macht es umgekehrt und hält seinen Text scharf.

Das Schild **Ende-zu-Ende-verschlüsselt** oben zeigt, wie viele Bilder dein
Browser verschlüsselt gesendet und wie viele er empfangen und geöffnet hat.

## Dein Hintergrund \{#your-background}

Wenn deine Kamera an ist, ist der Raum hinter dir schwarz. Dein Browser
findet dich in jedem Bild und schwärzt alles andere, bevor das Bild
verschlüsselt wird, also bekommen weder die Box noch die anderen deinen
Raum. Der Knopf **Raum hinter dir zeigen** sendet die Kamera, wie sie
ist, und **Hintergrund schwärzen** schaltet das Schwarz wieder ein. Der
Browser merkt sich deine Wahl für den nächsten Anruf.

Kann der Browser den Hintergrund nicht schwärzen, sendet deine Kamera
nichts statt deines Raums, und eine Meldung sagt das.

## Den Bildschirm teilen \{#sharing-your-screen}

**Bildschirm teilen** fragt deinen Browser, welchen Bildschirm, welches
Fenster oder welchen Tab du teilen willst. Solange jemand teilt, füllt sein
Bildschirm die Mitte des Anrufs, und die Leute rücken in einen Streifen
daneben. **Teilen beenden** oder der Stopp-Knopf des Browsers beendet es.
Der Bildschirm ist mit denselben Schlüsseln verschlüsselt wie die Kamera.

## Podcast-Modus \{#podcast-mode}

Der Podcast-Modus sendet den Anruf an eine RTMP-Adresse, etwa die, die dir
YouTube oder Twitch mit einem Stream-Schlüssel gibt. **Podcast-Modus**
öffnet ein Feld mit:

- der RTMP-Adresse, die mit `rtmp://` oder `rtmps://` beginnt;
- einem Titel und einem Untertitel, die unter das Bild gezeichnet werden;
- einem Overlay, einem PNG, das obendrauf gezeichnet wird, für ein Logo
  oder einen Rahmen.

Der Stream zeigt die Leute als Raster, oder einen geteilten Bildschirm in
der Mitte mit den Leuten daneben, wie der Anruf. **Live gehen** startet
ihn und **Stream beenden** hält ihn an. Ein Anruf wird jeweils nur von
einer Person gestreamt, und alle im Anruf sehen ein **Live**-Schild,
solange der Stream läuft.

Ein Livestream ist öffentlich, also ist er der eine Teil eines Anrufs, der
nicht Ende-zu-Ende-verschlüsselt ist. Dein Browser setzt Bild und Ton aus
dem Anruf zusammen, den er schon entschlüsselt hat, und schickt sie an
deine Box, die sie an die RTMP-Adresse weitergibt. Der Stream-Schlüssel
bleibt in deinem Browser und der Box und wird nie in die Protokolle der
Box geschrieben.

## Gäste \{#guests}

Jemand ohne Konto auf der Box kann mit einem Gast-Link einem Anruf
beitreten. **Gast einladen** erstellt den Link und zeigt ihn mit einem
Knopf **Link kopieren**. Schick ihn der Person, die du einlädst. Sie öffnet
ihn im Browser, gibt einen Namen an und tritt bei. Die anderen sehen sie
als *Name (Gast)*.

Ein Gast-Link:

- öffnet nur diesen Anruf, für 12 Stunden;
- lässt jeweils nur einen Gast herein;
- gibt kein Konto, keinen Chat und keinen Zugang zu etwas anderem auf der
  Box;
- funktioniert für den Gast nicht mehr, sobald alle aus dem Raum gegangen
  sind.

Gast-Links kommen von einem offiziellen LosOS-Edge, auf dem öffentlichen
Namen der Box, also kann eine Box ohne einen keine erstellen. Der Edge
erstellt den Link, kann aber nicht mithören. Der Teil nach `#` ist ein
Geheimnis, das dein Browser erzeugt, und ein Browser schickt diesen Teil
nie an einen Server. Dein Browser gibt ihn den anderen im Anruf in den
verschlüsselten Nachrichten des Chats. Der Browser des Gasts und der
Browser jeder Person prüfen damit einander, einigen sich dann mit
Schlüsseln, die nur für diesen Anruf entstehen, auf einen Schlüssel für die
beiden und geben erst dann die Schlüssel des Anrufs weiter. Wer den Link
ohne den Teil nach `#` hat, kommt in den Anruf, sieht und hört aber nichts.

## Was die Box sieht \{#what-the-box-sees}

Die Box leitet jedes Bild an die anderen im Anruf weiter, ohne es zu
öffnen. Sie weiß, wer wann in welchem Anruf ist, wie groß die Bilder sind
und wie oft sie kommen. Bild und Ton hat sie nie, außer einem
Podcast-Stream, den ihr jemand zum Weitergeben schickt.

Der Browser jeder Person macht für den Anruf einen eigenen Schlüssel. Die
Browser geben sich ihre Schlüssel in den verschlüsselten Nachrichten des
Chats zwischen Geräten, die die Box transportiert, aber nicht lesen kann.
Die Schlüssel wechseln, während der Anruf läuft:

- Wenn jemand beitritt, gehen alle zum nächsten Schlüssel über, und die
  neue Person bekommt ihn. Sie kann nichts öffnen, was vor ihr gesendet
  wurde.
- Wenn jemand geht, machen alle einen neuen zufälligen Schlüssel und geben
  ihn nur denen, die geblieben sind. Wer gegangen ist, kann nichts öffnen,
  was danach gesendet wird.
- Alle fünf Minuten gehen alle ohnehin zum nächsten Schlüssel über.

Die Schlüssel liegen nur im Speicher des Browsers und sind weg, wenn der
Anruf endet. Nichts wird gespeichert, also öffnet ein später gefundener
Schlüssel nichts aus dem Anruf.

Eine Grenze solltest du kennen. Ein Browser sendet seinen Schlüssel an die
Geräte, die der Chat-Server der Box für jede Person im Anruf nennt. Wer die
Software der Box übernommen hätte, könnte dort auch ein eigenes Gerät
nennen. Verifiziert eure Geräte im Chat gegenseitig (**Sichere
Nachrichten**, dann **Verifizieren**), dann wisst ihr, dass ein Gerät
wirklich der anderen Person gehört.

## Browser \{#browsers}

Ein Browser muss Bilder verschlüsseln können, um einem Anruf beizutreten.
Aktuelle Versionen von Firefox, Chrome, Edge und Safari können das. Ein
Browser, der es nicht kann, zeigt keine Anruf-Knöpfe und sendet nie ein
Bild ohne Verschlüsselung.

## Wo Anrufe funktionieren \{#where-calls-work}

Auf einer Box in ihrem eigenen Netz funktionieren Anrufe zwischen
Rechnern, die die Box dort erreichen. Die Box nimmt die Medien auf UDP-Port
7882 an (**Einstellungen → Erweitert → `losos.calls.udpPort`**), der in
ihrer Firewall offen ist, solange Anrufe an sind.

Auf einer Box, die über einen Edge geht, gehen auch die Medien über den
Edge. Die Box hält eine QUIC-Verbindung zum Edge und öffnet selbst keinen
Port, und Leute und Gäste senden ihre Medien an einen Port auf dem Edge.
Die Bilder sind dort genauso verschlüsselt wie auf der Box.
