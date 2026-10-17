---
title: Webseite der Edge
sidebar_position: 6.5
mdx:
  format: md
---

# Webseite der Edge

Eine Firmen-Edge hat unter `/manage/` eine Webseite für die Leute, die sie
betreuen. Sie listet die Boxen, die die Edge weiterleitet, und ändert auf
jeder Box, deren Besitzer es erlaubt, die Einstellungen von LosOS cloud und
LosOS Git und installiert oder entfernt Apps. Die offiziellen LosOS-Edges
haben diese Seite nicht, und eine Box lehnt es ab, von einer verwaltet zu
werden.

Die Seite ist das Werkzeug der Firma, nicht des Besitzers. Für alles andere
hat der Besitzer weiter die Admin-Seiten der Box, und er entscheidet auf der
Box, ob die Edge sie überhaupt verwalten darf.

## Einschalten

Setzen Sie auf der Edge `losos.edge.manage.enable`. Ein
[Edge-Gateway-Rechner](edge-federation.md) hat es schon an.

Beim ersten Start erzeugt die Edge einen Verwaltungsschlüssel: 64 Zeichen,
`0` bis `9` und `a` bis `f`, abgelegt in
`/var/lib/losos-registrar/manage-key`. Wer den Schlüssel hat, kann die Seite
benutzen, also bewahren Sie ihn bei den anderen Admin-Passwörtern der Firma
auf. Auf einem Edge-Gateway-Rechner gibt `losos-edge manage-key` auf der
Konsole den Schlüssel mit der Adresse der Seite aus. Um den Schlüssel zu
wechseln, löschen Sie die Datei und starten den Registrar neu. Die Edge
erzeugt einen neuen, und der alte gilt nicht mehr.

Die Seite antwortet unter der Adresse der Edge im lokalen Netz, unter der
die Boxen sie finden, zum Beispiel `http://<edge>.local:8443/manage/`.

## Anmelden

Die Seite fragt nach dem Verwaltungsschlüssel und prüft ihn bei der Edge,
bevor sie weitermacht. Sie behält den Schlüssel nur im Browser-Tab, also
meldet das Schließen des Tabs Sie ab. **Abmelden** oben tut dasselbe sofort.

Sagt die Edge, sie kenne den Schlüssel nicht, ist er vertippt oder wurde
gewechselt. Sagt sie, sie verwalte keine Boxen, ist
`losos.edge.manage.enable` aus.

## Die Liste der Boxen

![Die Liste der Boxen auf der Edge: zwei hier verwaltet, eine, deren Besitzer die Verwaltung nicht eingeschaltet hat.](@site/docs/img/edge-manage-boxes-de.png)

**Boxen** listet jede Box, die die Edge weiterleitet, mit ihrem Namen und
wann die Edge zuletzt von ihr gehört hat. Die Liste lädt alle 10 Sekunden
neu.

- **Hier verwaltet** heißt, der Besitzer der Box hat auf ihrer Mesh-Seite
  **Den Firmen-Edge diese Box verwalten lassen** eingeschaltet. **Verwalten**
  öffnet die Box.
- **Nicht eingeschaltet** heißt, der Besitzer hat das nicht getan. Die Edge
  leitet die Box weiter, kann sie aber nicht ändern. Braucht die Firma das,
  fragen Sie den Besitzer.

Eine Box sagt bei jedem Heartbeat, ob ihr Besitzer es erlaubt, also zeigt
sich eine Änderung auf der Box hier innerhalb eines Heartbeats. Eine Box,
die keine Heartbeats mehr schickt, fällt aus der Liste.

## Die Seite einer Box

![Die Seite einer Box: Einstellungen von LosOS cloud und LosOS Git, die installierten Apps und die App-Suche.](@site/docs/img/edge-manage-box-de.png)

Die Seite einer Box hat drei Teile, dieselben wie der Bereich Apps auf der
Box selbst:

- **Einstellungen von LosOS cloud und LosOS Git.** Standardsprache,
  Telefonregion, wie lange gelöschte Dateien und alte Versionen bleiben, das
  Standardkontingent, öffentliche Links mit Passwort und Ablauf, und bei
  LosOS Git Anmeldung, Privatsphäre, Startseite und Push. Die Föderation mit
  anderen Boxen ist auch hier. Eine Einstellung, die die Box in ihrer eigenen
  Konfiguration festgelegt hat, erscheint nicht.
- **Installierte Apps.** Was auf der Box läuft, mit einer Schaltfläche zum
  Entfernen jeder App.
- **App-Suche.** Die Suche im App-Store, mit einer Schaltfläche zum
  Installieren bei jedem Treffer.

Geänderte Einstellungen sammeln sich in einer Leiste unten. **Anwenden**
schickt sie an die Box, die sie prüft, in ihrer Konfigurationsgeschichte
speichert und neu baut, genau wie wenn der Besitzer eine Änderung auf der
Box anwendet. Die Leiste zeigt den Neubau, bis er fertig ist.

## Was die Edge nicht kann

Jeder Aufruf der Seite geht durch den eigenen Tunnel der Box zur Box, und
die Box entscheidet. Sie antwortet nur, wenn der Aufruf das Tunnel-Token der
Box trägt und der Schalter des Besitzers genau diese Edge nennt. Dann lässt
sie die Edge an die App-Einstellungen und an das, was jede angemeldete Seite
liest, an nichts sonst.

Die Edge kann deshalb nicht:

- Dateien von jemandem öffnen, weder in LosOS cloud, LosOS Git noch sonstwo;
- Personen anlegen, ändern oder entfernen oder Passwörter zurücksetzen;
- Speicher, Sicherungen, Netzwerk, Teilen oder Updates ändern;
- den Schalter einschalten oder ihn an lassen, wenn der Besitzer ihn
  ausschaltet.

Eine veränderte Seite auf der Edge kommt nicht weiter, weil die Box jeden
Aufruf selbst prüft. Der Schalter gehört zu der Edge, die die Box nutzte,
als der Besitzer ihn einschaltete. Wechselt die Box später zu einer anderen
Edge, erbt diese ihn nicht. Schaltet der Besitzer den Schalter aus, lehnt
die Box den nächsten Aufruf der Edge ab und erscheint nach ihrem nächsten
Heartbeat als **Nicht eingeschaltet**. Eine in dem Moment offene Seite der
Box meldet, dass die Box die Edge abgelehnt hat.
