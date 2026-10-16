---
title: Eine Box für sich allein
sidebar_position: 1
---

# Eine Box für sich allein

Das ist das einfachste LosOS, und jede Box beginnt hier nach dem ersten
Start. Es ist ein Rechner in einem lokalen Netz. Die Computer und Handys in
diesem Netz erreichen ihn, sonst niemand.

## Was du bekommst \{#what-you-get}

- LosOS cloud und LosOS Git unter der Adresse der Box, für alle im Netz, die
  ein Konto haben.
- Die Admin-Seiten, entsperrt mit dem Passwort des Besitzers.
- Nächtliche Selbstreparatur und Selbstaktualisierung, eine verschlüsselte
  Festplatte, die Härtungsbasis.
- Das Handbuch, das du gerade liest, unter `/handbook/`.

## Was du nicht bekommst \{#what-you-do-not-get}

- **Kein Zugriff von außen.** Die Box veröffentlicht nichts und öffnet keinen
  Port auf deinem Router. Um deine Dateien aus einem Café zu erreichen,
  brauchst du die [LosOS-Edge](official-edge) oder ein eigenes VPN ins
  Heimnetz.
- **Kein Mesh.** Die Box hat niemanden, dem sie Festplatte oder CPU leihen
  könnte, und die Schalter im Bereich Mesh bleiben aus. Ohne erreichbare Edge
  verweigert die Box das Teilen von Speicher, das vom Netz abhängt, sodass du
  den Schalter nicht aus Versehen einschalten kannst.
- **Kein Markt.**

![Der Bereich Mesh auf einer Box ohne Edge: Kein Edge-Proxy gefunden, mit dem, was die Box versucht hat, und die ausgegrauten Mesh-Schalter mit Braucht einen erreichbaren Edge-Proxy.](@site/docs/img/mesh-none-de.png)

## Wann diese Art die richtige ist \{#when-this-is-the-right-type}

Für die meisten Haushalte. Eine Box für sich allein hat die wenigsten
Zugänge. Der einzige ist ein Browser in deinem LAN. Updates kommen trotzdem
an, weil die Box sie selbst holt, und der Weg der Updates braucht keine Edge.

## Einstellungen, auf die es ankommt \{#settings-that-matter}

| Bereich      | Einstellung                 | Standard                                         |
| ------------ | --------------------------- | ------------------------------------------------ |
| Netzwerk     | Name                        | `losos`, bis du im Assistenten einen festlegst    |
| Netzwerk     | Verbindung verschlüsseln    | an; das Zertifikat ist das eigene der Box         |
| Netzwerk     | Auch von außerhalb erreichbar | aus, und genau das macht sie zu einer Box für sich allein |
| Apps         | LosOS Git                   | an                                                |
| Hardware     | Grafik                      | aus, außer die Box hat eine GPU, die Pods nutzen sollen |
| Sicherheit   | die vier zusätzlichen Schutzmaßnahmen | aus; jede kostet etwas                  |

Schalte **Auch von außerhalb erreichbar** ein, während eine Edge erreichbar ist, und
die Box wird zur nächsten Art. Neu installiert wird dabei nichts.
