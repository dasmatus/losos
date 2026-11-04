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

Anrufe folgen MatrixRTC, dem Matrix-Standard für Anrufe in Räumen, als
LosOS' eigener Anruftyp `org.losos.call`, mit einem Anruf pro Raum. Wer in
einem Anruf ist, veröffentlicht eine Mitgliedschaft, die den Anrufserver
der Box nennt, `losos-calls`. Eine Matrix-App, die diesen Anruftyp nicht
kennt, lässt ihn deshalb in Ruhe, statt ein **Beitreten** anzubieten, das
nicht funktionieren kann. Klingeln, Ablehnen und der Austausch der
Schlüssel nutzen die eigenen Ereignisse von MatrixRTC. Bild und Ton laufen
über `losos-calls`, nicht über LiveKit.

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

In einer Direktnachricht bekommt die andere Person in der Ecke ihrer
Chat-Seite eine Karte, die anderthalb Minuten lang klingelt. In einem Raum
bekommen alle sie, wenn die Person, die den Anruf startet, den ganzen Raum
benachrichtigen darf, was standardmäßig ein Raum-Admin ist; sonst sehen die
anderen den Anruf in der Zeitleiste. **Annehmen** tritt so bei, wie der
Anruf gestartet wurde. Bei einem Videoanruf tritt **Ohne Video annehmen**
nur mit dem Mikrofon bei. **Ablehnen** blendet die Karte aus. In einer
Direktnachricht erfährt es auch die Person, die anruft, und ihr Anruf
schließt sich.

Die Zeitleiste des Raums zeigt, dass ein Anruf begonnen hat. Solange er
läuft, hat die Zeile einen Knopf **Beitreten**, damit auch zurückkommen
kann, wer das Klingeln verpasst oder den Anruf verlassen hat. Schließt sich
der Browser von jemandem oder verliert er das Netz, sehen die anderen diese
Person nach höchstens einer halben Minute gehen. Wenn die letzte Person geht, zeigt die Zeitleiste, dass der Anruf beendet ist.
Andere Matrix-Apps zeigen stattdessen einen kurzen Hinweis mit einem Link,
der den Anruf in LosOS Chat öffnet, wo man sich mit LosOS Login anmeldet.

Räume und Direktnachrichten, die in LosOS Chat angelegt wurden, sind für
Anrufe bereit. In einem Raum, der mit einem älteren LosOS angelegt wurde,
schaltet ein Raum-Admin Anrufe ein, indem er den ersten startet. Bis dahin
bekommt jede andere Person, die es versucht, eine Meldung dazu.

## Während eines Anrufs \{#during-a-call}

Jede Person hat eine Kachel. Die Kachel zeigt ihre Kamera, oder ihr Bild,
solange ihre Kamera aus ist. Die Knöpfe unten schalten dein Mikrofon und
deine Kamera aus und ein, teilen deinen Bildschirm, wechseln deinen
Hintergrund, laden einen Gast ein, verändern die Stimme, fügen LosOS hinzu, öffnen den
Podcast-Modus und verlassen den Anruf. Die Kamera auszuschalten hält sie an, ihr Licht geht aus, und
bis du sie wieder einschaltest, wird nichts gesendet.

Deine Stimme geht mit 128 kb/s über den ganzen hörbaren Bereich hinaus,
mit Echo- und Rauschunterdrückung des Browsers. Deine Kamera sendet
gleichmäßig 30 Bilder pro Sekunde. Reicht die Verbindung nicht, senkt sie
zuerst die Schärfe und hält die Bewegung flüssig; ein geteilter Bildschirm
macht es umgekehrt und hält seinen Text scharf.

Das Schild **Ende-zu-Ende-verschlüsselt** oben zeigt, wie viele Bilder dein
Browser verschlüsselt gesendet und wie viele er empfangen und geöffnet hat.

## Deine Stimme \{#your-voice}

