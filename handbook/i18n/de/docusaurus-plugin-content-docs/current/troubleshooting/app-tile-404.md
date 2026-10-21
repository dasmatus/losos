---
title: Eine App-Kachel liefert 404 oder „nicht vertrauenswürdige Domain“
sidebar_position: 8
slug: /troubleshooting/app-tile-404
---

# Eine App-Kachel liefert 404 oder „nicht vertrauenswürdige Domain“

<div className="losos-symptom">

**Was du siehst:** Ein Klick auf **Aufgaben**, **Kalender** oder eine andere
Kachel der Übersicht öffnet eine schlichte Seite „Not Found“, oder LosOS
cloud zeigt „Zugriff über eine nicht vertrauenswürdige Domain“.

</div>

![Der Bereich Apps mit LosOS cloud und LosOS Git; die App-Kacheln der Übersicht öffnen dieselben Apps.](@site/docs/img/apps-de.png)

## „Not Found“ von einer App-Kachel \{#not-found-from-an-app-tile}

Es gibt zwei Ursachen. Die Pull Requests #65 und #66 haben beide am
6. Oktober 2026 behoben, und das nächtliche Update bringt die Korrekturen
mit:

- Die Kacheln verlinkten auf die kurze Form `/nextcloud/apps/<app>/`, auf
  die der Webserver von LosOS cloud im standardmäßigen
  [App-Modus](../types/app-modes) nicht antwortete. Die Kacheln gehen jetzt
  über `/nextcloud/index.php/apps/<app>/`, worauf jede Konfiguration
  antwortet.
- Die App selbst ist in LosOS cloud nicht installiert oder deaktiviert.
  Öffne LosOS cloud → Apps und aktiviere sie.

:::tip[Bis das Update da ist]
Bis das Update da ist, öffnest du LosOS cloud über die Kachel **Dateien**
oder unter `/nextcloud` und nimmst das eigene App-Menü von LosOS cloud.
Dessen Links funktionieren.
:::

## „Zugriff über eine nicht vertrauenswürdige Domain“ \{#access-through-untrusted-domain}

LosOS cloud antwortet nur auf Namen, denen es vertraut. Bei jeder Anfrage
teilt die Box ihm mit, über welche Adresse die Anfrage hereinkam, deshalb
funktionieren sowohl `<name>.local` als auch die IP-Adresse. Einen
Platzhalter gibt die Box ihm nie. Du siehst diese Seite, wenn:

- du die Box unter einem Namen erreicht hast, der nicht ihr eigener ist:
  ein Alias deines Routers, ein Eintrag in der hosts-Datei unter einem
  anderen Namen oder eine Domain über eine Edge, von der die Box nichts
  weiß. Nimm `<name>.local` oder die Adresse.
- du den Namen der Box gerade eben geändert hast und LosOS cloud noch nicht
  neu gestartet ist. Warte, bis der Rebuild fertig ist.
- du eine Box von vor dem 6. Oktober 2026 über die IP-Adresse erreicht
  hast. Pull Request #65 hat das Vertrauen pro Anfrage eingeführt. Nimm
  bis zum Update `<name>.local`.
