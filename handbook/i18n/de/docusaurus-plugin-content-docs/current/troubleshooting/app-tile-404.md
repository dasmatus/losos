---
title: Eine App-Kachel liefert 404
sidebar_position: 8
slug: /troubleshooting/app-tile-404
---

# Eine App-Kachel liefert 404

<div className="losos-symptom">

**Was du siehst:** Ein Klick auf **Dateien**, **Fotos** oder **Code** in der
Übersicht, oder ein altes Lesezeichen, öffnet eine Seite „Not Found“.

</div>

![Der Bereich Apps mit LosOS Cloud und LosOS Git; die App-Kacheln der Übersicht öffnen dieselben Apps.](@site/docs/img/apps-de.png)

## „Not Found“ von einer App-Kachel \{#not-found-from-an-app-tile}

- **Die App ist aus.** **Dateien** und **Fotos** öffnen LosOS Cloud, das
  `losos.cloud.enable` im Bereich **Erweitert** ein- und ausschaltet.
  **Code** öffnet LosOS Git, das der Bereich **Apps** ein- und ausschaltet.
- **Die Box ist gerade gestartet.** LosOS Cloud öffnet ein paar Sekunden,
  nachdem die Box läuft. Beim ersten Start nach einem Update von Nextcloud
  zieht es zuerst deine Dateien um, was länger dauert
  ([Eine Box, auf der Nextcloud lief](../manual/losos-cloud#a-box-that-ran-nextcloud)).
  Warte und lade die Seite neu.

## „Not Found“ von einem alten Lesezeichen \{#not-found-from-an-old-bookmark}

Lesezeichen auf die eigenen Seiten von Nextcloud, etwa
`/nextcloud/index.php/apps/calendar/`, führen nirgends mehr hin: Die Box
hat kein Nextcloud. Öffne stattdessen `https://<name>.local/cloud/`. Die
Apps für Computer und Telefon betrifft das nicht, LosOS Cloud antwortet
ihnen weiter unter der Adresse `/nextcloud`, mit der sie eingerichtet sind.