**Meine Stimme verändern** verändert, wie die anderen dich hören:
**Tiefer**, **Höher**, **Riese**, **Eichhörnchen** oder **Roboter**, und
**Natürlich** ist deine eigene Stimme. Das passiert in deinem Browser,
bevor der Anruf verschlüsselt wird, die Box und die anderen bekommen also
nur die veränderte Stimme, und niemand erkennt, dass es nicht deine ist.
Die Wahl bleibt für deine nächsten Anrufe, bis du **Natürlich** wählst.

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

## LosOS im Anruf \{#losos-in-the-call}

**LosOS hinzufügen** holt den Assistenten der Box in den Anruf. Er hört
zu, und wenn jemand seinen Namen sagt, antwortet er laut, allen, mit einer
eigenen Stimme. Sag „LosOS, was steht am Freitag auf der Liste?“, und er
antwortet. Etwa zwanzig Sekunden nach einer Antwort kann dieselbe Person
ohne den Namen weitersprechen. „Stopp“ bringt ihn zum Schweigen, wer auch
immer es sagt, ebenso **Nicht mehr sprechen** in seinem Panel. **LosOS
entfernen** nimmt ihn aus dem Anruf.

Seine Kachel zeigt statt eines Bildes einen Roboter, und alle im Anruf
sehen ihn, damit niemand abgehört wird, ohne es zu wissen.

Anrufe sind Ende-zu-Ende-verschlüsselt, die Box kann sie also nicht hören,
und ein Assistent, der auf der Box läuft, auch nicht. LosOS tritt deshalb
aus dem Browser der Person bei, die es hinzugefügt hat, als zweites Mitglied
des Anrufs, mit den Schlüsseln, die diese Person schon hat. Dieser Browser
hört zu und spricht:

- Whisper, eine Spracherkennung, macht aus der Stimme jeder Person Text.
  Jede Person kommt auf einem eigenen Strom an, es ist also klar, wer was
  gesagt hat.
- Kokoro, eine Sprachsynthese, spricht die Antwort.
- Beide laufen als WebAssembly auf dem Computer dieser Person. Ein neuerer
  Computer kommt mit, auf einem langsamen kommen die Antworten später.

Beim ersten Mal lädt der Browser die beiden Modelle, zusammen etwa 150 MB,
von der Box und behält sie. Von sonst woher wird nichts geladen. Danach
startet ein Anruf ohne Download.

Den Browser verlässt die Frage, als Text, an den Assistenten der Box,
LosOS Tag, zusammen mit den letzten Minuten des Gesprächs, damit „was
meinst du dazu?“ einen Bezug hat. Der aufgenommene Ton bleibt im Browser
und ist weg, sobald er gelesen wurde. Verlässt die Person, die LosOS
hinzugefügt hat, den Anruf oder schließt die Seite, geht LosOS mit.

Die Antwort wird allen im Anruf vorgelesen, deshalb nutzt der Assistent
standardmäßig nicht die Dateien, Mails und Räume der Person, die ihn
hinzugefügt hat. **Darf meine Dateien, Mails und Räume nutzen** in seinem
Panel schaltet das für diesen Anruf ein.

LosOS versteht und spricht vorerst Englisch. Es ist an, solange du
**Einstellungen → Erweitert → `losos.calls.agent.enable`** nicht
ausschaltest; ohne sie liefert die Box keine Sprachmodelle aus, und der
Knopf fehlt.

## Was die Box sieht \{#what-the-box-sees}

Die Box leitet jedes Bild an die anderen im Anruf weiter, ohne es zu
öffnen. Sie weiß, wer wann in welchem Anruf ist, wie groß die Bilder sind
und wie oft sie kommen. Bild und Ton hat sie nie, außer einem
Podcast-Stream, den ihr jemand zum Weitergeben schickt.

Das Hinzufügen von LosOS ändert daran für die Box nichts: Es tritt aus
einem Browser bei und hält dieselbe Art Schlüssel, die Box sieht also
weiterhin nur verschlüsselte Frames. Was es hört, bringt die Person mit,
die es hinzugefügt hat.

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
Geräte im Anruf, so wie der Chat-Server der Box sie nennt. Wer die
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
