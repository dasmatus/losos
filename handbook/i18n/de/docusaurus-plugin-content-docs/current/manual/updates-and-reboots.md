---
title: Updates und Neustarts
sidebar_position: 6
---

# Updates und Neustarts

Auf jeder Box laufen zwei Timer. Der Bereich Über zeigt beide.

![Eine Nacht auf einer Box, von 22:00 bis 06:00: Neustart um 00:07, Update um 03:00, Aufräumen um 04:30.](@site/docs/img/nightly-cycle-de.svg)

| Zeit      | Was                                                                                                                                      |
| --------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| **00:07** | **Neustart.** Ohne Bedingung. Die Systemplatte startet leer, und die Box baut sie aus ihrer Beschreibung neu auf, also repariert sich die Box mit diesem Neustart selbst. Eine Box, die um 00:07 aus war, startet neu, sobald sie an ist. |
| **03:00** | **Update.** Die Box holt die neueste Version von LosOS von GitHub, behält ihre eigenen Einstellungen und baut sich darauf neu auf. |
| **04:30** | **Aufräumen.** Die Box löscht Systemversionen, die älter als 14 Tage sind, und das Bootmenü behält die letzten fünf. So laufen die Festplatte und die Boot-Partition nicht voll. |

## Was erhalten bleibt \{#what-survives}

Nur, was auf einer kurzen Liste steht: deine Dateien und die Daten von
LosOS Git, der eigene Store des Systems, die von dir gewählten
Einstellungen, die Schlüssel und die Identität der Box, die Mitgliedschaft
im Mesh. Alles andere ist um 00:07 weg. Die
[Referenzseite](../reference/what-survives-a-reboot) hat die Liste.

![Der Bereich Über, der die Zeiten für Neustart und Update zeigt und wann jedes zuletzt lief.](@site/docs/img/settings-about-de.png)

## Während sie arbeitet \{#while-it-works}

Eine Box bleibt erreichbar, während sie ein Update oder eine Änderung
anwendet. Die Apps antworten weiter, bis die Box auf das neue System
wechselt, und das dauert Sekunden. Schlägt ein Update fehl, läuft das
vorherige System weiter, und die Box versucht es in der nächsten Nacht
erneut.

## Woher neue Versionen kommen \{#where-new-versions-come-from}

Die Box hält ihre Beschreibung in ihrem eigenen Repository, dem, das LosOS
Git im Bereich Historie zeigt. Vor dem Neuaufbau um 03:00 holt sie den
neuesten Commit des Zweigs `main` von `https://github.com/dasmatus/losos.git`
und macht ihn zur nächsten Version der Box, als einen Commit namens
`Update to dasmatus/losos main` mit der kurzen ID des Commits. Zwei Dateien bleiben,
wie die Box sie hat: `modules/overrides.nix` mit deinen Einstellungen und
`modules/install-target.nix` mit der Liste der Festplatten, dem
Firmware-Modus und dem Entsperrmodus. Der Neuaufbau baut dann diesen Commit,
und genauso jedes Anwenden danach.

:::note[Eigene Änderungen stoppen Updates]
Hast du eine andere Datei im Repository der Box geändert, zum Beispiel
indem du aus einem Klon einen Commit nach LosOS Git gepusht hast, behält die
Box deine Änderung und aktualisiert sich nicht. Der Bereich Historie sagt
das. Setze diese Dateien zurück, um dem Projekt wieder zu folgen.
:::

Drei Nix-Einstellungen steuern das, und der Bereich Erweitert zeigt sie:

| Option                                | Standard                                |
| ------------------------------------- | --------------------------------------- |
| `losos.configRepo.upstream.enable`    | an                                      |
| `losos.configRepo.upstream.url`       | `https://github.com/dasmatus/losos.git` |
| `losos.configRepo.upstream.branch`    | `main`                                  |

Eine Firma kann die Adresse auf ihren eigenen Fork zeigen lassen. Das
Repository muss ohne Passwort lesbar sein. Schaltest du die erste aus, baut
sich die Box nur noch aus der Version neu auf, die sie hat.

:::note[Kein Internet um 03:00]
Eine Box, die um 03:00 kein Internet hat, baut die Version neu, die sie hat,
und versucht es in der nächsten Nacht erneut.
:::

## Downloads bleiben auf der Box \{#downloads-stay-on-the-box}

Jeder Download eines Neuaufbaus geht über eine lokale Kopie auf der Box, die
auf der verschlüsselten Platte unter `/var/cache/losos-nix-cache` liegt. Die
Aufräumrunde um 04:30 löscht die Werkzeuge, die der letzte Neuaufbau geholt
hat, und der nächste findet sie in der Kopie, statt sie erneut
herunterzuladen. Die Kopie übersteht auch den Neustart um 00:07, sodass eine
Box ohne Internet um 03:00 aus dem neu bauen kann, was sie schon einmal
geholt hat.

Nichts in der Kopie gilt als vertrauenswürdig, nur weil es dort liegt. Jede
Software darin trägt die Signatur des Caches, aus dem sie stammt, und die Box
prüft diese Signatur bei jeder Installation.

| Option                          | Standard | Was                                                   |
| ------------------------------- | -------- | ----------------------------------------------------- |
| `losos.cache.local.enable`      | an       | Die Kopie behalten                                    |
| `losos.cache.local.maxSize`     | `8g`     | Höchstens so viel Platz; zuerst geht, was am längsten ungenutzt ist |
| `losos.cache.local.minFree`     | `4g`     | Freier Platz, den sie immer für deine Dateien lässt   |
| `losos.cache.local.shareOnLan`  | aus      | Andere Boxen in deinem Netz dürfen sie nutzen         |
| `losos.cache.local.peers`       | keine    | Kopien anderer Boxen, die zuerst gefragt werden       |

Mit mehreren Boxen zu Hause schaltest du `shareOnLan` auf einer ein und
trägst sie auf den anderen unter `peers` ein, als `http://<name>.local:8790`.
Dann laden sie jedes Update gemeinsam nur einmal aus dem Internet. Nur
Adressen aus deinem eigenen Netz kommen herein.

## Wenn die Box aus ist \{#when-the-box-is-off}

Nichts passiert, und nichts geht verloren. Schalte sie ein, und sie startet,
startet neu, falls sie eine Mitternacht verpasst hat, und aktualisiert sich
das nächste Mal um 03:00.
