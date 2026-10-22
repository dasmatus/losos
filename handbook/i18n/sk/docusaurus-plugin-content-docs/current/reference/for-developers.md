---
title: Pre vývojárov
sidebar_position: 6
---

# Pre vývojárov

Väčšina tejto príručky je pre vlastníkov. Zostavovanie, testovanie a úpravy
LosOS opisujú dve miesta:

- kapitola [Do hĺbky](/in-depth/index.md): Inštalácia podrobne, Správa,
  Mesh, Trh, Virtuálne stroje, Master proxy, Federácia edge, Diagramy, Hardening,
  Architektúra;
- [`.claude/CLAUDE.md`](https://github.com/dasmatus/losos/blob/main/.claude/CLAUDE.md)
  v repozitári: príkazy na zostavenie, architektúra naprieč súbormi a zoznam
  chýb, ktoré sa ľahko zopakujú.

## Úpravy tejto príručky \{#changing-this-handbook}

Zdroj je `handbook/` v repozitári, stránka postavená na
[Docusaurus](https://docusaurus.io/). Stránky sú v Markdowne pod
`handbook/docs/`. Bočný panel kopíruje strom priečinkov a každý priečinok
dostane názov zo svojho `_category_.json`.

```sh
cd handbook
npm ci
npm start                 # live preview on http://localhost:3000
npm run build             # fails on a broken link, on purpose
```

Každý push do `main` zverejní stránku na [losos.dasmat.us](https://losos.dasmat.us)
(`.github/workflows/handbook.yml`) a každé zariadenie si zostaví vlastnú
kópiu (`losos-handbook` vo `nix/packages.nix`, poskytovanú na `/handbook/`
cez `modules/containers.nix`). Po zmene `package-lock.json` obnovte
`npmDepsHash` vo `nix/packages.nix`:

```sh
nix run nixpkgs#prefetch-npm-deps -- handbook/package-lock.json
```

### Obrázky \{#pictures}

Obrázky sú v `handbook/docs/img/`. Sú troch druhov:

- **Admin stránky a sprievodca.** `admin-ui/app/tests/handbook-screens.mjs`
  ich nasníma zo zostavenej SPA voči náhradnému lososd, takže ukazujú
  aktuálne rozhranie a logo bez zariadenia. Zapisuje PNG v rozlíšení 2x,
  ktoré idú do príručky zmenšené na šírku 1600 px a zredukované na
  256 farieb. Väčšina z nich je aj v slovenčine a nemčine ako
  `<name>-sk.png` a `<name>-de.png` a preložené stránky ukazujú tie.
- **Obrazovky skutočného počítača.** Inštalátor, banner na tty1, výzva na
  heslo a odmietnutia pri Secure Boot pochádzajú z behov vo VM
  (`nix/tests/secure-boot.nix` a inštalácie do VM). Žiadny skript ich znova
  nenasníma.
- **Diagramy.** Sú to ručne písané SVG na bielej karte, vo farbách admin
  rozhrania (`admin-ui/app/src/styles/tokens.css`), takže sa čítajú rovnako
  vo svetlej aj tmavej téme. Upravujte ich ako text.

```sh
cd admin-ui/app
npm run build
node tests/handbook-screens.mjs /tmp/handbook-screens          # every shot
node tests/handbook-screens.mjs /tmp/handbook-screens mesh     # names containing "mesh"
```

:::note[Keď Playwright nemá Chromium]
Ak chýba vlastný Chromium od Playwrightu, nastavte `LOSOS_CHROMIUM` na
binárku Chromia.
:::

Alt text píšte tak, aby hovoril, čo obrázok ukazuje, nie čo hovorí stránka.

## Preklady \{#translating}

Slovenčina a nemčina sú zapojené. Reťazce rozhrania sú
v `handbook/i18n/<locale>/code.json`. Ak chcete preložiť stránku, skopírujte
ju do `handbook/i18n/<locale>/docusaurus-plugin-content-docs/current/<same path>`
a preložte kópiu. Stránka bez prekladu sa zobrazí v angličtine v preloženom
rámci stránky, takže môžete prekladať po jednej stránke.

:::tip[Držte tri jazyky v súlade]
Každá stránka je preložená celá. Meňte všetky tri jazyky naraz.
:::

```sh
npm run write-translations -- --locale sk   # refresh the string files
npm run start -- --locale sk                # preview one locale
```

## Pravidlá písania, ktorými sa táto príručka riadi \{#writing-rules-this-handbook-follows}

- Začnite tým, čo čitateľ vidí; príčinu dajte na druhé miesto a opravu na
  tretie.
- Každá stránka o riešení problémov začína krokmi, ktorým stačí LAN.
- IP adresa je vždy pred názvom `.local`.
- Keď sa správanie nedávno zmenilo, uveďte pull request, aby čitateľ so
  starším zariadením vedel, prečo jeho obrazovka vyzerá inak.
- Žiadne dlhé pomlčky.
