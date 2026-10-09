---
title: Tvorca widgetov
sidebar_position: 4.7
mdx:
  format: md
---

# Tvorca widgetov

Vlastník opíše widget bežnými slovami a Claude ho napíše. Editor widgetov
(**Pridať widget**, potom **Napísať**) má vedľa karty **Písať** kartu
**Vytvoriť s Claude**. Súbory widgetu sa objavia v editore, kde si ich
vlastník môže prečítať, vyskúšať v náhľade a upraviť. Nič sa neuloží, kým
vlastník nestlačí Uložiť, a widget od Claude beží v rovnakom sandboxovanom
rámci ako ručne písaný (pozri [Správa](administration.md#ručne-písané-widgety)).

Opis a súbory poslané na úpravu idú spoločnosti Anthropic a vzťahujú sa na ne
jej [Zásady používania](https://www.anthropic.com/legal/aup) a
[Obchodné podmienky](https://www.anthropic.com/legal/commercial-terms). Karta to uvádza pod tlačidlom Vytvoriť.

Tvorca je funkcia edge. Predvolene je vypnutý, na troch úrovniach.

|                          | Predvolene | Prepínač                                 |
| ------------------------ | ---------- | ---------------------------------------- |
| Edge ho ponúka           | vypnuté    | `losos.edge.builder.enable`              |
| Box ho smie používať     | vypnuté    | `losos.edge.tenants.<id>.market`         |
| Tvorba sa môže začať     | nie        | zostatok boxu pokryje jednu tvorbu       |

Box sa pýta len oficiálneho edge, rovnako ako pri trhu, lebo požiadavka nesie
proxy token boxu.

## Ako tvorba prebieha

Edge spúšťa [Claude Managed Agent](https://platform.claude.com/docs/en/managed-agents/overview):
agenta s pevným systémovým promptom a zmluvou widgetu, ktorého operátor
vytvorí raz, a na každú tvorbu cloudový kontajner, ktorý hostí Anthropic. Pre
každú tvorbu edge otvorí reláciu s opisom od vlastníka, jazykom stránky a pri
úprave aj s aktuálnymi súbormi widgetu. Agent zapíše `index.html`, súbory,
ktoré prepája (štýl, skript, dáta), a krátky `summary.txt`. Keď relácia
prejde do nečinnosti, edge ich stiahne, reláciu aj súbory zmaže a súbory
widgetu odovzdá boxu. Súbory s menom, aké widget mať nemôže, vynechá, a
widget nad 12 súborov alebo 128 KiB odmietne rovnako ako ručne písaný.

Agent má vo vlastnom kontajneri nástroje na súbory a shell a nemá prístup na
web. Jeho kontajner sa k boxu nedostane. Tvorba, ktorá beží dlhšie ako 15
minút, sa zastaví.

## Koľko to stojí

Vlastník platí cenníkovú cenu Anthropicu za milión vstupných a výstupných
tokenov modelu (Claude Opus 5.5, 4 $ vstup a 20 $ výstup) plus prirážku,
ktorú nastaví operátor (`markupBps`, predvolene 2000, teda 20 %). Čítanie a
zápis do cache sa účtujú podľa vlastných cenníkových cien s rovnakou
prirážkou. Čas kontajnera sa neprenáša. Edge prepočíta doláre na menu trhu
podľa `usdRate`.

Platí sa vopred. Box si dobije zostatok jedným z kreditových balíkov edge
(`packs`, predvolene 5, 10 a 20 v mene trhu) cez Stripe Checkout na
vlastnom účte Stripe edge, tom istom, ktorý používa trh. Zostatok sa zmení,
keď na edge dorazí podpísaný webhook `checkout.session.completed` od Stripe.

Každá tvorba beží s pevným rozpočtom relácie: toľko, koľko zostatok pokryje
v cenníkovej cene, najviac `maxBuildCents` (predvolene 3 $). Anthropic
agenta zastaví, keď rozpočet dosiahne, a vlastník uvidí, že widget môže byť
nedokončený. Účtuje sa to, čo relácia spotrebovala, zaokrúhlené nahor na
cent, aj keď agent skončí bez použiteľného widgetu. Tvorba, ktorej reláciu
už edge nevie prečítať, sa neúčtuje.

## Nastavenie pre operátora

1. V Claude Console vytvorte API kľúč vo vlastnom workspace s limitom
   výdavkov. Ten limit je posledné slovo o tom, koľko môže stáť chyba.
2. Zapečaťte kľúč pomocou `systemd-creds` zo stdin, aby sa otvorený text
   nikdy nedotkol disku:

   ```sh
   systemd-creds encrypt --name=claude-api-key - /var/secrets/losos-claude-api-key.cred
   ```

   Cesta je `losos.edge.builder.claudeKeySealed`. Názov musí byť presne
   `claude-api-key`.
3. Raz vytvorte agenta a jeho prostredie:

   ```sh
   systemd-run --pipe --wait \
     -p LoadCredentialEncrypted=claude-api-key:/var/secrets/losos-claude-api-key.cred \
     sh -c 'losos-registrar builder-setup --key-file "$CREDENTIALS_DIRECTORY/claude-api-key"'
   ```

   Vypíše dva riadky do konfigurácie edge: `losos.edge.builder.agentId` a
   `losos.edge.builder.environmentId`. Po aktualizácii ho spustite znova s
   `--agent-id` a `--environment-id`, aby sa agent aktualizoval na mieste.
4. Zapnite trh (`losos.edge.market.enable`, pozri
   [Trh](market.md#nastavenie-pre-operátora)), ktorého brána Stripe prijíma
   dobitia, a potom nastavte `losos.edge.builder.enable = true`. Jednotka
   registrátora sa bez zapečateného kľúča nespustí, preto ho zapečaťte ako
   prvý.
5. Nastavte `losos.edge.tenants.<id>.market = true` pre každý box, ktorý ho
   smie používať.

Začnite v **testovacom režime** Stripe. Testy edge bežia proti náhradám
Stripe a Anthropicu, ktoré overia, čo edge posiela a ako reaguje, no
nevedia povedať, či to niektorá zo služieb prijme.

## Voľby

| Voľba                                | Predvolene                               |
| ------------------------------------ | ---------------------------------------- |
| `losos.edge.builder.enable`          | `false`                                  |
| `losos.edge.builder.claudeKeySealed` | `/var/secrets/losos-claude-api-key.cred` |
| `losos.edge.builder.agentId`         | žiadna, povinná                          |
| `losos.edge.builder.environmentId`   | žiadna, povinná                          |
| `losos.edge.builder.markupBps`       | `2000` (20 %)                            |
| `losos.edge.builder.usdRate`         | `"1.0"`                                  |
| `losos.edge.builder.packs`           | `[ 500 1000 2000 ]`                      |
| `losos.edge.builder.maxBuildCents`   | `300`                                    |

## Bezpečnostné vlastnosti

- Kľúč Anthropicu drží len jednotka registrátora na edge, ako systemd
  credential čítaný pri každej požiadavke. Boxy ani prehliadače ho nevidia.
- Kľúč Stripe zostáva v bráne trhu. Brána prijme kreditový checkout len na
  sumu z balíkov operátora, v mene edge, bez cieľového účtu a bez poplatku.
- Zostatok sa zmení len po podpísanom webhooku, ktorý sa zhoduje so
  zaznamenanou objednávkou v id, relácii, sume aj mene, a každá objednávka sa
  pripíše raz.
- Box spúšťa naraz jednu tvorbu a edge najviac osem.
- Widget je text. Box ho vloží do editora a mimo sandboxovaného rámca
  widgetu ho nič nespustí.

## API

Box sa k tvorcovi dostane cez lososd, ktorý pridá jeho proxy token:

| Cesta na boxe                       | Cesta na edge           |
| ----------------------------------- | ----------------------- |
| `GET /api/builder`                  | `POST /builder/account` |
| `POST /api/builder/credits`         | `POST /builder/credits` |
| `POST /api/builder/builds`          | `POST /builder/builds`  |
| `GET /api/builder/builds/{id}`      | `POST /builder/build`   |

Tvary sú v `backend/schema.json`.
