---
title: Nur das lokale Netzwerk funktioniert
sidebar_position: 1
slug: /troubleshooting/only-the-lan-works
---

# Nur das lokale Netzwerk funktioniert

<div className="losos-symptom">

**Was du siehst:** Die eigenen Seiten der Box gehen unter ihrer
`http://192.168.…`-Adresse oder ihrem `.local`-Namen auf, und darin
funktioniert alles. Der Fernzugriff von außen geht nicht, der Bereich Mesh
findet keine Edge, oder auf keinem Rechner zu Hause lädt irgendetwas aus dem
Internet.

</div>

Die Box ist für genau diesen Fall gebaut. **Alles, was auf der Box lebt,
funktioniert im lokalen Netzwerk weiter**, egal was die Verbindung nach
außen gekappt hat. Nichts geht verloren oder wird angehalten, und wenn die
Verbindung nach außen zurückkommt, richtest du nichts neu ein.

## Was die Ursache sein kann \{#what-can-cause-it}

Jede dieser Ursachen lässt die Box aus dem LAN erreichbar und sonst nichts.
Die Schritte unten sind für alle gleich. Die Ursache entscheidet nur, wer
sie behebt.

| Ursache                                 | Woran du sie erkennst                                                                                   |
| --------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| Die Leitung des Anbieters ist ausgefallen | Auf keinem Gerät zu Hause öffnet sich eine Website, und die Statusleuchte des Routers zeigt es. Du kannst nur warten. |
| Der Router hat keinen Uplink            | Der Router hat seine Verbindung verloren, durch ein abgezogenes Kabel, einen laufenden Neustart oder eine geänderte Einstellung. Das LAN funktioniert weiter, weil der Switch funktioniert. |
| Die Edge ist ausgefallen oder nicht erreichbar | Das Internet funktioniert auf deinem Laptop, aber der Bereich Mesh findet keine Edge, und der öffentliche Name antwortet nicht. Siehe [Keine Edge gefunden](/troubleshooting/edge-not-found.md). |
| Das DNS des Anbieters ist kaputt        | Seiten über ihren Namen schlagen fehl, die Box über ihre IP funktioniert. Stell den Router oder den Rechner auf einen anderen Resolver um. |
| Die Box wurde nie mit einer Edge verbunden | Kein Ausfall. Eine [einzelne Box](../types/single-box) hat bewusst keinen Fernzugriff und kein Mesh. |

## Was weiter funktioniert \{#what-still-works}

![Eine Box, deren Internet ausgefallen ist, deren Firmen-Edge im LAN aber noch antwortet: Der Bereich Mesh führt sie unter In diesem Netzwerk.](@site/docs/img/mesh-company-de.png)

| Was                                 | Warum es kein Internet braucht                                                                                   |
| ----------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| Die Anmeldung an den Admin-Seiten   | Die Box prüft das Passwort gegen LosOS Cloud auf der Box selbst, über ihr eigenes Loopback. Der gedruckte Ersatzschlüssel funktioniert auch. |
| LosOS Cloud und LosOS Git           | Beide laufen auf der Box, also bleiben Dateien, Kalender, Kontakte, Repositorys und die Handy-Apps im selben WLAN lokal. |
| Deine eigenen Dateien               | Sie liegen auf der verschlüsselten Festplatte der Box, in jedem Speichermodus.                                   |
| Lokales Teilen zwischen Leuten zu Hause | Die Box entscheidet über Freigaben, Links und Berechtigungen innerhalb von LosOS Cloud.                       |
| Die ganze Admin-Oberfläche          | Anwenden, Einstellungen, der Bereich Aussehen, der Bereich Speicher und Neustarts sprechen alle über das LAN mit der Box. |
| Eingebaute und selbst geschriebene Widgets | Sie werden auf der Box gezeichnet, aus den eigenen Messwerten der Box. Ein selbst geschriebenes Widget, das *von sich aus* etwas aus dem Internet holt, zeigt diesen Teil leer, bis die Verbindung nach außen zurück ist, und zeichnet den Rest. |
| Dieses Handbuch                     | Die Box liefert unter `/handbook/` eine eigene Kopie aus, mit Suche. Wenn du diese Seite über Einstellungen → Über geöffnet hast, liest du diese Kopie. |
| Der nächtliche Neustart und das Update um 03:00 mit der Standardquelle | Die Standardquelle ist die eigene Kopie der Box von ihrer Beschreibung, auf der Festplatte. Der Rebuild läuft und wird offline fertig. |
| Eine Firmen-Edge im selben LAN      | Ein [Einsatz in einer Firma](../types/company-edge) behält seine Edge, sein Mesh und sein Teilen im lokalen Netzwerk. Der Internetanbieter liegt nicht auf diesem Weg. |

## Was auf die Verbindung nach außen wartet \{#what-waits-for-the-outside}

