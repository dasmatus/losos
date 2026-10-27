---
title: Die Apps
sidebar_position: 4
slug: /manual/apps
---

# Die Apps

## LosOS Cloud \{#losos-cloud}

Deine Dateien, Fotos, Kalender, Kontakte und Boards, unter
`https://<address>/cloud/`. Jeder, der sich mit LosOS Login anmelden kann
([Anmelden](sign-in-and-spare-key)), hat eigene. Der Nextcloud-Desktop-Client
und die Handy-Apps synchronisieren mit ihm, und Kalender-Apps erreichen ihn
über CalDAV und CardDAV. Die Einzelheiten stehen in
[LosOS Cloud](losos-cloud). Auf der Übersicht öffnen ihn die Kacheln
**Dateien** und **Fotos**.

## LosOS Git \{#losos-git}

Deine Repositorys, unter `http://<address>/git/<konto>/<repository>.git`.
LosOS Git ist der eigene Git-Server der Box. Wen LosOS Login kennt, hat
darin ab dem ersten Besuch ein Konto, unter demselben Namen. Ein Rechner
klont und pusht mit einem eigenen Token, das git als Passwort abfragt:
`losos git setup` erstellt eines ([Der Terminal-Client](terminal)). Die
eigene Konfiguration der Box ist das private Repository
`notshared/losos-config` ([Administration](../in-depth/administration)).
Clone-Adressen folgen der Adresse, über die du die Box erreicht hast.
Schalte es im Bereich
**Apps** ab, wenn die Box nicht für Code gedacht ist.

## Der Bereich Apps \{#the-apps-pane}

