---
title: Teilen wird abgelehnt
sidebar_position: 12
slug: /troubleshooting/sharing-refused
---

# Teilen wird abgelehnt

<div className="losos-symptom">

**Was du siehst:** Der Schalter, mit dem du deine Festplatte teilst, dem
Mesh beitrittst oder freie Zeit verleihst, lässt sich nicht einschalten,
ist ausgegraut, oder Anwenden lehnt die Änderung mit einer Begründung ab.

</div>

## Die Gründe, und welcher bei dir zutrifft \{#the-reasons-and-which-one-you-have}

![Der Bereich Mesh ohne erreichbare Edge: Die Schalter zum Teilen sind ausgegraut mit Braucht einen erreichbaren Edge-Proxy.](@site/docs/img/mesh-none-de.png)

| Was du siehst                                          | Grund                                                                                       | Was zu tun ist                                                             |
| ------------------------------------------------------ | ------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| Schalter ausgegraut, „Braucht einen erreichbaren Edge-Proxy.“; der Bereich Mesh sagt **Kein Edge-Proxy gefunden** | Die Box hat keine Edge gefunden. Das Teilen von Speicher und Rechenleistung braucht ein Ziel, *an das* geteilt wird. | [Keine Edge gefunden](edge-not-found).                                      |
| Anwenden abgelehnt: „no edge proxy is reachable from this box, so `losos.sharingMyStorage` cannot be turned on …“ | Die Edge hat geantwortet, als du den Schalter umgelegt hast, und war weg, als du angewendet hast. Die Box verspricht keinem Pool Platz, den sie nicht erreicht. | Warte, bis der Bereich Mesh die Edge wieder zeigt (er prüft alle 20 Sekunden neu), und wende noch einmal an. Nichts hat sich geändert. |
| „Mit den Gateways dieses Netzwerks teilen“ ausgegraut, „Braucht ein erreichbares lokales Gateway. Ein offizieller Edge reicht nicht.“ | Teilen mit diesem Netzwerk braucht eine Edge, die die Box in ihrem eigenen Netzwerk erreicht. Eine offizielle Edge oder eine, die die Box nur über das Internet erreicht, zählt nicht. | Bring die Box ins selbe Netzwerk wie ihr Gateway; der Bereich Mesh zeigt die lokalen Gateways, die sie sieht. Siehe [Speichermodi](../types/storage-modes#shared-with-this-network-only). |
| Anwenden abgelehnt: „no local gateway is reachable from this box, so `losos.sharingLocally` cannot be turned on …“ | Das lokale Gateway ist zwischen Schalter und Anwenden verschwunden. | Warte, bis es im Bereich Mesh wieder unter **Lokale Gateways** steht, und wende noch einmal an. Nichts hat sich geändert. |
| Die Zeile der Edge trägt ein Warnzeichen; Markt sagt „Der Edge-Proxy, den diese Box gefunden hat, hat die Gateway-Prüfung nicht bestanden“ | Die Edge zeigt keinen Gateway-Pass (meist die eigene Edge einer Firma), oder ihre Prüfung ist gescheitert, oder Gateway-Image und Auszahlungsadressen von LosOS sind noch nicht veröffentlicht. Teilen darüber funktioniert. Handel darüber lehnt die Box absichtlich ab. | Nimm für den Handel ein geprüftes Gateway; der Tooltip der Edge sagt, welche Prüfung gescheitert ist. Siehe [offiziell oder nicht](../types/official-edge#official-or-not). |
| „Tritt zuerst dem Mesh bei.“                           | Freie Zeit zu verleihen setzt voraus, dass die Box im Cluster der Edge ist.                 | Schalte **Dem Mesh beitreten** ein und stell dann die Stunden ein; beides geht mit einem Anwenden raus. |
| Stunden eingestellt, aber nie wird etwas verliehen     | Das Zeitfenster beginnt und endet in derselben Minute, oder die Box ist darin nie untätig.  | Stell ein echtes Zeitfenster ein. Die Zeile unter den Stunden sagt, wie lang es ist. |
| Anwenden schlägt bei einer Mesh-Änderung fehl          | Der Rebuild selbst ist fehlgeschlagen, nachdem die Box die Änderung angenommen hatte.        | [Anwenden schlägt fehl](apply-fails), dann noch einmal versuchen, während die Edge erreichbar ist. |

:::note[Abgelehnt wird nur das Einschalten]
Die Box lehnt immer nur das *Einschalten* ab. Eine Box, deren Edge weg ist,
behält das Teilen, das sie schon hatte, kann weiterhin jede andere
Einstellung ändern und das Teilen jederzeit ausschalten.
:::

## Was das Teilen nie tut \{#what-sharing-never-does}

Es teilt nie deine eigenen Dateien. Geteilter Platz gehört einem eigenen
Konto auf der Box mit eigenem verschlüsseltem Verzeichnis. Deine Dateien und
die Kopien des Mesh können einander nicht lesen. Schaltest du das Teilen
aus, wird dieses Verzeichnis wieder unlesbar, sogar für die Box.
