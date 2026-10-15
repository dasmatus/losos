---
title: Secure Boot a podpísané médiá
sidebar_position: 5
---

# Secure Boot a podpísané médiá

Inštalačné ISO každého vydania je podpísané a počítač so zapnutým Secure
Boot ho dokáže overiť skôr, než sa spustí jediný bajt LosOS. Táto stránka
hovorí, čo firmvér overuje, ako svojmu firmvéru povoliť dôverovať LosOS
a ako stiahnutý súbor overiť ručne.

## Čo sa overuje a kto to overuje \{#what-is-verified-and-by-whom}

So zapnutým Secure Boot overuje firmvér UEFI jednu vec: program, ktorý sa
chystá spustiť, voči certifikátom vo svojom zozname `db`. LosOS zariadi,
aby táto jedna kontrola pokryla celý štart:

1. **Firmvér overí zavádzač.** Zavádzač UEFI na USB kľúči,
   `EFI/BOOT/BOOTX64.EFI`, je jeden *unified kernel image*. V jednom súbore
   obsahuje linuxové jadro, počiatočný ramdisk a príkazový riadok jadra,
   podpísané certifikátom LosOS. Zmena jediného bajtu v ktorejkoľvek z týchto
   troch častí poruší podpis a firmvér ho odmietne spustiť. Pred touto
   kontrolou z USB kľúča nebeží nič.
2. **Zavádzač overí systém.** Zvyšok USB kľúča je jeden obraz systému
   s veľkosťou 1,5 GB, `nix-store.squashfs`. Jeho hash SHA-256 je súčasťou
   podpísaného príkazového riadka a počiatočný ramdisk obraz na USB kľúči
   pred pripojením zahashuje. Upravený obraz odmietne, na obrazovke ukáže
   *THE MEDIUM HAS BEEN ALTERED* a počítač vypne.

![USB kľúč, ktorého obraz systému bol po podpísaní zmenený: zavádzač ho odmietne hláškou THE MEDIUM HAS BEEN ALTERED a vypíše očakávaný a nájdený hash.](@site/docs/img/secure-boot-refused-tampered.png)

Inštalátor potom pri svojom menu firmvéru oznámi, čo sa stalo:

```
Secure Boot: enabled. The firmware verified this medium's signature.
```

alebo `Secure Boot: disabled ...`, keď firmvér nič nekontroloval, alebo
`Secure Boot: not available (legacy BIOS boot)`. Secure Boot je funkcia UEFI.
USB kľúč naštartovaný v režime BIOS neoveruje nič, rovnako ako pri
akomkoľvek inom štarte cez BIOS.

![Prvá obrazovka inštalátora na podpísanom USB kľúči: Secure Boot je zapnutý, firmvér overil podpis tohto média.](@site/docs/img/installer-menu.png)

Čo pokryté **nie je**, je nainštalovaný systém. Zariadenie štartuje cez
systemd-boot zo svojho vlastného disku. Podpísaný nie je ani tento zavádzač,
ani jadrá, ktoré inštalujú jeho nočné prestavby, takže samotné zariadenie
potrebuje na štart Secure Boot **vypnutý**. Dnes je postup takýto: zapíšte
certifikát, naštartujte USB kľúč so zapnutým Secure Boot, aby firmvér
médium overil, nainštalujte a pred prvým štartom z disku Secure Boot
vypnite. Alebo ho nechajte vypnutý celý čas a namiesto toho overte
stiahnutý súbor ručne, ako je opísané nižšie. Podpisovanie zavádzacieho
reťazca nainštalovaného systému by pri každej prestavbe potrebovalo
podpisový kľúč na zariadení. To je samostatný návrh a do tohto nepatrí.

## Ako firmvéru povoliť dôverovať LosOS \{#letting-your-firmware-trust-losos}

Počítač prichádza s dôverou v certifikáty Microsoftu a v nič iné, takže so
zapnutým Secure Boot USB kľúč LosOS odmietne. OVMF a väčšina firmvérov
vypíše *Access Denied*, niektoré USB kľúč bez správy preskočia. Toto
odmietnutie znamená, že kontrola podpisu funguje. Ak chcete USB kľúč
naštartovať so zapnutým Secure Boot, pridajte certifikát LosOS do `db`
firmvéru, vedľa certifikátov Microsoftu:

![Firmvér, ktorý dôveruje iba kľúčom Microsoftu, odmieta USB kľúč LosOS: Access Denied, pravdepodobne odmietnuté cez Secure Boot.](@site/docs/img/secure-boot-refused-microsoft-keys.png)

