---
title: Formát aplikácií
sidebar_position: 4.8
mdx:
  format: md
---

# Formát aplikácií

Aplikácia pre zariadenie LosOS je jeden súbor Compose a súbory vedľa neho.
Zariadenie ju spúšťa ako `notshared` na Podmane bez roota, cez Podman
Compose, a nepotrebuje na to žiadny klaster. Ten istý formát slúži dvom
zdrojom:

- aplikáciám, ktoré zariadeniu pošle vlastník alebo program, ktorý pre neho
  pracuje,
- vlastným aplikáciám zariadenia, deklarovaným v Nixe cez
  `losos.apps.builtin`.

Chart z vyhľadávania na paneli **Aplikácie**, nainštalovaný ako
`notshared`, skončí na tom istom mieste: zariadenie ho vykreslí cez
`helm template` a z jeho podov urobí projekt Compose tohto tvaru.

## Súbor

```yaml
x-losos:
  port: 8080            # port, na ktorom počúva stránka aplikácie
services:
  web:
    image: docker.io/library/nginx:1.27-alpine
    command: ["nginx", "-g", "daemon off;", "-c", "/site/nginx.conf"]
    volumes:
      - ./site:/site:ro       # priečinok, ktorý aplikácia prináša
      - data:/var/lib/board   # dáta aplikácie
  db:
    image: docker.io/library/postgres:17
    environment:
      PGPORT: "5433"
    volumes:
      - db:/var/lib/postgresql/data
volumes:
  data: {}
  db: {}
```

`x-losos` je blok samotného zariadenia:

| Kľúč | Význam |
|---|---|
| `port` | Port, na ktorom počúva stránka aplikácie. Tam vedie odkaz **Otvoriť** aj presmerovanie z LAN. Bez neho zariadenie vyberie port uvedený v `ports` alebo `expose`, prednostne službu s názvom `web` alebo `ui`. |
| `images` | Archívy obrazov v úložisku Nix, načítané pred spustením aplikácie. Pre aplikácie zostavené Nixom; aplikácia odinakiaľ si obrazy stiahne z registra. |

## Čo s ním zariadenie urobí

Každá služba beží:

- na vlastnej sieti zariadenia (`network_mode: host`), takže `ports` a
  `expose` zariadeniu iba hovoria, kde aplikácia počúva;
- ako uid 1000 v kontajneri a ako `notshared` mimo neho
  (`userns_mode: keep-id`), takže to, čo aplikácia zapíše, patrí `notshared`;
- bez oprávnení (capabilities) a s `no-new-privileges`;
- znova sa spustí, ak ju nik nezastavil, pokiaľ súbor nehovorí inak.

Služby sa navzájom nájdu menom: každý názov služby ukazuje na `127.0.0.1`.
Keďže zdieľajú jednu sieť, musia počúvať na rôznych portoch, všetkých od
1024 vyššie, a na žiadnom z portov, ktoré si zariadenie necháva pre seba
(jeho webový server, administrátorské API na 8082, Forgejo na 3000,
PostgreSQL na 5432, Redis na 6379, porty klastra, rozsah presmerovaní 30000
až 30099).

Zväzky:

| V súbore | Na zariadení |
|---|---|
| `name:/path`, s `name` pod `volumes:` | `/home/notshared/data/apps/<app>/<name>`, zostáva aj po odstránení aplikácie |
| `./path:/path` | súbor alebo priečinok, ktorý aplikácia prináša, z jej priečinka projektu |
| samotné `/path` | anonymný zväzok v úložisku obrazov, zmizne s kontajnerom |
| `type: tmpfs` | pamäť |

`secrets:` a `configs:` so súborom `file:`, ktorý aplikácia prináša, sa
pripoja iba na čítanie na `/run/secrets/<name>` a `/<name>`. `env_file` musí
pomenovať súbor, ktorý aplikácia prináša.

Zariadenie odmietne, vetou, ktorá pomenuje službu a kľúč: `privileged`,
`cap_add`, `devices`, `pid` alebo `ipc`, akýkoľvek `network_mode` okrem
`host`, `build` (aplikácie sa na zariadení nezostavujú), `extends`,
`volumes_from`, `sysctls`, cestu mimo aplikácie (`/etc`, `../`), pomenovaný
zväzok s ovládačom alebo označený ako `external`, port pod 1024 alebo port,
ktorý si zariadenie necháva, a port, ktorý používa iná aplikácia.

