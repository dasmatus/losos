---
title: Keine Edge gefunden
sidebar_position: 11
slug: /troubleshooting/edge-not-found
---

# Keine Edge gefunden

<div className="losos-symptom">

**Was du siehst:** Der Bereich Mesh sagt **Kein Edge-Proxy gefunden** und
**Teilen ist aus**; die Schalter für das Mesh und das Teilen der Festplatte
sind ausgegraut mit „Braucht einen erreichbaren Edge-Proxy.“; ein Anwenden
wurde mit „no edge proxy is reachable from this box“ abgelehnt; der
öffentliche Name der Box antwortet nicht mehr.

</div>

## Wonach die Box sucht \{#what-the-box-looks-for}

![Was die Box alle 20 Sekunden versucht: das LAN per DNS-SD und die konfigurierte Adresse; keine Antwort von beiden bedeutet Kein Edge-Proxy gefunden.](@site/docs/img/edge-discovery-de.svg)

Alle 20 Sekunden sucht die Box an zwei Stellen nach einer Edge. Sie horcht
im lokalen Netzwerk auf eine Ankündigung, so findet sie eine
[Firmen-Edge](../types/company-edge) im selben LAN. Außerdem fragt sie die
konfigurierte Adresse, so erreicht sie die [LosOS-Edge](../types/official-edge)
oder eine Firmen-Edge an einem anderen Ort. Eine Edge zählt, sobald ihr
`/health` antwortet. Der Bereich Mesh listet jede Edge, die die Box gefunden
hat, jede mit einem Zeichen, ob sie sich als offiziell ausgewiesen hat. Hat
sie keine gefunden, sagt der Satz unter **Kein Edge-Proxy gefunden**, was
die Box versucht hat:

| Der Bereich sagt                                                           | Bedeutung                                                                     |
| -------------------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| „Diese Box hat ihr Netzwerk durchsucht und `<address>` gefragt; nichts hat geantwortet.“ | Die Box hat beides versucht. Die Adresse ist die konfigurierte.               |
| „Diese Box hat ihr Netzwerk durchsucht, und nichts hat geantwortet.“       | Es ist keine Adresse konfiguriert, also hat die Box nur das lokale Netzwerk durchsucht. |
| „Diese Box konnte ihr Netzwerk nicht durchsuchen, und anderswo hat nichts geantwortet.“ | Der mDNS-Dienst der Box lief nicht, also konnte sie keine Edge im LAN finden, selbst wenn eine da war. Ein Neustart bringt den Dienst zurück. |

## Aus dem LAN \{#from-the-lan}

![Der Bereich Mesh mit Kein Edge-Proxy gefunden, dazu was die Box durchsucht und gefragt hat.](@site/docs/img/mesh-none.png)

1. **Hat die Box Internet?** Eine Edge im Internet braucht es, eine
   Firmen-Edge im LAN nicht. Die Übersicht sagt, ob die Box antwortet, aber
   eine Anzeige für „Internet“ hat die Box nicht. Probier die Adresse der
   Edge in deinem eigenen Browser aus demselben Netzwerk. Wenn nur das
   lokale Netzwerk funktioniert, siehe
   [Nur das lokale Netzwerk funktioniert](only-the-lan-works).
2. **Stimmt die Adresse?** Öffne Einstellungen → Erweitert und such
   `losos.proxy.registrarUrl`. Das ist die Basisadresse der API der Edge,
   `https://losos-edge.dasmat.us` für die Steuerungsebene der LosOS-Edge.
   Eine Firmen-Edge im LAN kündigt ihre eigene an, meist
   `http://<edge>.local:8443`.
3. **Firmen-Edge im LAN: dasselbe Netzwerksegment?** mDNS-Ankündigungen
   gehen nicht über Router oder VLANs hinweg. Trag die Adresse der Edge ein,
   statt dich auf die automatische Suche zu verlassen.
4. **Kündigt sich die Edge an?** Boxen finden eine Firmen-Edge im LAN nur,
   wenn `losos.edge.lan.advertise` an ist. Ohne diese Option brauchen Boxen
   die Adresse der Edge. Wer die Edge betreibt, setzt diese Option.
5. **Ist diese Box angemeldet?** Eine Edge nimmt nur Boxen auf ihrer
   Zulassungsliste an, jede mit einem Token. Die Suche braucht keine
   Anmeldung, der Tunnel schon. Eine neu installierte Box hat eine neue
   Identität, und wer die Edge betreibt, muss sie neu anmelden.
6. **Wurde die Edge neu installiert?** Die Box merkt sich den
   Tunnelschlüssel der Edge beim ersten Kontakt und lehnt eine neu
   aufgesetzte Edge mit neuem Schlüssel ab, und zwar absichtlich. Der
   Betreiber löscht entweder den gemerkten Schlüssel auf der Box oder
   meldet die Box neu an. Der gemerkte Schlüssel ist eine Datei unter den
   Geheimnissen der Box, und nur ein Rebuild kommt an sie heran.

## Die Box verbindet sich von selbst wieder \{#the-box-reconnects-on-its-own}

Sobald die Edge wieder antwortet, findet die nächste Suche sie innerhalb
von 20 Sekunden, und der Bereich Mesh zeigt sie ein paar Sekunden später.
Der Tunnel kommt ohne Neustart zurück. Ein Ausfall schaltet nichts ab, was
du eingeschaltet hast. Solange die Box keine Edge findet, lehnt sie nur das
*Einschalten* ab, und darum geht es unter
[Teilen wird abgelehnt](sharing-refused).
