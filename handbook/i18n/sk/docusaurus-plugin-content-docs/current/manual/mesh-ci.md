---
title: Mesh CI
sidebar_position: 4.65
slug: /manual/mesh-ci
---

# Mesh CI

![Mesh CI: runner tohto boxu a kedy berie úlohy iných boxov, potom behy, jeden beží, jeden zrušený a dva prešli, a hore Spustiť workflow.](@site/docs/img/mesh-ci-sk.png)

Mesh CI spúšťa workflowy GitHub Actions vo vašich repozitároch v
[LosOS Git](losos-git). Push alebo pull request spustí workflowy z
priečinka `.github/workflows/` v danom commite, boxy vašej siete si úlohy
rozdelia a výsledok každej úlohy sa ukáže pri commite a v kontrolách pull
requestu. **Mesh CI** v bočnom paneli ho otvorí na `/ci`.

## Kedy je Mesh CI k dispozícii \{#when-mesh-ci-is-there}

Mesh CI existuje len vtedy, keď box zdieľa svoje úložisko so sieťou, do
ktorej sa pripojil (pozrite [režimy úložiska](../types/storage-modes)). So
zdieľaním vypnutým alebo mimo siete nie je v bočnom paneli **Mesh CI** a
žiadny workflow sa nespustí. Behy, ktoré už prebehli, zostávajú na disku a
vrátia sa s ním.

## Behy \{#runs}

Stránka ukazuje najnovšie behy ako prvé: workflow, jeho číslo, čo ho
spustilo (push, pull request alebo **Spustiť workflow**), repozitár, commit a
ako dlho trval. Otvorte beh a uvidíte jeho úlohy. Pri každej úlohe je
napísané, kde bežala: **Tento box**, alebo **Sieť** s menom boxu, ktorý ju
prevzal. Vyberte úlohu a prečítate si jej log krok po kroku, s návratovým
kódom každého kroku a výstupmi úlohy.

**Zrušiť beh** zastaví úlohy behu a ich kontajnery sa odstránia.
**Spustiť znova** spustí tie isté workflowy na tom istom commite.

**Spustiť workflow** spustí workflow, ktorý má medzi spúšťačmi
`workflow_dispatch`. Zadajte repozitár ako `vlastník/názov`, vetvu
(predvolenú, ak pole necháte prázdne), stlačte **Nájsť workflowy** a jeden
vyberte.

![Beh, ktorý prešiel: jeho dve úlohy, každá s boxom mesh siete, na ktorom bežala, a kroky jednej s výstupom a trvaním.](@site/docs/img/mesh-ci-run-sk.png)

## Kde úloha beží \{#where-a-job-runs}

Úloha ide do siete, kde ju môže prevziať ktorýkoľvek box, ktorý požičiava
svoj runner, okrem prípadov, keď:

- jej `runs-on` uvádza `self-hosted`;
- jej repozitár je súkromný, pokiaľ nie je zapnuté `losos.ci.privateOnMesh`.
  Iný box by videl kód;
- box nemá na dosah žiadny edge.

Vtedy beží na tomto boxe.

## Požičiavanie runnera tohto boxu \{#lending-this-boxs-runner}

**Spúšťať úlohy iných boxov** v časti **Runner tohto boxu** určuje, kedy
tento box preberá úlohy od zvyšku siete:

- **Vždy**;
- **V okne výpočtov**, v hodinách nastavených na stránke Sieť (predvolené);
- **Nikdy**. Box naďalej spúšťa vlastné úlohy.

Box berie naraz jednu úlohu. Úloha beží pod účtom `shared` v kontajneri
bez roota, bez prístupu do vlastnej siete boxu či k čomukoľvek inému v LAN.
Dostane najviac `losos.ci.runner.memory` pamäte (predvolene 2 GB) a
`losos.ci.runner.cpus` procesorového času (1,5 jadra), s nízkou prioritou,
takže vlastná práca boxu má prednosť.

## Čo môže workflow používať \{#what-a-workflow-can-use}

Workflowy sa čítajú ako workflowy GitHub Actions, takže väčšina z nich
beží bez úprav:

- `on: push`, `pull_request` a `workflow_dispatch`, s filtrami vetiev a
  značiek;
- úlohy s `needs`, `if`, `strategy.matrix`, `outputs`, `env`, `defaults`
  a `timeout-minutes` a výrazy `${{ }}`;
- kroky s `run` a akcie z `uses`: akcie v JavaScripte, zložené a Docker
  akcie z GitHubu, akcie v repozitári (`./cesta`) a obrazy `docker://`;
- `container` a `services`. Služba je dostupná pod svojím menom aj na
  `localhost` a jej kontrola zdravia prejde skôr, než začnú kroky.

Úloha bez `container` beží vo vlastnom obraze LosOS: bash, bežné nástroje
príkazového riadka, git a curl. `losos.ci.runner.images` priraďuje štítkom
`runs-on` iné obrazy, napríklad
`ubuntu-latest=docker.io/library/ubuntu:24.04`.

Niektoré časti GitHub Actions potrebujú vlastné služby GitHubu a log to
povie tam, kde sa objavia:

- `actions/checkout` nerobí nič: súbory commitu už sú v pracovnom
  priečinku;
- `actions/cache`, `upload-artifact` a `download-artifact` nič neuchovajú;
- `secrets` je prázdne, `schedule` sa nikdy nespustí a znovupoužiteľné
  workflowy (`jobs.<id>.uses`) sa odmietnu.

## Kontajnery a pm \{#containers-and-pm}

Box spúšťa a zastavuje kontajnery každej úlohy správcom balíkov
[pm](https://github.com/losos-project/pm), cez vlastný plugin Mesh CI pre
pm. Plugin je podpísaný a pm nenačíta žiadny plugin, ktorý podpísaný nie
je. Odpovedá len za kontajnery Mesh CI, ktorých mená začínajú na `lci-`, a
odmietne čokoľvek spustiť ako root.

Krok môže tiež zostaviť recept pm akciou `losos-project/pm`:

```yaml
- uses: losos-project/pm@v0
  id: pkg
  with:
    recipe: pkg/build.rhai
    trust: 3f0c…   # verejný kľúč v hex, ktorým je recept podpísaný
- run: ls -l ${{ steps.pkg.outputs.archive }}
```

pm zostaví recept vo vlastnom väzení, v pracovnom priečinku úlohy, a výstup `archive` je
balík, ktorý vyrobil. Recept musí byť podpísaný kľúčom z `trust`.
`command: explain` povie, čo by recept urobil, bez zostavenia, a
`permissive: true` odovzdá pm prepínač `--permissive`.

## Nastavenia \{#settings}

Na paneli **Rozšírené**:

- `losos.ci.enable` vypne Mesh CI;
- `losos.ci.privateOnMesh` dovolí úlohám súkromných repozitárov bežať na
  iných boxoch;
- `losos.ci.runner.memory`, `losos.ci.runner.cpus` a
  `losos.ci.runner.images` sú opísané vyššie.
