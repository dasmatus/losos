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

Deine Repositorys, unter `http://<address>/forgejo/`. Das ist Forgejo in
denselben Farben. Registriere dich beim ersten Mal darin. Es führt eigene
Konten. Clone-URLs verwenden den Namen der Box; ein Rechner, der `.local`
nicht auflöst, sollte stattdessen die Adresse verwenden. Schalte es im
Bereich **Apps** ab, wenn die Box nicht für Code gedacht ist.

### Föderation \{#federation}

LosOS Git spricht ActivityPub mit anderen Forgejo-Servern. Forgejo nennt das
noch experimentell. Sterne, die Leute auf einem anderen Server deinen
Repositorys geben, zählen hier. Leute auf anderen Servern können einem Konto
auf deiner Box folgen und sehen, was es tut. Die Box antwortet außerdem auf
die Erkennungsadresse des Fediverse, `/.well-known/nodeinfo`. All das reicht
so weit wie die Box selbst. Über eine [Edge](../types/official-edge) ist das
das Internet. Bei einer Box, die nur in deinem lokalen Netz ist, sind es die
anderen Boxen in diesem Netz. Die Box veröffentlicht nie, wie viele Konten
sie hat oder wie aktiv sie sind.

Um mitzumachen, trägst du auf der Seite **Einstellungen → Föderation** eines
Repositorys die anderen Server ein, deren Sterne zählen. Um die Föderation
ganz abzuschalten, schalte **Mit anderen Forgejo-Servern föderieren** im
Abschnitt LosOS Git des Bereichs **Apps** aus.

:::note[Föderation nur mit geteilter Festplatte]
Die Föderation funktioniert nur, solange die Box ihre
Festplatte teilt (`losos.sharingMyStorage`), weil der geteilte Teil der
Festplatte nur dann entsperrt ist. Ist das Teilen aus, spricht die Box mit
keinem anderen Server, und der Bereich **Apps** zeigt den
Föderationsschalter ausgegraut.
:::

## Der Bereich Apps \{#the-apps-pane}

Für jede App zeigt der Bereich, ob sie an ist, in welchem
[Modus](../types/app-modes) sie läuft, und einen Link. Darunter haben LosOS
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

Für LosOS Git, nach seinem Föderationsschalter ([Föderation](#federation)):
ob nur angemeldete Konten überhaupt etwas sehen, was ein
Besucher zuerst sieht, ob ein neues Repository privat, öffentlich oder wie
das letzte des Kontos beginnt, ob die E-Mail-Adresse eines neuen Kontos
verborgen ist und ob ein Push in ein noch nicht vorhandenes Repository es
anlegt.

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
  30000 bis 30099, und nur dein lokales Netz erreicht ihn.

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

:::note[Als shared nur im Workload-Modus]
Eine `notshared`-App lässt sich auf jeder Box installieren. `shared`
braucht den eigenen Cluster der Box, der nur läuft, solange Code auf
**Abgeschottet** steht ([Workload-Modus](../types/app-modes)),
und die Box muss ihre Festplatte teilen. Um das Installieren abzuschalten,
setze `losos.apps.enable` im Bereich **Erweitert** auf false.
:::

## Apps, die für die Box gemacht sind \{#apps-made-for-the-box}

Apps können auch als Compose-Datei im
[LosOS-App-Format](../in-depth/app-format) kommen, eine, die du schreibst,
oder eine, die ein Programm für dich schickt. Sie laufen als
`notshared`, stehen unter **Installierte Apps** mit der Zeile „für diese
Box gemacht“ und haben **Öffnen** und **Entfernen**, aber kein **Ändern**,
weil sie keine Eigenschaften haben.