## Inštalácia

`POST /api/apps/compose` prijme súbor a súbory vedľa neho:

```json
{
  "release": "board",
  "title": "Board",
  "compose": "x-losos:\n  port: 8080\nservices: ...",
  "files": { "site/nginx.conf": "...", "site/index.html": "..." }
}
```

`release` pomenuje aplikáciu a jej priečinok s dátami: malé písmeno, potom
najviac 39 malých písmen, číslic alebo pomlčiek. Súbory sú text, najviac
500 a spolu 4 MiB. Odpoveďou je záznam aplikácie s `phase: "installing"`;
`GET /api/apps` ju ukáže ako `running` alebo `failed`, s dôvodom. Poslanie
toho istého názvu znova aplikáciu nahradí a zachová jej dáta aj port.

Aplikácia dostane port z `losos.apps.ports`, dostupný iba z lokálnej siete.
**Odstrániť** na paneli **Aplikácie** alebo `POST /api/apps/remove` ju
zastaví a port zatvorí. Priečinok s dátami zostane.

## Vlastné aplikácie zariadenia

Modul prináša jednu z aplikácií zariadenia takto:

```nix
losos.apps.builtin.board = {
  compose = {
    "x-losos" = {
      port = 8091;
      images = [ boardImage ]; # obraz z pkgs.dockerTools
    };
    services.web = {
      image = "localhost/board:1";
      volumes = [ "data:/var/lib/board" ];
    };
    volumes.data = { };
  };
  files = ./board-files; # alebo null
  path = "/board/";
};
```

Pri každom štarte ju `losos-builtin-board.service` upraví ako
nainštalovanú aplikáciu a spustí. S `path` ju vstupná brána podáva na tejto
ceste, bez predpony a iba do lokálnej siete, ak nie je `lanOnly = false`;
`locationConfig` pridá do tej location riadky nginx.
Jej názov si nainštalovaná aplikácia nemôže vziať a panel **Aplikácie** ju
nemôže odstrániť.

:::caution[Kto volá]
Aplikácia na sieti hostiteľa počúva na porte, ku ktorému sa dostane každý
proces na zariadení, aj ostatné aplikácie. Hlavičku, ktorú nastaví nginx,
napríklad kto je prihlásený, nesmie brať ako dôkaz. Služba, ktorá to
potrebuje, beží na sokete Unix, ktorý môže otvoriť iba nginx, mimo tohto
formátu.
:::

## Kde to je

| Čo | Kde |
|---|---|
| Záznam aplikácie a súbory, ako prišli | `/var/lib/losos/apps/<app>/` (iba root) |
| Repozitár aplikácií: súbor Compose a súbory každej aplikácie, jeden commit na inštaláciu, aktualizáciu a odstránenie | `/var/lib/losos-podman/notshared/apps/<app>/` (vlastný Git repozitár používateľa `notshared`) |
| Projekt, ktorý spúšťa Podman Compose | `/var/lib/losos-podman/notshared/projects/<app>/compose.json` |
| Úložisko obrazov | `/var/lib/losos-podman/notshared/storage` |
| Dáta aplikácie | `/home/notshared/data/apps/<app>/` |
| Jednotka, ktorá ju spúšťa | `losos-podman-app@<app>.service` (`up` pri štarte, `down` pri zastavení) |
| Záznam poslednej inštalácie | `/var/lib/losos/apps/<app>/job.log` |

Pravidlá sú v `backend/src/compose.rs`, jednotky v `modules/apps.nix` a
`nix/tests/podman-apps.nix` spustí zariadenie bez klastra a pustí na ňom
jednu aplikáciu každého druhu.

Repozitár aplikácií je oddelený od konfigurácie zariadenia v `/etc/nixos` a
patrí používateľovi `notshared`, takže história toho, čo vlastník spúšťal,
zostáva na jeho strane zariadenia. Chart je v ňom uložený ako súbor Compose,
ktorý z neho zariadenie urobilo. Vlastné aplikácie zariadenia v ňom nie sú:
patria do konfigurácie. Súbory aplikácie môžu obsahovať jej heslá, preto
repozitár nemôže čítať nikto okrem `notshared` (a roota).
