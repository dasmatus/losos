---
title: Die Apps
sidebar_position: 4
slug: /manual/apps
---

# Die Apps

## LosOS cloud \{#losos-cloud}

Deine Dateien, Fotos, Kalender, Kontakte, Notizen, Aufgaben, Musik und die
übrigen unten genannten Apps, unter `http://<address>/nextcloud`. Das ist
Nextcloud in den Farben und mit dem Namen der Box. Melde dich mit dem
Passwort des Besitzers an. LosOS cloud kann alles, was Nextcloud kann. Der
Desktop-Client synchronisiert einen Ordner auf deinem Rechner, die
Handy-Apps laden Fotos hoch, und Kalender und Kontakte synchronisieren über
CalDAV und CardDAV.

![Die Zeichen von LosOS cloud und LosOS Git im hellen und im dunklen Design, um einen lebenden Lachs gezeichnet.](@site/docs/img/cloud-and-git-marks.png)

Die Übersicht hat eine Kachel für jede App im eigenen Menü von LosOS cloud,
in derselben Reihenfolge: Dateien, Dashboard, Fotos, Aktivität, Kontakte,
Kalender, Notizen, Lesezeichen, Deck, Musik, Kollektive, Umfragen,
Formulare, Tabellen, Memories, News, Aufgaben und Karten. Die Box schaltet
sie bei jedem Start alle ein. E-Mail ist nicht dabei, weil die Box keinen
Mailserver betreibt, den die App lesen könnte. Installierst du E-Mail
trotzdem aus dem App Store, erscheint ihre Kachel auf der Übersicht, wenn die
Seite das nächste Mal lädt. Die Links gehen absichtlich über `index.php`,
weil jede Konfiguration auf diesen Pfad antwortet.

Konten für Familie oder Kollegen legst du in LosOS cloud unter Benutzer an,
nicht auf den Admin-Seiten. Nur das Konto des Besitzers entsperrt die
Admin-Seiten.

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
ganz abzuschalten, verwende `losos.forgejo.federation.enable` im Bereich
**Erweitert**.

LosOS cloud föderiert ebenfalls: Du kannst Dateien und Kalender mit Leuten
teilen, deren Konten auf anderen Nextcloud-Servern liegen, über ihre Adresse
`user@server`. `losos.nextcloud.federation.enable` schaltet das ab.

Beide Arten der Föderation funktionieren nur, solange die Box ihre
Festplatte teilt (`losos.sharingMyStorage`), weil der geteilte Teil der
Festplatte nur dann entsperrt ist. Ist das Teilen aus, spricht die Box mit
keinem anderen Server, und der Bereich **Erweitert** zeigt beide
Föderationsschalter als nicht verfügbar.

## Der Bereich Apps \{#the-apps-pane}

Für jede App zeigt der Bereich, ob sie an ist, in welchem
[Modus](../types/app-modes) sie läuft, und einen Link. Darunter durchsucht
**Mehr finden** [Artifact Hub](https://artifacthub.io) nach Helm-Charts. Die
Box holt die Ergebnisse selbst, also braucht die Suche das Internet. Jede
Zeile nennt, wer das Chart veröffentlicht hat. Niemand auf der Seite von
LosOS hat eines davon geprüft.

![Der Bereich Apps mit einer installierten App über Mehr finden und Suchergebnissen aus Artifact Hub, jedes mit dem Herausgeber und einer Schaltfläche Installieren oder Ändern.](@site/docs/img/apps-find-more-de.png)

## Einstellungen für alle \{#settings-for-everyone}

Unter den beiden Apps hat der Bereich Apps die Einstellungen, die für jedes
Konto in LosOS cloud und LosOS Git gelten. Sonst fändest du sie auf den
eigenen Verwaltungsseiten jeder App.

Für LosOS cloud: die Sprache der Anmeldeseite und der Konten, die keine
gewählt haben, das Land, zu dem eine Telefonnummer ohne Ländervorwahl
gehört, ob Dateien per öffentlichem Link geteilt werden dürfen, ob jeder
Link ein Passwort braucht, nach wie vielen Tagen Links nicht mehr
funktionieren, wie viel Platz ein neues Konto bekommt und wie lange der
Papierkorb und alte Versionen aufbewahrt werden. Ein leeres Feld überlässt
die Wahl LosOS cloud.

Für LosOS Git: ob nur angemeldete Konten überhaupt etwas sehen, was ein
Besucher zuerst sieht, ob ein neues Repository privat, öffentlich oder wie
das letzte des Kontos beginnt, ob die E-Mail-Adresse eines neuen Kontos
verborgen ist und ob ein Push in ein noch nicht vorhandenes Repository es
anlegt.

![Der Bereich Apps mit den Einstellungen für alle in LosOS cloud und LosOS Git.](@site/docs/img/apps-site-settings-de.png)

Drücke **Anwenden**, um sie zu übernehmen, wie bei jeder anderen
Einstellung. Die Box setzt sie bei jedem Start der App erneut, sodass die
eigenen Verwaltungsseiten der Apps diese Werte zeigen, sie aber nicht ändern
können. Die eigenen Einstellungen jedes Kontos, die Kontenliste und alles,
was nicht im Bereich steht, bleiben in der App.

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

Die Box installiert das Chart dann in ihren eigenen Cluster und ändert es
so, dass es so läuft wie LosOS cloud und LosOS Git.

- Jeder Teil der App läuft als der gewählte Benutzer, nie als root, ohne
  zusätzliche Rechte und ohne Zugriff auf den Cluster.
- Der Speicher, den das Chart anfordert, wird zu Ordnern unter
  `/home/<user>/data/apps/<name>`.
- Die App bekommt einen eigenen Port aus `losos.apps.ports`, standardmäßig
  30000 bis 30099, und nur dein lokales Netz erreicht ihn.

Ein Chart, das mehr braucht, lehnt die Box ab und sagt, warum. Typische
Gründe sind Admin-Rechte, ein Port unter 1024, ein Port, den schon eine
andere App benutzt, oder ein Ordner des Systems. Eine App aus mehreren
Diensten, die einander über ihren Namen aufrufen, startet vielleicht nicht,
weil der eigene Cluster der Box kein internes Netz hat.

**Installierte Apps** über der Suche listet jede App mit ihrem Zustand.
**Öffnen** geht zum Port der App auf der Adresse, die du gerade benutzt.
**Ändern** öffnet denselben Dialog mit den aktuellen Eigenschaften der App.
**Entfernen** fragt zuerst nach und nimmt die App dann von der Box. Ihr
Datenordner bleibt, sodass eine Installation unter demselben Namen die Daten
wiederfindet.

Die Suche bietet Installationen nur an, solange Dateien oder Code auf
**Abgeschottet** steht, also im [Workload-Modus](../types/app-modes). Nur
dann läuft der eigene Cluster der Box. Um das Installieren abzuschalten,
setze `losos.apps.enable` im Bereich **Erweitert** auf false.

## Vertrauenswürdige Adressen \{#trusted-addresses}

LosOS cloud antwortet auf den Namen der Box und auf die Adresse, an der die
Anfrage angekommen ist. Die Box gibt diese Adresse mit jeder Anfrage weiter.
LosOS cloud antwortet nie auf einen Platzhalter. Deshalb zeigt eine Box, die
du über ihre IP-Adresse erreichst, keine Seite „nicht vertrauenswürdige
Domain“. Siehst du doch einmal eine, erklärt [Nicht vertrauenswürdige
Domain](../troubleshooting/app-tile-404#access-through-untrusted-domain),
warum.
