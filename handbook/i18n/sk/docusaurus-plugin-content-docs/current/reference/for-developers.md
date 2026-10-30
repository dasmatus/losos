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

## Pred commitom \{#before-you-commit}

Repozitár nesie v `nix/hooks/` git hooky, ktoré pred commitom alebo pushom
spustia kontroly CI pre súbory, ktoré meníte. Zapne ich `devenv shell` aj
`nix develop`. Bez nich ich vo svojom klone zapnite raz týmto príkazom:

```sh
bash nix/hooks/install
```

Hook pri commite naformátuje a skontroluje Rust a Nix, ktorých ste sa
dotkli, overí typy webového rozhrania a vyhodnotí každý výstup flaku. Hook
pri pushi pridá testy Rustu, testy webového rozhrania v prehliadači a VM
testy zmenených súborov. Záznamy každého behu sú v `.git/losos-hooks/logs/`.
Všetky kontroly vymenúvajú poznámky pre vývojárov v `docs/development.md`.

## Úpravy tejto príručky \{#changing-this-handbook}

Zdroj je `handbook/` v repozitári, stránka postavená na
[Docusaurus](https://docusaurus.io/). Stránky sú v Markdowne pod
`handbook/docs/`. Bočný panel kopíruje strom priečinkov a každý priečinok
dostane názov zo svojho `_category_.json`.

```sh
cd handbook
bun install --frozen-lockfile --ignore-scripts
bun run start             # live preview on http://localhost:3000
bun run build             # fails on a broken link, on purpose
```

Každý push do `main` zverejní stránku na [losos.dasmat.us](https://losos.dasmat.us)
(`.github/workflows/handbook.yml`) a každé zariadenie si zostaví vlastnú
kópiu (`losos-handbook` vo `nix/packages.nix`, poskytovanú na `/handbook/`
cez `modules/containers.nix`). Docusaurus beží v prostredí
[Bun](https://bun.sh), nie v Node. Po zmene `bun.lock` obnovte `hash` príručky vo
`nix/packages.nix`:

```sh
python3 .github/scripts/fix-hashes.py
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
  nenasníma. Obrázky ponuky pravého tlačidla sú tiež zo zariadenia vo VM:
  `nix/tests/context-menu.nix` ich vo všetkých troch jazykoch zapíše do
  `shots/after/handbook/` vo svojom výstupe.
- **Diagramy.** Sú to ručne písané SVG na bielej karte, vo farbách admin
  rozhrania (`admin-ui/app/src/styles/tokens.css`), takže sa čítajú rovnako
  vo svetlej aj tmavej téme. Upravujte ich ako text.

```sh
cd admin-ui/app
bun run build
bun tests/handbook-screens.mjs /tmp/handbook-screens           # every shot
bun tests/handbook-screens.mjs /tmp/handbook-screens mesh      # names containing "mesh"
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
bun run write-translations --locale sk      # refresh the string files
bun run start --locale sk                   # preview one locale
```

## Pravidlá písania, ktorými sa táto príručka riadi \{#writing-rules-this-handbook-follows}

- Začnite tým, čo čitateľ vidí; príčinu dajte na druhé miesto a opravu na
  tretie.
- Každá stránka o riešení problémov začína krokmi, ktorým stačí LAN.
- IP adresa je vždy pred názvom `.local`.
- Keď sa správanie nedávno zmenilo, uveďte pull request, aby čitateľ so
  starším zariadením vedel, prečo jeho obrazovka vyzerá inak.
- Žiadne dlhé pomlčky.
