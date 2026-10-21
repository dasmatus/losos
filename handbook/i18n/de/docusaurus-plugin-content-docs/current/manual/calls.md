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
solange ihre Kamera aus ist. Die drei Knöpfe unten schalten dein Mikrofon
und deine Kamera aus und ein und verlassen den Anruf. Die Kamera
auszuschalten hält sie an, ihr Licht geht aus, und bis du sie wieder
einschaltest, wird nichts gesendet.

Das Schild **Ende-zu-Ende-verschlüsselt** oben zeigt, wie viele Bilder dein
Browser verschlüsselt gesendet und wie viele er empfangen und geöffnet hat.

## Was die Box sieht \{#what-the-box-sees}

Die Box leitet jedes Bild an die anderen im Anruf weiter, ohne es zu
öffnen. Sie weiß, wer wann in welchem Anruf ist, wie groß die Bilder sind
und wie oft sie kommen. Bild und Ton hat sie nie.

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

Anrufe funktionieren zwischen Rechnern, die die Box in ihrem eigenen Netz
erreichen. Die Box nimmt die Medien auf UDP-Port 7882 an
(**Einstellungen → Erweitert → `losos.calls.udpPort`**), der in ihrer
Firewall offen ist, solange Anrufe an sind. Anrufe von außerhalb dieses
Netzes funktionieren noch nicht.
