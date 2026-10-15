---
title: Kein Platz mehr
sidebar_position: 10
slug: /troubleshooting/out-of-room
---

# Kein Platz mehr

<div className="losos-symptom">

**Was du siehst:** Die Übersicht sagt **Fast voll** oder **Kein Platz
mehr**; Uploads in LosOS cloud schlagen fehl; ein Anwenden oder das
nächtliche Update schlägt fehl.

</div>

![Die Übersicht auf einer Box, die fast voll ist.](@site/docs/img/overview-almost-full.png)

## Warum es mehr ausmacht als auf einem Laptop \{#why-it-matters-more-than-on-a-laptop}

Die eigene Software der Box liegt auf demselben Volume wie deine Dateien.
Jedes Update fügt eine Version hinzu, und das Aufräumen um 04:30 entfernt
Versionen, die älter als 14 Tage sind. Eine volle Festplatte blockiert
deshalb genau das, was die Box repariert, also handle, bevor **Kein Platz
mehr** erscheint.

## Was zu tun ist, der Reihe nach \{#what-to-do-in-order}

![Reserve nutzen, das Erste, was du versuchen solltest: Der Dialog sagt, wie viel Platz es bringt.](@site/docs/img/storage-use-reserve.png)

1. **Nimm die Reserve in Anspruch.** Bereich Speicher → **Reserve
   nutzen**. Der Installer hat dafür 10 % der Festplatte zurückgehalten.
   Das dauert eine Minute und braucht keinen Neustart. Siehe
   [Festplatte vergrößern](../manual/disk-growth).
2. **Leere den Papierkorb in LosOS cloud.** Gelöschte Dateien liegen 30
   Tage lang unter „Gelöschte Dateien“ und belegen weiter Platz.
   Vorschaubilder der Fotos-App und alte Dateiversionen, unter „Versionen“
   in der Seitenleiste einer Datei, summieren sich ebenfalls.
3. **Warte eine Nacht.** Das Aufräumen um 04:30 löscht alte
   Systemversionen. Wenn die Box in den letzten zwei Wochen installiert
   oder oft aktualisiert wurde, können das mehrere Gigabyte sein.
4. **Lagere Daten aus**, mit dem Desktop-Client oder per Download aus LosOS
   cloud, und lösch sie dann dort.
5. **Teile weniger.** Eine Box, die dem Mesh Platz leiht, mit dem
   Speichermodus auf geteilt, hält Kopien für andere Boxen. Schaltest du das
   Teilen aus, wird dieser Platz wieder frei, sobald der Pool die Daten neu
   repliziert hat.
6. **Füg eine Festplatte hinzu** oder installiere auf eine größere neu.
   Siehe [Festplatte vergrößern](../manual/disk-growth).

## Der Bereich Speicher meldet keine Größe \{#the-storage-pane-does-not-report-a-size}

„Die Box hat nicht gemeldet, wie voll ihre Festplatte ist.“ ist direkt nach
einem Neustart eine Minute lang normal. Bleibt es dabei, antwortet der
Steuerdienst nicht. `http://<address>/api/health` sollte `{"ok":true}`
sagen. Wenn nicht, behebt das der nächtliche Neustart.
