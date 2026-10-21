---
title: Vyskúšajte vo virtuálnych strojoch
sidebar_position: 6
---

# Vyskúšajte vo virtuálnych strojoch

Jeden riadok na počítači s Linuxom pripraví dve zariadenia LosOS a medzi
nimi edge, ako virtuálne stroje na súkromnej sieti, a prejde s vami tým, čo
robia spolu:

```sh
curl -fsSL https://raw.githubusercontent.com/dasmatus/losos/main/nix/demo/box-edge-box/bootstrap.sh | sh
```

Okrem [Nixu](https://nixos.org/download) netreba nič inštalovať vopred. Ak
Nix chýba, skript vypíše riadok, ktorý ho nainštaluje, a skončí. Nič
nepotrebuje root, nič sa neinštaluje mimo `/nix` a priečinka ukážky
(`~/.local/share/losos-demo`) a k virtuálnym strojom sa dostanete len cez
`127.0.0.1`.

## Čo skript robí \{#what-the-script-does}

Najprv si pozrie počítač: Linux na x86_64, KVM, procesory, pamäť, voľné
miesto na disku a Nix. Potom zapíše druhý skript, `stage2.sh` v priečinku
ukážky, s tým, čo zistil, na jeho začiatku, a spustí ho. Druhý skript
všetko zostaví alebo stiahne a spustí virtuálne stroje. Neskôr ho môžete
spustiť znova aj samostatne.

Ukážka má dve veľkosti a skript vyberie väčšiu, ktorú počítač unesie:

| | Plná | Ľahká |
| --- | --- | --- |
| Potrebuje | KVM, 14 GiB pamäte, 60 GB voľného miesta | 6 GiB pamäte, 15 GB voľného miesta |
| Zariadenia | nainštalované z inštalačného ISO, s LosOS cloud | admin stránky a časti, ktoré sa rozprávajú s edge |
| Edge | virtuálny stroj s edge, ktorý je aj routerom siete | [edge gateway](../in-depth/edge-federation.md) |
| Trvá | asi pol hodiny s KVM | asi 15 minút, s KVM aj bez, po prvom zostavení |

`--full` alebo `--light` za `sh -s --` vyberie veľkosť ručne, napríklad
`... | sh -s -- --light`.

## Čo uvidíte \{#what-you-see}

1. Dve zariadenia a žiadny edge. Každé ho hľadá a nič nenájde. Keď ho
   požiadate, aby zdieľalo svoj disk, odmietne, lebo nie je edge, cez ktorý
   by zdieľalo.
2. Edge sa zapne. Obe zariadenia ho na sieti nájdu samy.
3. Edge zverejní obe zariadenia pod ich verejnými názvami, každé jedným
   tunelom.
4. Zariadenie A otvorí zariadenie B cez edge pod jeho verejným názvom a
   zariadenie B otvorí zariadenie A. Admin stránky sú na tejto ceste
   odmietnuté. Zostávajú na lokálnej sieti.
5. Obaja vlastníci dovolia edge spravovať svoje zariadenie a vlastná stránka
   edge ukáže obe.
6. S edge v dosahu obe zariadenia prijmú zdieľanie disku.
7. Edge sa vypne. Obe zariadenia si to všimnú, zavrú tunely, znova odmietnu
   zdieľanie a každé ďalej slúži svojej sieti.

Ak to chcete niekomu ukázať krok po kroku, pridajte `--pause`. Ľahká ukážka
potom pri každom kroku počká a vypíše, kde v prehliadači otvoriť stránku
Mesh každého zariadenia a stránku edge.

## Čím sa líši od skutočného nasadenia \{#how-it-differs-from-a-real-setup}

- Edge nemá certifikát, lebo súkromná sieť nemá verejný názov, ktorý by
  mohla preukázať. Skutočný edge s verejným názvom ho dostane od Let's
  Encrypt.
- Verejné názvy zariadení nasmeruje na edge ukážka. V skutočnej sieti to
  robí DNS.
- Edge je firemný, nie oficiálny, takže trh zostáva vypnutý.
- V ľahkej ukážke LosOS cloud nebeží. Namiesto neho odpovedá malá náhrada,
  za skutočným webovým serverom zariadenia.