Für jede App zeigt der Bereich, ob sie an ist, und einen Link. Darunter haben LosOS
Cloud und LosOS Git je einen eigenen ausklappbaren Abschnitt, geschlossen,
bis du ihn öffnest. Darunter durchsucht
**Mehr finden** [Artifact Hub](https://artifacthub.io) nach Helm-Charts. Die
Box holt die Ergebnisse selbst, also braucht die Suche das Internet. Jede
Zeile nennt, wer das Chart veröffentlicht hat. Niemand auf der Seite von
LosOS hat eines davon geprüft.

![Der Bereich Apps mit einer installierten App über Mehr finden und Suchergebnissen aus Artifact Hub, jedes mit dem Herausgeber und einer Schaltfläche Installieren oder Ändern.](@site/docs/img/apps-find-more-de.png)

## Einstellungen für alle \{#settings-for-everyone}

In den Abschnitten LosOS Cloud und LosOS Git hat der Bereich Apps die
Einstellungen, die für jedes Konto in der jeweiligen App gelten.

Für LosOS Cloud: wie viele Tage der Papierkorb eine gelöschte Datei behält.
Ein leeres Feld behält sie 30 Tage.

Für LosOS Git: ob nur angemeldete Konten überhaupt etwas sehen, ob ein
neues Repository privat, öffentlich oder wie das letzte des Kontos beginnt
und ob ein Push in ein noch nicht vorhandenes Repository es anlegt.

![Der Bereich Apps mit den Einstellungen für alle in LosOS Cloud und LosOS Git.](@site/docs/img/apps-site-settings-de.png)

Drücke **Anwenden**, um sie zu übernehmen, wie bei jeder anderen
Einstellung. Die Box setzt sie bei jedem Start der App erneut, sodass eine
Änderung in der App selbst nicht hält. Die eigenen Einstellungen jedes
Kontos bleiben in der App.

## Eine App aus der Suche installieren \{#installing-an-app-from-the-search}

Eine Zeile, die die Box holen kann, hat eine Schaltfläche **Installieren**.
Sie öffnet einen Dialog in zwei Schritten.

Der erste Schritt liest das Chart und zeigt drei Dinge.

- **Name auf dieser Box.** Er beginnt als Name des Charts. Er benennt auch
  den Ordner der App und kann sich deshalb später nicht ändern.
- **Ausführen als.** `notshared` oder `shared`, die beiden Benutzer der Box.
  Eine `notshared`-App hält ihre Daten auf der Seite der Festplatte, die dem
  Besitzer gehört, neben deinen Dateien. Eine `shared`-App hält sie auf der
  Seite, die die Box an das Mesh verleiht. Der Dialog graut `shared` aus,
  solange die Box ihre Festplatte nicht teilt.
- **Eigenschaften.** Jede Einstellung aus der Values-Datei des Charts, mit
  ihrem Standardwert, nach Abschnitten eingeklappt. Liefert das Chart ein
  Schema mit, bekommt jede Einstellung auch ihre Beschreibung und ihre
  Auswahlmöglichkeiten. **Als Text** zeigt stattdessen die ganze
  Values-Datei zum Bearbeiten. Eine Eigenschaft, die du im Formular änderst,
  hat Vorrang vor derselben Zeile im Text.

![Der Installationsdialog: der Name, die Wahl zwischen notshared und shared mit ausgegrautem shared, und die Eigenschaften des Charts nach Abschnitten eingeklappt.](@site/docs/img/apps-install-de.png)

Der zweite Schritt fragt noch einmal nach. Er sagt, dass die Box Code
herunterlädt und ausführt, den der Herausgeber geschrieben hat, als der von
dir gewählte Benutzer und ohne Admin-Rechte. Die Box holt nichts, bis du
dort **Installieren** drückst.

Was die App ausführt, hängt vom Benutzer ab. Eine `notshared`-App läuft
auf Podman ohne root, als `notshared` selbst, und braucht keinen Cluster.
Eine `shared`-App läuft im eigenen Cluster der Box, weil das der Teil der
Box ist, den der Verbund berührt. In beiden Fällen ändert die Box das
Chart so, dass es so läuft wie LosOS Git.

- Jeder Teil der App läuft als der gewählte Benutzer, nie als root, ohne
  zusätzliche Rechte und ohne Zugriff auf einen Cluster.
- Der Speicher, den das Chart anfordert, wird zu Ordnern unter
  `/home/<user>/data/apps/<name>`.
- Die App bekommt einen eigenen Port aus `losos.apps.ports`, standardmäßig
  30000 bis 30099, und nur dein lokales Netz erreicht ihn. Der Port öffnet
  sich, sobald die App läuft, und schließt sich, wenn sie anhält.

:::note[Charts, die die Box ablehnt]
Ein Chart, das mehr braucht, lehnt die Box ab und sagt, warum. Typische
Gründe sind Admin-Rechte, ein Port unter 1024, ein Port, den schon eine
andere App benutzt, oder ein Ordner des Systems. Die Dienste einer
`notshared`-App finden einander über ihren Namen direkt auf der Box, also
müssen sie auf verschiedenen Ports lauschen. Eine `shared`-App aus mehreren
Diensten, die einander über ihren Namen aufrufen, startet vielleicht nicht,
weil der eigene Cluster der Box kein internes Netz hat. Ein geplanter Job
(ein CronJob) läuft nur als `shared`.
:::

**Installierte Apps** über der Suche listet jede App mit ihrem Zustand.
**Öffnen** geht zum Port der App auf der Adresse, die du gerade benutzt.
**Ändern** öffnet denselben Dialog mit den aktuellen Eigenschaften der App.
**Entfernen** fragt zuerst nach und nimmt die App dann von der Box. Ihr
Datenordner bleibt, sodass eine Installation unter demselben Namen die Daten
wiederfindet.

:::note[Als shared nur mit einem Mesh]
Eine `notshared`-App lässt sich auf jeder Box installieren. `shared`
braucht den eigenen Cluster der Box, der nur läuft, solange die Box ihre
Festplatte mit einem Mesh teilt ([Wo die Apps laufen](../types/app-modes)). Um das Installieren abzuschalten,
setze `losos.apps.enable` im Bereich **Erweitert** auf false.
:::

## Statistik \{#statistics}

**Einstellungen › Statistik** zeigt, wie oft deine installierten Apps
aufgerufen wurden. Du siehst die Anfragen der letzten Stunde, Minute für
Minute, und wie viele davon mit einem Fehler endeten. Du siehst auch, wie
viel die Box gesendet hat und wie lange die langsamsten Antworten gedauert
haben. Jede App steht mit ihrem Port in der Liste. Die Zähler beginnen von
vorn, wenn die Box neu startet.

Das Widget **Statistik** zeigt die letzte Stunde auf der Übersicht. Füge es
unter **Widget hinzufügen** hinzu. Ein Klick darauf öffnet die Seite.

## Deine Apps \{#your-apps}

Kleine eigene Apps, eine Einkaufsliste, ein Putzplan, eine Leseliste,
die du beschreibst und die KI auf der Box schreibt
([Mit KI bauen](../in-depth/widget-builder)). Sie stehen unter **Deine Apps**
auf der Seite Apps. **App erstellen** öffnet den Editor: Beschreibe die App
im Tab **Mit KI bauen** oder schreibe ihren Code im Tab **Code**. Die
Vorschau daneben ist die echte App, du kannst sie also vor dem Speichern
benutzen. Was du dort eingibst, vergisst sie; die gespeicherte App behält
ihre Daten auf der Box.

Jede gespeicherte App hat eine eigene Seite. Eine App ist eine
React-Komponente aus den LosOS-Komponenten, die die Box beim Speichern prüft
und übersetzt. Sie läuft nur in deinem Browser, in einem abgeschotteten
Rahmen ohne Zugriff auf die Box außer auf ihre eigenen Daten. **Ändern**
öffnet sie wieder im Editor, und **Löschen** entfernt sie von der Box mit
allem, was eingegeben wurde.

Jede App hat außerdem ein eigenes privates Repository in LosOS Git, nach ihr
benannt (`app-chores` für eine App namens Chores), und jedes Speichern ist
dort ein Commit. **Quelle** auf der Seite der App öffnet es. Klone es, ändere
`app.jsx` und pushe: Innerhalb einer Minute prüft die Box die Änderung wie
beim Speichern, und die App führt sie aus. Lässt sich die Änderung nicht
übersetzen, bleibt die App, wie sie war, ihre Seite sagt warum, und die Box
fügt einen Commit hinzu, der die App zurücksetzt, sodass dein nächstes
`git pull` das ebenfalls zeigt. Dateien, die du ins Repository legst,
bleiben dort, und die Daten der App sind nicht darin. Das Repository bleibt
in LosOS Git, wenn du die App löschst; lösche es dort, wenn du es nicht mehr
willst. Ist LosOS Git aus, behält die Box die Versionen selbst.

## Apps, die für die Box gemacht sind \{#apps-made-for-the-box}

Apps können auch als Compose-Datei im
[LosOS-App-Format](../in-depth/app-format) kommen, eine, die du schreibst,
oder eine, die ein Programm für dich schickt. Sie laufen als
`notshared`, stehen unter **Installierte Apps** mit der Zeile „für diese
Box gemacht“ und haben **Öffnen** und **Entfernen**, aber kein **Ändern**,
weil sie keine Eigenschaften haben.
