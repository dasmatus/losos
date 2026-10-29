---
title: Zugangsdaten
sidebar_position: 4.6
slug: /reference/credentials
---

# Zugangsdaten

Die Box bewahrt einige Geheimnisse für sich selbst auf: den Ersatzschlüssel
zur Verwaltung, die Schlüssel von LosOS Login, das Geheimnis, mit dem LosOS
Mail geöffnet wird, und die Schlüssel deines Sicherungs-Buckets. Keines davon
ist eine einfache Datei. Jedes ist ein Zugangsdatum, verschlüsselt und an
diese Box gebunden, und das System gibt es nur den Diensten, die es lesen.

**Einstellungen → Zugangsdaten** listet sie auf. Dort steht, wofür jedes da
ist, was es liest, woran es gebunden ist und wann es sich zuletzt geändert
hat. Ein Wert wird nie angezeigt, und nichts auf der Seite kann einen
offenlegen.

![Einstellungen, Zugangsdaten: woran die Geheimnisse versiegelt sind, dann jedes mit seinem Zweck, seiner Herkunft, seiner letzten Änderung und wer es liest, und Neu erzeugen bei denen, die sich ersetzen lassen.](@site/docs/img/settings-credentials-de.png)

## Wo sie liegen

Jedes Zugangsdatum ist eine Datei in `/etc/credstore.encrypted`,
verschlüsselt mit `systemd-creds`. Das Verzeichnis bleibt über Neustarts auf
dem verschlüsselten Volume erhalten (siehe
[Was einen Neustart übersteht](what-survives-a-reboot.md)), und ein Löschen
der Box entfernt es.

Eine Datei ist an einen von zwei Schlüsseln gebunden:

- **Das TPM**, wenn die Box eines hat. Der Chip hält den Schlüssel, also ist
  eine Kopie der Datei anderswo nutzlos, und eine Kopie der ganzen Festplatte
  auch. Die Bindung hängt nicht an Firmware-Messwerten, aus dem Grund, den
  die [TPM-Seite](tpm.md) für den Festplattenschlüssel nennt: Die Box
  aktualisiert sich nachts selbst, und niemand ist da, um sie von Hand zu
  entsperren.
- **Der eigene Schlüssel der Box**, `/var/lib/systemd/credential.secret`,
  nur für root lesbar, wenn die Box kein TPM hat. Eine Kopie einer einzelnen
  Datei ist allein weiterhin nutzlos, aber eine Kopie des ganzen Volumes
  nimmt den Schlüssel mit.

Der Bereich zeigt, welchen diese Box verwendet.

Jede Datei trägt außerdem ihren eigenen Namen, der beim Öffnen geprüft wird.
Eine Datei, die über die eines anderen Zugangsdatums kopiert wurde, wird
deshalb abgelehnt statt verwendet.

## Wie ein Dienst eines bekommt

Wenn ein Dienst startet, öffnet systemd die Zugangsdaten, die er bekommen
darf, und legt sie in ein Verzeichnis nur dieses Dienstes, das nur im
Arbeitsspeicher liegt. Der Dienst erfährt, wo er nachsehen soll, und sonst
nichts. Wenn er stoppt, ist das Verzeichnis weg. Kein anderer Dienst und
niemand, der an der Box angemeldet ist, kann es lesen.

## Einen neuen Wert erzeugen

Manche Zugangsdaten lassen sich im Bereich neu erzeugen: **Neu erzeugen**
fragt zuerst, dann erzeugt die Box einen Zufallswert, versiegelt ihn und
startet neu, was ihn liest. Ein neuer Sitzungsschlüssel für LosOS Login zum
Beispiel meldet alle ab.

Die anderen nicht, und der Bereich sagt, warum:

- Der Ersatzschlüssel: Du hast ihn beim Einrichten der Box notiert, und ein
  neuer würde dir einen Schlüssel lassen, der nicht mehr funktioniert.
- Der Speicherschlüssel von LosOS Login: Er verschlüsselt die zweiten
  Faktoren aller, und ein neuer würde sie verlieren.
- Die Schlüssel, die LosOS Login dem Begleiter für KI-Werkzeuge gibt: Ein
  neuer würde jeden Begleiter abmelden.
- Das Geheimnis von LosOS Mail: Die Verwaltung hält es vom Moment ihres
  Starts an. Das Geheimnis, mit dem die Verwaltung LosOS Login fragt, wird
  von selbst neu erzeugt, wann immer es nötig ist.
- Die Schlüssel des Buckets: Die gibst du im Bereich Sicherung ein, auf den
  der Bereich Zugangsdaten verlinkt.

Im Terminal zeigt `losos credentials list` dieselbe Liste, und
`losos credentials rotate <name>` erzeugt eines neu.

## Boxen, die vorher eingerichtet wurden

Eine Box, die vor dem Speicher für Zugangsdaten eingerichtet wurde, hielt
diese Geheimnisse als einfache Dateien unter `/var/secrets`. Beim ersten
Start mit dem Speicher versiegelt sie jeden Wert so, wie er ist, damit nichts
bricht, was ihn schon verwendet, überschreibt dann die einfache Datei und
löscht sie. Der Bereich markiert jedes, das sie übernommen hat.

## Wenn sich ein Zugangsdatum nicht öffnen lässt

Wurde das TPM gelöscht oder ging der eigene Schlüssel der Box verloren, lässt
sich eine versiegelte Datei nicht mehr öffnen. Die Box legt die Datei
beiseite, erzeugt einen neuen Wert, und der Bereich markiert ihn als beim
Start neu erzeugt. Ein neu erzeugter Ersatzschlüssel wird nirgends angezeigt,
melde dich also stattdessen mit deinem Passwort an.

## Wogegen es nicht schützt

Der Speicher schützt, was auf der Festplatte liegt: eine Sicherungskopie,
eine aus der Box genommene Festplatte, einen Fehler, durch den ein Dienst
Dateien lesen kann, die er nicht lesen sollte. Er schützt nicht gegen root
auf der laufenden Box, das das System bitten kann, jedes Zugangsdatum zu
öffnen. Aus diesem Grund hat die Box keine Shell und kein SSH.
