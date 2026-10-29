---
title: Wenn etwas nicht funktioniert
sidebar_position: 0
sidebar_label: Hier anfangen
slug: /troubleshooting
---

# Wenn etwas nicht funktioniert

Dieses Kapitel geht vom schlimmsten Fall aus, mit dem du noch arbeiten
kannst. **Du bist im selben lokalen Netzwerk wie die Box, und mehr hast du
nicht.** Kein Internet, keine Edge, keine Shell. Jede Seite ist danach
geordnet, was du siehst, und beginnt mit den Schritten, die allein aus dem
LAN funktionieren.

:::note[Wenn nur die Verbindung nach außen fehlt]
Wenn die eigenen Seiten der Box aufgehen und nur die Verbindung nach außen
fehlt, liegt der Fehler nicht bei der Box. [Nur das lokale Netzwerk
funktioniert](/troubleshooting/only-the-lan-works.md) nennt die möglichen
Ursachen, etwa die Leitung des Anbieters, den Uplink des Routers oder die
Edge. Dort steht auch, was weiter funktioniert, was wartet und warum du
danach nichts neu einrichten musst.
:::

## Die ersten fünf Minuten \{#the-first-five-minutes}

Geh diese Schritte der Reihe nach durch. Die meisten Probleme enden bei
einem davon.

1. **Schau auf den Bildschirm der Box.** Ein Banner mit
   `LosOS <version> is ready` und einer `http://`-Adresse bedeutet, dass die
   Box gestartet ist und ein Netzwerk hat. Unter der Adresse stehen die Teile
   von LosOS, die die Box betreibt, mit einem Haken für jeden, der läuft. Ein
   Teil, der nicht läuft, bekommt ein Kreuz, und die Überschrift wird zur
   Warnung. `LosOS <version> cannot start its admin pages` heißt, dass die
   Adresse eine Seite öffnet, die nicht laden kann, bis sie starten. Kein
   Banner, eine Login-Eingabe im Textmodus oder eine Abfrage nach einer
   Passphrase bedeuten ein anderes Problem. Siehe [Hängt an der Passphrase-Abfrage](/troubleshooting/stuck-at-passphrase.md)
   oder bei schwarzem Bildschirm [Die Box ist nicht erreichbar](/troubleshooting/cannot-reach-the-box.md).
2. **Nimm die IP-Adresse, nicht den Namen.** Tipp das `http://192.168.…`
   aus dem Banner in einen Browser im selben Netzwerk. Die Hälfte aller
   Meldungen „die Box ist ausgefallen“ kommt von einem Rechner, der `.local`
   nicht auflöst. Siehe [Der Name wird nicht aufgelöst](/troubleshooting/name-does-not-resolve.md).
3. **Versuch es mit einfachem `http://`.** Eine `https://`-Adresse mit einer
   Zertifikatswarnung ist kein Ausfall. Siehe [Zertifikatswarnung](/troubleshooting/certificate-warning.md).
4. **Frag die Box, ob sie lebt.** Öffne `http://<address>/api/health`. Die
   Antwort `{"ok":true}` bedeutet, dass der Steuerdienst läuft. Öffne
   `http://<address>/setup/state.json`, um Namen und Adresse der Box so zu
   sehen, wie sie selbst sie sieht.
5. **Warte auf den nächsten Neustart oder lös einen aus.** Die Box startet
   sich um 00:07 selbst neu und baut dabei ihre Systemplatte von Grund auf
   neu auf. Stecker ziehen und wieder einstecken bewirkt dasselbe sofort.
   Ein Neustart berührt deine Dateien nicht, sie liegen auf dem
   verschlüsselten Volume.

## Was dir die Box im LAN verrät \{#what-the-box-can-tell-you-from-the-lan}

![Die Übersicht, wenn die Box ihren Gesundheitscheck nicht beantwortet: Der Status unter dem Namen wechselt von Antwortet zu Antwortet nicht.](@site/docs/img/overview-not-answering-de.png)

| Wo                                      | Was                                                                                          |
| --------------------------------------- | -------------------------------------------------------------------------------------------- |
| Der Bildschirm (tty1)                   | Die IP-Adresse, der `.local`-Name und welche Teile von LosOS laufen. Wird neu gezeichnet, wenn sie sich ändern. |
| `http://<address>/api/health`           | Ob der Steuerdienst antwortet.                                                               |
| `http://<address>/setup/state.json`     | Name, Adresse, ob HTTPS an ist, der Fingerabdruck des Zertifikats.                           |
| Seite Übersicht                         | Antwortet oder nicht, Laufzeit, Apps an oder aus, das Widget **Änderungen** mit dem Ergebnis jedes Rebuilds. |
| Bereich Über                            | Name, Adresse, Speichermodus, Zustand des Mesh, die beiden Timer.                            |
| Bereich Speicher                        | Wie voll die Festplatte ist.                                                                 |
| Bereich Mesh                            | Ob eine Edge gefunden wurde, und welche Art.                                                 |

## Was sie dir nicht verraten kann \{#what-it-cannot-tell-you}

Es gibt keinen Log-Viewer und kein Terminal, weil es keine Shell gibt. Wenn
nichts davon ein Problem erklärt, gilt eins von zwei Dingen. Entweder
behebt es der nächtliche Neustart von selbst, oder die Box muss neu
installiert werden. [Hilfe bekommen](/troubleshooting/getting-help.md) sagt,
was du sammeln solltest, bevor du fragst.

## Die Seiten \{#the-pages}

- [Nur das lokale Netzwerk funktioniert](/troubleshooting/only-the-lan-works.md)
- [Die Box ist nicht erreichbar](/troubleshooting/cannot-reach-the-box.md)
- [Der Name wird nicht aufgelöst](/troubleshooting/name-does-not-resolve.md)
- [Zertifikatswarnung](/troubleshooting/certificate-warning.md)
- [Passwort vergessen](/troubleshooting/forgot-password.md)
- [Hängt an der Passphrase-Abfrage](/troubleshooting/stuck-at-passphrase.md)
- [Der Assistent wartet und wartet](/troubleshooting/wizard-waits.md)
- [Eine App-Kachel liefert 404](/troubleshooting/app-tile-404.md)
- [Anwenden schlägt fehl](/troubleshooting/apply-fails.md)
- [Kein Platz mehr](/troubleshooting/out-of-room.md)
- [Keine Edge gefunden](/troubleshooting/edge-not-found.md)
- [Teilen wird abgelehnt](/troubleshooting/sharing-refused.md)
- [Hilfe bekommen](/troubleshooting/getting-help.md)
