---
title: Nastavenia a voľby
sidebar_position: 3
---

# Nastavenia a voľby

Každé pole na paneloch nastavení zapisuje jednu voľbu v Nix opise
zariadenia (`losos.*`). Túto tabuľku použite, keď chcete niečo, čo panely
neponúkajú. Správca tieto voľby nastavuje v `modules/overrides.nix`
a [Správa](/in-depth/administration.md#settings) ich opisuje úplne.

## Na paneloch \{#on-the-panes}

| Panel        | Pole                        | Voľba                                               | Predvolene              |
| ------------ | --------------------------- | --------------------------------------------------- | ----------------------- |
| Sieť         | Názov                       | `losos.hostName`                                    | `losos`                 |
| Sieť         | Šifrovať pripojenie         | `losos.tls.enable`                                  | zapnuté                 |
| Sieť         | Dostupné aj mimo domova     | `losos.proxy.enable`                                | vypnuté                 |
| Sieť         | Adresa edge                 | `losos.proxy.registrarUrl`                          | edge LosOS              |
| Aplikácie    | LosOS Git                   | `losos.forgejo.enable`                              | zapnuté                 |
| Aplikácie    | režim (iba na čítanie)      | `losos.nextcloud.mode`, `losos.forgejo.mode`        | `container`             |
| Aplikácie    | LosOS cloud, nastavenia pre všetkých | `losos.nextcloud.site.*` | ako ho dodáva LosOS cloud |
| Aplikácie    | LosOS Git, nastavenia pre všetkých | `losos.forgejo.site.*` | ako ho dodáva LosOS Git |
| Úložisko     | Použiť rezervu…             | spustí `losos-ctl grow`, rezerva je `losos.storage.fillPercent` | 90 %  |
| Mesh         | Pripojiť sa k sieti mesh    | `losos.cluster.enable`                              | vypnuté                 |
| Mesh         | Požičiavať toto zariadenie, kým spím | `losos.cluster.shareCompute`               | vypnuté                 |
| Mesh         | Hodiny                      | `losos.cluster.computeWindow.{start,end}`           | nenastavené             |
| Trh          | Zdieľať disk tohto zariadenia so sieťou mesh | `losos.sharingMyStorage`           | vypnuté                 |
| Hardvér      | Grafika                     | `losos.gpu.enable`                                  | vypnuté                 |
| Zabezpečenie | Obmedziť programy, ktoré komunikujú so sieťou (AppArmor) | `losos.hardening.apparmor` | vypnuté         |
| Zabezpečenie | Prísnejšia správa pamäte    | `losos.hardening.malloc`                            | vypnuté                 |
| Zabezpečenie | Polovičný procesor na uzavretie úniku medzi úlohami (bez SMT) | `losos.hardening.nosmt` | vypnuté         |
| Zabezpečenie | Ignorovať neskôr pripojené USB zariadenia (USBGuard) | `losos.hardening.usbguard` | vypnuté            |

## Mimo panelov \{#not-on-the-panes}

| Čo                                   | Voľba                              | Predvolene                                 |
| ------------------------------------ | ---------------------------------- | ------------------------------------------ |
| Odkiaľ prichádzajú aktualizácie      | `losos.configRepo.upstream.url`, `.branch` | `https://github.com/dasmatus/losos.git`, `main` |
| Či sa zariadenie vôbec aktualizuje   | `losos.configRepo.upstream.enable` | zapnuté                                    |
| Čo zostavuje nočná prestavba         | `losos.upgradeFlakeUri`            | `git+file:///etc/nixos#install`, vlastný repozitár zariadenia |
| Základný hardening                   | `losos.hardening.enable`           | zapnuté                                    |
| Federácia LosOS Git (ActivityPub)    | `losos.forgejo.federation.enable`  | zapnutá, kým je disk zdieľaný              |
| Federácia LosOS cloud                | `losos.nextcloud.federation.enable` | zapnutá, kým je disk zdieľaný             |
| Režim odomykania disku               | `losos.tpm.enable`                 | podľa toho, čo našiel inštalátor           |
| UEFI alebo BIOS                      | `losos.bios`                       | podľa toho, čo našiel inštalátor           |
| Disky                                | `losos.targetDrives`               | podľa toho, čo našiel inštalátor           |
| Platnosť certifikátu                 | `losos.tls.validityDays`           | 730                                        |
| Prah nečinnosti pre požičiavanie CPU | `losos.cluster.idleLoadThreshold`  | 0,25 na jadro                              |
| Binárna cache                        | `losos.cache.substituters`         | `https://proxy.losos.dasmat.us`, potom `https://losos-cache-proxy.vercel.app`, potom `https://losos.dasmat.us/proxy` |
| Kto môže z webovej stránky čítať `/setup/state.json` | `losos.setup.finderOrigins` | stránka find na edge LosOS     |

![Panel Rozšírené, kde sa upravujú voľby, ktoré nie sú na iných paneloch, každá skontrolovaná pred stlačením Použiť.](@site/docs/img/settings-advanced-sk.png)

## Pravidlá, ktoré panely vynucujú \{#rules-the-panes-enforce}

- Názov tvoria písmená, číslice a pomlčky, na začiatku aj na konci je
  písmeno alebo číslica, najviac 63 znakov.
- Port je medzi 1024 a 65535.
- Hodiny sú dva časy v 24-hodinovom formáte, `HH:MM`, v časovom pásme
  zariadenia. Okno môže prechádzať cez polnoc.