1. Získajte certifikát. Je priamo na USB kľúči na
   `EFI/losos/losos-secure-boot.cer` vo formáte DER a ako `.pem` a v každom
   [vydaní](https://github.com/dasmatus/losos/releases/latest) ako
   `losos-secure-boot-db.cer`. Poznámky k vydaniu uvádzajú jeho odtlačok
   SHA-256 a `openssl x509 -in losos-secure-boot-db.pem -noout
   -fingerprint -sha256` vypíše to isté pre súbor, ktorý máte.
2. V nastaveniach Secure Boot vo firmvéri prepnite z režimu *Standard* na
   *Custom*. Zvoľte *Append to db*, *Enroll key* alebo *Add signature from
   file* a vyberte `.cer` z EFI oddielu USB kľúča. Názvy sa líšia podľa
   výrobcu. EFI oddiel USB kľúča je obyčajný zväzok FAT, ktorý vie prechádzať
   každý firmvér.
3. Naštartujte USB kľúč. Prvý riadok inštalátora hovorí *Secure Boot:
   enabled*.

### Certifikáty Microsoftu ponechajte \{#keep-microsofts-certificates}

LosOS do zoznamu pridajte. Nenahrádzajte zoznam LosOS. Na bežnom počítači
sú pod certifikátmi Microsoftu podpísané tri veci a prestanú štartovať
v momente, keď tieto certifikáty zmiznú z `db`:

- Vlastný správca zavádzania Windows.
- Iné linuxové distribúcie, ktorých zavádzač prvej fázy (shim) podpisuje
  Microsoft.
- Ovládače firmvéru na grafických a sieťových kartách, nazývané option ROM.
  Na stolnom počítači so samostatnou grafickou kartou môže ich strata
  znamenať úplne čiernu obrazovku, a to aj v nastaveniach firmvéru.

Preto sa vyhnite voľbám *Delete all keys*, *Clear Secure Boot keys* a *Reset
to Setup Mode*, pokiaľ ďalším krokom nie je *Restore factory keys*. Ak váš
firmvér dokáže pridať kľúč iba po ich vymazaní, najprv obnovte továrenské
kľúče a až potom pridajte certifikát LosOS.

Z bežiaceho Linuxu v režime *Setup* pridáte certifikát príkazom
`efi-updatevar -a -c losos-secure-boot-db.pem db` z balíka efitools.
S sbctl použite `--microsoft`: `sbctl enroll-keys --microsoft` ponechá
certifikáty Microsoftu vedľa tých, ktoré sbctl zapíše, a bez tohto
prepínača ich zahodí. Potom pridajte certifikát LosOS do db v sbctl.

Testovacia sada kontroluje presne tento stav. Jej firmvér má vedľa seba
certifikát LosOS, päť certifikátov `db` od Microsoftu a dva certifikáty
`KEK`. USB kľúč LosOS naštartuje a rovnako aj shim Ubuntu podpísaný
Microsoftom. Keď certifikáty Microsoftu odstránite, ten istý shim firmvér
odmietne:

![Firmvér so zapísanými certifikátmi LosOS a Microsoftu: z naštartovaného USB kľúča LosOS KEK uvádza testovací KEK LosOS a dve KEK CA od Microsoftu a db uvádza testovací certifikát LosOS a päť CA od Microsoftu.](@site/docs/img/secure-boot-cosigned-db.png)

Zápis certifikátu je jednorazová zmena daného počítača.

Zavádzač LosOS podpisuje LosOS, nie Microsoft. USB kľúč, ktorý by
naštartoval na bežnom počítači bez zapísaného certifikátu, by potreboval
shim, ktorý Microsoft podpísal pre LosOS, po verejnej revízii shimu
a podpisovom procese Microsoftu. Tým LosOS neprešiel. Dovtedy si každý
počítač certifikát LosOS raz zapíše.

## Ručné overenie stiahnutého súboru \{#verifying-a-download-by-hand}

Každé vydanie obsahuje `SHA256SUMS` a oddelený podpis `SHA256SUMS.sig`,
vytvorený tým istým certifikátom, ktorý podpisuje médium:

```sh
R=https://github.com/dasmatus/losos/releases/download/<tag>
curl -fsSLO $R/SHA256SUMS $R/SHA256SUMS.sig $R/losos-secure-boot-db.pem
openssl dgst -sha256 -verify <(openssl x509 -pubkey -noout -in losos-secure-boot-db.pem) \
  -signature SHA256SUMS.sig SHA256SUMS
sha256sum -c --ignore-missing SHA256SUMS
```

Prvý príkaz vypíše `Verified OK`. Druhý skontroluje tie súbory z vydania,
ktoré ste stiahli. Ak chcete skontrolovať podpis zavádzača vo vnútri ISO,
ktoré už máte, spustite naň priamo nástroj z repozitára:

```sh
nix run github:dasmatus/losos#losos-sign-iso -- verify --cert losos-secure-boot-db.pem losos-installer-<tag>.iso
```

Odtlačok certifikátu porovnajte s tým v poznámkach k vydaniu a s
`keys/secure-boot-db.pem` v repozitári. Všetky tri sú ten istý súbor.

## Kde je kľúč \{#where-the-key-lives}

Súkromný kľúč sa nikdy nedostane do zostavenia Nix ani na zariadenie.
Vlastník ho raz vytvorí na vlastnom počítači pomocou
`provisioning/secure-boot/keygen.sh` a uchováva ho offline. Jediná ďalšia
kópia je tajomstvo repozitára, ktorým podpisuje CI. Certifikát je v
repozitári ako `keys/secure-boot-db.pem`. Kým tento súbor žiadny certifikát
neobsahuje, vydania uvádzajú *Not signed* a USB kľúč potrebuje Secure Boot
vypnutý. Testovacia sada overuje mechanizmus s jednorazovým kľúčom, ktorý
vytvorí a potom zahodí. `tests/secure-boot.nix` naštartuje podpísané médium
pod verziou OVMF so Secure Boot a zvnútra ukáže príznak *enabled* priamo
z firmvéru. Firmvér v tomto teste má certifikáty Microsoftu vedľa
testovacieho kľúča a shim podpísaný Microsoftom na ňom stále naštartuje.
Test tiež sleduje, ako firmvér odmietne to isté médium pri kľúčoch iba od
Microsoftu, upravenú kópiu aj nepodpísané zostavenie a ako odmietne shim,
keď certifikáty Microsoftu zmiznú.
