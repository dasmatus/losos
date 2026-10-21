---
title: KI-Modelle
sidebar_position: 4.6
mdx:
  format: md
---

# KI-Modelle

Boxen im Mesh können Sprachmodellen Arbeitsspeicher leihen, die für jede
einzelne von ihnen zu groß sind. Der Edge fasst verleihende Boxen zu einer
Gruppe zusammen, bis ein Modell hineinpasst, betreibt das Modell über die
ganze Gruppe und verkauft seine Antworten pro Token. Der Käufer zahlt
laufend in USDC, aus einem Guthaben, das der
[Escrow](/in-depth/market.md#paying-with-usdc) hält; jede Rechnung wird
zwischen den Boxen, die die Arbeit gemacht haben, und der Treasury von
LosOS geteilt.

Die Modelle laufen auf [llama.cpp](https://github.com/ggml-org/llama.cpp).
Jede verleihende Box betreibt dessen RPC-Server, der einen Teil der
Schichten des Modells hält und darauf rechnet. Der Edge betreibt pro Gruppe
einen llama.cpp-Server, der jeden Teil über den Tunnel der jeweiligen Box
erreicht und dem Käufer antwortet.

## Arbeitsspeicher verleihen {#lending-memory}

Das Verleihen hat einen eigenen Schalter, `losos.inference.lend.enable`.
Wie viel Arbeitsspeicher die Box verleiht, legt
`losos.inference.lend.memoryMiB` fest, standardmäßig 4096 MiB, und mit wie
vielen Prozessor-Threads sie rechnet, `losos.inference.lend.threads`. Ohne
eine Thread-Zahl nimmt sie die Hälfte der Threads der Box.

- Der RPC-Server läuft als Benutzer `shared`, derselbe Benutzer wie der
  Mesh-Speicher, und sieht die Dateien des Besitzers nie.
- Er lauscht nur auf Loopback. Eine Firewall-Regel lässt nur root, der den
  Tunnel betreibt, an seinen Port; jeder andere Benutzer auf der Box wird
  abgewiesen.
- Er kann nicht mehr Arbeitsspeicher nehmen, als der Besitzer eingestellt
  hat. Die Unit hat ein hartes Speicherlimit, und wächst der Server darüber
  hinaus, beendet systemd den Server, nie LosOS cloud.
- Er läuft mit niedrigem Prozessor- und Festplattengewicht, damit die
  eigene Arbeit des Besitzers Vorrang hat.
- Der Edge erreicht ihn über einen zweiten Dienst im bestehenden Tunnel der
  Box. Eine Box ohne Tunnel zum Edge kann nicht verleihen.

Das Rechenfenster gilt hier nicht: Eine Box verleiht, solange der Schalter
an ist.

Damit der Edge sie auswählen kann, braucht eine Box außerdem eine
Auszahlungsadresse in dem Netz, in dem der Käufer zahlt, also EVM oder
Solana, und ihr Edge muss sie verkaufen lassen. Eine Box, die über den
eigenen Edge von jemandem im LAN erreicht wird, kann noch nicht verleihen.

## Gruppen {#groups}

Für jedes Modell und jedes Zahlungsnetz hält der Edge höchstens eine
Gruppe. Er wählt verleihende Boxen mit dem meisten Arbeitsspeicher zuerst,
bis zu 16, bis ihr Speicher zusammen deckt, was das Modell braucht. Eine
Box ist immer nur in einer Gruppe. Alle Käufer desselben Modells im
selben Netz nutzen dieselbe Gruppe.

| Zustand     | Bedeutung                                                                       |
| ----------- | ------------------------------------------------------------------------------- |
| Lädt        | Der Server des Edge startet und schickt das Modell an die Boxen.                |
| Bereit      | Das Modell antwortet.                                                           |
| Ausgefallen | Eine Box ist weggefallen. Die Gruppe startet neu, sobald jede Box zurück ist.   |

Eine Gruppe, die mehr als fünfmal neu startet oder länger als 30 Minuten
zum Laden braucht, gibt der Edge auf, und ihre Boxen bleiben eine Stunde
lang aus neuen Gruppen draußen. Fehlt eine Box länger als 15 Minuten, endet
die Gruppe. Eine Gruppe, die eine Stunde lang niemand genutzt hat, wird
aufgelöst.

Das erste Laden schickt die Schichten des Modells an jede Box. Der
RPC-Server behält sie in seinem Cache, sodass das nächste Laden desselben
Modells schneller geht.

## Laufend bezahlen {#paying-as-you-go}

Ein Käufer lädt einmal ein USDC-Guthaben im Escrow auf, in einem Netz, und
kann dann jedes Modell in diesem Netz ohne weitere Zahlung nutzen.

1. Der Käufer wählt ein Modell und wie viel er pro Stunde höchstens
   ausgeben will, zwischen 0,01 $ und 100 $.
2. Der Edge antwortet mit einem Angebot: ein **Tap** pro Box in der Gruppe.
   Ein Tap nennt einen Verkäufer und den Betrag, den der Edge darüber pro
   Stunde höchstens abrechnen darf, also das Stundenlimit des Käufers mal
   den Anteil dieser Box am Arbeitsspeicher. Das Angebot gilt 30 Minuten.
3. Die Wallet des Käufers eröffnet alle Taps in einer Transaktion.
4. Der Edge liest die Taps von der Chain zurück, prüft, dass jeder den
   Verkäufer, den Satz und die Gebühr aus seinem Angebot nennt, und beginnt
   zu antworten.
5. Der Edge rechnet jede Antwort nach ihren Tokens ab, zum Preis des
   Modells für eine Million Prompt-Tokens und eine Million erzeugte Tokens,
   und teilt die Rechnung nach dem Anteil jeder Box auf die Taps auf.

Die Tokens zählt der Edge selbst. Er setzt den Prompt zusammen und zählt
ihn, bevor er ihn weiterschickt, und begrenzt die Antwort auf die Zahl von
Tokens, die die Anfrage höchstens verlangt hat, oder auf 256, wenn sie
keine nennt. Die Boxen rechnen nur Schichten und melden nichts, also kann
keine Box eine Rechnung erhöhen. Eine fehlgeschlagene Antwort wird nicht
berechnet. Ein Zähler beantwortet immer nur eine Anfrage auf einmal.

Vor jeder Antwort prüft der Edge, dass das Guthaben das Höchste deckt, was
diese Antwort kosten kann, und dass jeder Tap unter seinem Stundensatz noch
Spielraum hat. Ein neuer Tap beginnt mit einer Stunde Spielraum, sodass der
Käufer sofort bedient wird.

### Abrechnen {#settling}

Der Edge rechnet jeden Tap von Zeit zu Zeit auf der Chain ab:

| Netz    | Wann                                                    |
| ------- | ------------------------------------------------------- |
| Solana  | Jede Stunde.                                            |
| EVM     | Einmal am Tag, oder sobald auf einem Tap 1 $ offen ist. |

Ein geschlossener Tap, ein Zähler, der endet, und ein Guthaben, dessen
Auszahlung der Käufer verlangt hat, werden sofort abgerechnet. Jede
Abrechnung nimmt den Betrag aus dem Guthaben des Käufers, schickt die
Gebühr von 4 % an die Treasury und den Rest an die Auszahlungsadresse der
Box.

### Was der Escrow durchsetzt {#what-the-escrow-enforces}

Der Escrow traut dem Edge nicht mehr zu, als der Käufer erlaubt hat:

- Eine Abrechnung kann nur den Verkäufer bezahlen, den der Tap nennt.
- Sie kann höchstens den Satz des Taps für die Zeit seit der letzten
  Abrechnung abrechnen, dazu die Stunde, mit der ein neuer Tap beginnt, und
  nie mehr als sieben Tage davon auf einmal.
- Sie kann nur abrechnen, was das Guthaben hergibt.
- Nachdem ein Tap geschlossen ist, kann sie nur die Zeit bis zum Schließen
  abrechnen.

Ein gestohlener Schlüssel des Edge kostet einen Käufer also höchstens sein
eigenes Stundenlimit für die Zeit, bis er seine Taps schließt.

### Schließen und auszahlen {#closing-and-withdrawing}

Der Käufer kann einen Zähler im Markt-Bereich anhalten oder jeden Tap aus
seiner Wallet schließen; die Taps eines Zählers, der endet, schließt der
Edge. Um Geld herauszunehmen, beantragt der Käufer eine Auszahlung, wartet
48 Stunden und zahlt dann aus. In diesen 48 Stunden kann der Edge noch
abrechnen, was genutzt wurde. Eine Auszahlung braucht den Edge nie, sodass
ein Guthaben erreichbar bleibt, auch wenn der Edge verschwindet.

Der Markt-Bereich warnt, wenn das Guthaben beim aktuellen Stundenlimit
weniger als 24 Stunden deckt.

## Was es nicht kann {#what-it-cannot-do}

- **Die Boxen einer Gruppe sehen die Arbeit.** Jede Box rechnet mit dem
  Prompt, während er durch die Schichten des Modells läuft. Schick nichts,
  was die Box eines Fremden nicht sehen darf.
- **Über Heimanschlüsse ist es langsam.** Jedes Token läuft durch jede Box
  der Gruppe, also antwortet ein Modell, das auf Boxen an Heimanschlüssen
  verteilt ist, weit langsamer als dasselbe Modell auf einem Rechner.
- **Große Modelle laden langsam.** Das erste Laden schickt Gigabytes an
  jede Box.
- **Gruppen haben höchstens 16 Boxen**, also ist das größte Modell, das ein
  Edge anbieten kann, durch das begrenzt, was 16 Boxen verleihen.
- **Karten werden für Modelle nicht angeboten.** Speicher, Rechenleistung
  und Maschinen werden weiter pro bezahltem Zeitraum verkauft, wie der
  [Marktplatz](/in-depth/market.md) beschreibt.