| Was                                         | Was du in der Zwischenzeit siehst                                                                                   |
| ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| Fernzugriff über die LosOS-Edge             | Die öffentliche Adresse deiner Box antwortet von außen nicht. Der Tunnel verbindet sich von selbst wieder, wenn die Verbindung nach außen zurück ist. |
| Die [LosOS-Edge](../types/official-edge) selbst: das Mesh, das Teilen von Rechenleistung, der Markt | Der Bereich Mesh sagt, dass keine Edge gefunden wurde. Einen Schalter, der die Edge braucht, lehnt die Box mit einer Meldung ab, statt ihn halb eingeschaltet zu lassen. |
| Platz, der **mit dem Mesh geteilt** wird    | Der Pool verteilt sich über die Boxen der Edge, also ist, was er enthält, außer Reichweite, solange die Edge es ist. Deine eigenen Dateien liegen nie im Pool. Sie bleiben unberührt auf der Box. |
| **Mehr finden** auf der Seite Apps          | Die Box holt den Katalog aus dem Internet, also zeigt die Suche nichts, bis das Internet zurück ist. Schon installierte Apps laufen weiter. |
| Das Update um 03:00 mit einer `github:`-Quelle | Es kann nichts herunterladen, und das Widget **Änderungen** verzeichnet den Rebuild der Nacht als fehlgeschlagen. Die Box läuft mit dem weiter, was sie hat, und versucht es in der nächsten Nacht wieder. |
| Der Binär-Cache                             | Ein Rebuild, der neue Software braucht, baut sie stattdessen auf der Box, langsam, oder schlägt für diese Nacht fehl. Die nächste erfolgreiche Nacht holt es nach. |
| Einen Besucher über einen öffentlichen Link hereinlassen | Öffentliche Freigabelinks laufen über die öffentliche Adresse der Box; sie funktionieren wieder, wenn die Verbindung nach außen zurück ist. |

## Einen Ausfall von einem Fehler der Box unterscheiden \{#how-to-tell-an-outage-from-a-box-fault}

:::note[Keine Anzeige für das Internet]
Die Box hat keine Anzeige für „das Internet funktioniert“. Was sie hat, sind
die oben aufgeführten Dinge.
:::

1. Öffne die Adresse der Box. Wenn die Übersicht **Antwortet** sagt und die
   App-Kacheln aufgehen, sind die Box und das LAN in Ordnung. Wenn nicht
   einmal das aufgeht, ist nicht der Internetanbieter das Problem. Siehe
   [Die Box ist nicht erreichbar](/troubleshooting/cannot-reach-the-box.md).
2. Öffne den Bereich **Mesh**. Eine Box, die mit der LosOS-Edge verbunden
   ist, sagt, dass keine Edge gefunden wurde, solange die Verbindung nach
   außen weg ist. Eine Box mit einer Firmen-Edge im LAN zeigt sie weiter als
   gefunden. Eine Box, die nie verbunden wurde, zeigt so oder so nichts, was
   für eine [einzelne Box](../types/single-box) normal ist. Wenn das
   Internet eine Edge erreicht, die Box aber nicht, liegt das Problem bei
   der Edge, nicht bei der Leitung. Siehe
   [Keine Edge gefunden](/troubleshooting/edge-not-found.md).
3. Schau nach 03:00 auf das Widget **Änderungen** in der Übersicht. Auf
   einer Box mit einer `github:`-Updatequelle ist ein Rebuild, der während
   eines Ausfalls fehlgeschlagen ist, der Ausfall und kein Fehler der Box.
   Der Eintrag der nächsten Nacht zeigt das.
4. Öffne mit einem Handy im selben WLAN irgendeine Website. Wenn auch das
   fehlschlägt, liegt das Problem bei der Leitung oder beim Router, und die
   Box wartet mit dir. Wenn es funktioniert, fehlt der eigene Uplink der
   Box oder die Edge.

## Wenn die Verbindung nach außen zurückkommt \{#when-the-outside-comes-back}

Nichts zu tun. Der Tunnel zur Edge verbindet sich wieder, der geteilte Pool
kommt zurück, der nächste Lauf um 03:00 holt, was der letzte nicht holen
konnte, und der Bereich Mesh kehrt in seinen üblichen Zustand zurück. Du
gibst nichts neu ein und startest die Box nicht neu. Wenn der Bereich Mesh
eine Stunde, nachdem das Internet zurück ist, immer noch keine Edge zeigt,
siehe [Keine Edge gefunden](/troubleshooting/edge-not-found.md).

## Was du während des Ausfalls lassen solltest \{#what-to-avoid-during-the-outage}

- **Installier nicht neu und setz nicht auf Werkseinstellungen zurück**,
  nur weil der Fernzugriff aufgehört hat. Keins von beiden bringt die
  Verbindung nach außen zurück, und beide werfen lokalen Zustand weg.
- **Schalte das Teilen nicht aus und wieder ein**, um das Mesh zu wecken.
  Die Box lehnt das Einschalten ab, solange keine Edge antwortet, und der
  Pool kommt von selbst zurück.
- **Ändere die Updatequelle nicht**, um eine fehlgeschlagene Nacht zu
  umgehen. Die Standardquelle funktioniert offline, und eine
  `github:`-Quelle wartet.
