---
title: Mesh CI
sidebar_position: 4.65
slug: /manual/mesh-ci
---

# Mesh CI

![Mesh CI: der Runner dieser Box und wann er Jobs anderer Boxen nimmt, dann die Läufe, einer läuft, einer abgebrochen und zwei bestanden, oben Workflow ausführen.](@site/docs/img/mesh-ci-de.png)

Mesh CI führt die GitHub-Actions-Workflows in deinen Repositorys in
[LosOS Git](losos-git) aus. Ein Push oder ein Pull-Request startet die
Workflows im Ordner `.github/workflows/` des Commits, die Boxen deines
Mesh teilen sich die Jobs, und das Ergebnis jedes Jobs steht am Commit und
in den Checks des Pull-Requests. **Mesh CI** in der Seitenleiste öffnet es
unter `/ci`.

## Wann Mesh CI da ist \{#when-mesh-ci-is-there}

Mesh CI gibt es nur, solange die Box ihren Speicher mit einem Mesh teilt,
dem sie beigetreten ist (siehe [Speichermodi](../types/storage-modes)).
Ist das Teilen aus oder die Box in keinem Mesh, gibt es kein **Mesh CI** in
der Seitenleiste, und kein Workflow läuft. Läufe, die es schon gab,
bleiben auf der Platte und kommen mit ihr zurück.

## Läufe \{#runs}

Die Seite zeigt die neuesten Läufe zuerst: den Workflow, seine Nummer, was
ihn gestartet hat (ein Push, ein Pull-Request oder **Workflow ausführen**),
das Repository, den Commit und wie lange er gedauert hat. Öffne einen Lauf,
um seine Jobs zu sehen. Bei jedem Job steht, wo er lief: **Diese Box**
oder **Mesh** mit dem Namen der Box, die ihn übernommen hat. Wähle einen
Job, um sein Log Schritt für Schritt zu lesen, mit dem Exit-Code jedes
Schritts und den Ausgaben des Jobs.

**Lauf abbrechen** stoppt die Jobs eines Laufs, und ihre Container werden
entfernt. **Erneut ausführen** startet dieselben Workflows auf demselben
Commit.

**Workflow ausführen** startet einen Workflow, der `workflow_dispatch`
unter seinen Auslösern hat. Gib das Repository als `besitzer/name` an,
einen Branch (den Standard-Branch, wenn du das Feld leer lässt), drücke
**Workflows suchen** und wähle einen aus.

![Ein bestandener Lauf: seine zwei Jobs, jeder mit der Box im Mesh, auf der er lief, und die Schritte eines Jobs mit Ausgabe und Dauer.](@site/docs/img/mesh-ci-run-de.png)

## Wo ein Job läuft \{#where-a-job-runs}

Ein Job geht ins Mesh, wo jede Box, die ihren Runner verleiht, ihn
übernehmen kann, außer wenn:

- sein `runs-on` `self-hosted` nennt;
- sein Repository privat ist, außer `losos.ci.privateOnMesh` ist an. Eine
  andere Box würde den Code sehen;
- die Box keine Edge in Reichweite hat.

Dann läuft er auf dieser Box.

## Den Runner dieser Box verleihen \{#lending-this-boxs-runner}

**Jobs anderer Boxen ausführen** unter **Runner dieser Box** legt fest,
wann diese Box Jobs vom Rest des Mesh übernimmt:

- **Immer**;
- **Im Rechenfenster**, zu den Stunden, die auf der Seite Mesh eingestellt
  sind (Standard);
- **Nie**. Die Box führt ihre eigenen Jobs weiter aus.

Die Box nimmt einen Job nach dem anderen. Ein Job läuft unter dem Konto
`shared` in einem Container ohne Root, ohne Zugang zum eigenen Netz der Box
oder zu irgendetwas anderem im LAN. Er bekommt höchstens
`losos.ci.runner.memory` Speicher (standardmäßig 2 GB) und
`losos.ci.runner.cpus` Rechenzeit (1,5 Kerne), mit niedriger Priorität,
damit die eigene Arbeit der Box vorgeht.

## Was ein Workflow nutzen kann \{#what-a-workflow-can-use}

Workflows werden als GitHub-Actions-Workflows gelesen, also laufen die
meisten unverändert:

- `on: push`, `pull_request` und `workflow_dispatch`, mit Branch- und
  Tag-Filtern;
- Jobs mit `needs`, `if`, `strategy.matrix`, `outputs`, `env`, `defaults`
  und `timeout-minutes` sowie die `${{ }}`-Ausdrücke;
- Schritte mit `run` und Actions aus `uses`: JavaScript-, Composite- und
  Docker-Actions von GitHub, Actions im Repository (`./pfad`) und
  `docker://`-Images;
- `container` und `services`. Ein Service ist unter seinem Namen und auf
  `localhost` erreichbar, und sein Health-Check besteht, bevor die
  Schritte beginnen.

Ein Job ohne `container` läuft im eigenen Image von LosOS: bash, die
üblichen Kommandozeilen-Werkzeuge, git und curl. `losos.ci.runner.images`
ordnet `runs-on`-Labels andere Images zu, etwa
`ubuntu-latest=docker.io/library/ubuntu:24.04`.

Manche Teile von GitHub Actions brauchen GitHubs eigene Dienste, und das
Log sagt es, wo sie vorkommen:

- `actions/checkout` tut nichts: die Dateien des Commits liegen schon im
  Arbeitsordner;
- `actions/cache`, `upload-artifact` und `download-artifact` behalten
  nichts;
- `secrets` ist leer, `schedule` löst nie aus, und wiederverwendbare
  Workflows (`jobs.<id>.uses`) werden abgelehnt.

## Container und pm \{#containers-and-pm}

Die Box startet und stoppt die Container jedes Jobs mit dem Paketmanager
[pm](https://github.com/losos-project/pm), über das eigene pm-Plugin von
Mesh CI. Das Plugin ist signiert, und pm lädt kein Plugin, das es nicht
ist. Es ist nur für die Container von Mesh CI zuständig, deren Namen mit
`lci-` beginnen, und führt nichts als root aus.

Ein Schritt kann auch ein pm-Rezept mit der Action `losos-project/pm`
bauen:

```yaml
- uses: losos-project/pm@v0
  id: pkg
  with:
    recipe: pkg/build.rhai
    trust: 3f0c…   # der öffentliche Schlüssel in Hex, mit dem das Rezept signiert ist
- run: ls -l ${{ steps.pkg.outputs.archive }}
```

pm baut das Rezept in seinem eigenen Gefängnis, im Arbeitsordner des Jobs, und
die Ausgabe `archive` ist das Paket, das es gebaut hat. Das Rezept muss mit
einem Schlüssel aus `trust` signiert sein. `command: explain` sagt, was das
Rezept tun würde, ohne es zu bauen, und `permissive: true` gibt pm
`--permissive` mit.

## Einstellungen \{#settings}

Im Bereich **Erweitert**:

- `losos.ci.enable` schaltet Mesh CI aus;
- `losos.ci.privateOnMesh` lässt die Jobs privater Repositorys auf
  anderen Boxen laufen;
- `losos.ci.runner.memory`, `losos.ci.runner.cpus` und
  `losos.ci.runner.images` sind oben beschrieben.
