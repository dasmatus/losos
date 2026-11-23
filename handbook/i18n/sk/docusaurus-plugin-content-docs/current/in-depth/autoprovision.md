---
title: Autoprovision
sidebar_position: 5
mdx:
  format: md
---

# Autoprovision vrátane netboot média

Autoprovision je obmena inštalátora pre stroje, ktoré mesh bootuje zo
siete, a súčasne synchronizácia upravenej konfigurácie, takže má každý box
lokálneho meshe rovnakú. Tri kusy:

* `losos-ctl provision` — obmena `losos-ctl install` s jedným krokom
  navrchu: stroj vezme konfiguráciu meshe zo *zrkadla*, takže box
  nainštalovaný provisionom beží s tým, čo mesh beží, nie so východzími
  predvoľbami.
* `nixosConfigurations.netboot` — netbootová obmena inštalačného média:
  kernel, initrd a squashfs store pre klienta PXE/iPXE, ktorého autorun
  končí v `losos-ctl provision`.
* `losos.autoprovision.enable` — synchronizácia na boxi: lososd udržuje
  `modules/overrides.nix` tohto boxu v kroku s tým istým zrkadlom, takže
  zmena aplikovaná na jednom boxi dosiahne aj ostatné.

## Zrkadlo

Jedna vetva jednej git adresy, dosiahnuteľná bez pýtania zo všetkých boxov
meshe (obyčajné http(s), bare repozitár na ceste k súboru, alebo ssh
adresa, ktorú daemon otvorí bez otázok po kľúči). Všetko na ňom je
zdieľané v meshe a nič na ňom nie je tajné: LUKS kľúče a tokeny neopustia
box, ktorému patria.

```
refs/heads/provision
├── modules/overrides.nix                upravená konfigurácia meshe
└── boxes/
    ├── box-9f13c4ad/install-target.nix  hardvér samotného stroja
    ├── box-9f13c4ad/overrides.nix       konfigurácia, s ktorou bol nainštalovaný
    └── mattbox/…
```

Stroj netbootovaný na provision sa volá `box-<first 8 hex of the machine
id>`, pokiaľ operátor nežiada meno (`losos-autoprovision --name lease-3`).
V LosOS Gite meshe boxu je to ďalší súkromný klon repozitára: vytvorte ho
tam pre zrkadlo, alebo ukazačte na ľubovoľný git vzdialený repozitár, ktorý
mesh dosiahne.

## Provisioning netbootovaného stroja

Nabootujte stroj s `boot.ipxe` z netboot image, alebo spustite príkaz
ručne na ľubovoľnom inštalačnom médiu:

```
nix build .#nixosConfigurations.netboot.config.system.build.netbootImage
# serve the result over http, point iPXE at boot.ipxe, and
# replace SERVER in it with the netboot server
```

Čo jeden provision urobí, v poradí:

1. Prečíta zrkadlo (`git ls-remote`, potom plytký fetch vetvy `provision`)
   a vezme jeho `modules/overrides.nix`. Zrkadlo bez vetvy, vetva bez
   konfigurácie, alebo konfigurácia, ktorá neprejde tým istým tvarovým
   testom, čím Apply na boxi, sa zastaví tu, pred diskami.
2. Ponuka firmwaru nezmenená zo sticku: vyráta 30 sekúnd a ukáže, čo
   urobí.
3. Obyčajný inštalačný plán s jedným krokom navyše: `modules/overrides.nix`
   z meshe sa položí do klonu flake vedľa generovaného
   `modules/install-target.nix`, takže inštalácia stavia nastavenia meshe.
4. Keď je strom na disku, box sa zapíše do zrkadla ako `boxes/<name>/` na
   nový commit cez hlavičku vetvy (iba fast-forward, takže dva stroje
   provisionované naraz sa vyvrátia ako prepísanie). Zrkadlo, ktoré zápis
   nevezme, je varovanie, nikdy neúspešná inštalácia.

Vlajky, ktoré inštalácia berie, berie `provision` tiež: voľby otvárania
diskov (`--tpm`/`--no-tpm`, `--tang`/`--tang-edge`, `--bios`/`--uefi`,
zoznam diskov) a `--no-install`. Testové cesty, ktorými prebuilt disko
skript alebo `--emit-target` formuje, odmieta, pretože tie stromy boxu
nestavia.

Prostredie buildu (alebo `losos.autoprovision.tang`) môže vlastné voľby
meshe predpísať do média, takže pri boxe nikto nič nepíše:
`LOSOS_TANG=URL#THUMBPRINT[,more]` (alebo možnosť
`losos.autoprovision.tang`) spôsobí, že autorun inštaluje s `--tang` — kľúč
disku je zapečatený na TPM2 *a* na tom Clevis Tang serveri (`tang-show-keys`
na serveri vytlačí thumbprint). Box provisionovaný takto sa nepodarí
otvoriť z jeho disku samej, od prvého bootu; `--tpm`, `--tang-edge` a
každa iná rukou napísaná voľka stále prevažuje nad predpísanou.

## Ako boxy zostávajú v kroku

Zapnutie (`modules/autoprovision.nix`):

```nix
losos = {
  autoprovision = {
    enable = true;
    mirror = "https://git.home.example/mesh-config.git";
    # box = "mattbox";  # defaults to losos.hostName
  };
};
```

Rekonziliátor konfigurácie lososd — to isté vlákno, ktoré vedie repozitár
konfigurácie na vlastnom LosOS Gite boxu — pozrie tiež každú pol minútu do
zrkadla a rozhodne z dvoch skutočností: text boxu a text hlavičky zrkadla
proti tomu, čo si napravec z oboch naposledy zapísal. Keď sa obe pohnú →
víťazí najnovší pisateľ (commit čas hlavičky zrkadla proti chvíli, keď
tento box naposledy pushoval). Pohla sa len jedna strana → tá strana je
pravda; box pushne alebo vezme.

Vezma prejde bránami, ktorými prechádza každý Apply — tvarový test,
options dokument a edge brána — a keď sa jej niečo odmietne, ostatné
ostáva odmietnuté s dôvodom na mesh riadku v History panely (`GET /api/config`,
pole `mesh`). Žiadny box nikdy nepushuje silou. Keď zrkadlo nemá ešte vetvu,
prvý box, ktorý sa ňou pozrie, pushne vlastnú konfiguráciu a zaseje ju. A
keď provisionovaný box nainštaluje konfiguráciu, ktorá sa potom posunie,
ďalšia sync ju pushne späť a celý mesh comes prestavia s ňou; to je presne
bod celého systému.

`losos-ctl config` — History panel — nesie odpoveď zrkadla vedľa odpovede
LosOS Git sync a zavedenej vetvy (`mesh.sync.state`).

## Spolužitie

Všetci traja stráže `/etc/nixos` bežia na jednom vlákne daemona a berú
jeden state lock, takže neumožnia si písať v rovnakom čase:

* LosOS Git — vlastná konfigurácia *boxu*, súkromná
  (`losos.configRepo.enable`), pokrýva každý súbor, ktorý drží.
* zavedená vetva — nové verzie LosOS ([Install in depth](install.md)).
* zrkadlo — `modules/overrides.nix` *meshe* len; fakty stroja sa nikdy
  nevracajú zo zápisu `boxes/`.

Publikovať do zrkadla meshe, verejného pre cudzích, je presne tak
úmyselné ako požičať úložisko: `losos.autoprovision.enable` je v predvolenom
vypnuté, a vziať konfiguráciu meshe znamená pustiť si to, čo mesh
commituje.
