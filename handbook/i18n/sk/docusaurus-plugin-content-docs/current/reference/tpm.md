---
title: TPM a kľúč disku
sidebar_position: 4.5
slug: /reference/tpm
---

# TPM a kľúč disku

Zariadenie bez TPM čipu je menej bezpečné ako zariadenie s ním. Táto stránka
vysvetľuje, čo je TPM, na čo ho LosOS používa a čo stráca zariadenie, ktoré
ho nemá.

## Čo je TPM \{#what-a-tpm-is}

TPM (Trusted Platform Module) je malý bezpečnostný čip na základnej doske. Na
väčšine novších počítačov je namiesto samostatného čipu súčasťou firmvéru
procesora. Intel to volá PTT a AMD fTPM. Windows 11 vyžaduje TPM 2.0, takže
počítač predávaný s Windows 11 ho má. Staršie mini-PC ho často majú tiež,
niekedy vypnutý v nastaveniach firmvéru.

Čip uchováva vlastný kľúč, ktorý ho nikdy neopustí. Softvér môže čipu
odovzdať tajomstvo na zapečatenie a odpečatiť ho dokáže iba ten istý čip.
Zapečatené tajomstvo skopírované na iný počítač alebo prečítané z disku je
tam nepoužiteľné.

Presne to potrebuje zariadenie bez obsluhy. Pri každom štarte musí odomknúť
svoj šifrovaný disk bez toho, aby pri klávesnici sedel niekto, kto napíše
heslo. Kľúč musí byť niekde, kam sa zariadenie dostane samo, a TPM je jediné
také miesto, ktoré nie je samotný disk.

## Na čo ho LosOS používa \{#what-losos-uses-it-for}

Dátový zväzok (`/persist`) je na každom zariadení šifrovaný cez LUKS.
Inštalátor vytvorí náhodný kľúč, naformátuje ním zväzok a potom kľúč
zapečatí do TPM pomocou `systemd-cryptenroll`. Pri štarte si initrd vypýta
kľúč od čipu a otvorí zväzok. Bootovací oddiel nenesie žiadne tajomstvo.

Zariadenie do čipu zapečatí aj menšie tajomstvá:

- Kľúč, ktorý šifruje adresár úložiska pre mesh (`modules/fscrypt.nix`).
- Každé tajomstvo vo svojom úložisku prihlasovacích údajov: náhradný kľúč,
  kľúče LosOS Login a kľúče bucketu na zálohy (pozrite
  [Prihlasovacie údaje](credentials.md)).

:::note[Záložná kópia kľúča]
Kópia diskového kľúča zostáva aj vnútri šifrovaného zväzku na
`/etc/keys/persist-keyfile`. `losos-ctl grow` ju používa na zmenu veľkosti
zväzku. Zvonku zväzok otvoriť nedokáže, pretože je v ňom.
:::

## Čo stráca zariadenie bez TPM \{#what-a-box-without-a-tpm-gives-up}

Inštalátor zväzok šifruje stále rovnako. Mení sa to, kde je kľúč pri štarte.
Bez čipu inštalátor vloží kľúč do initrd ako `/crypto_keyfile.bin`. Initrd
leží na bootovacom oddiele (ESP) a bootovací oddiel nie je šifrovaný. Kľúč
leží hneď vedľa zámky.

Má to dva dôsledky.

1. Ktokoľvek, kto získa disk, si môže prečítať všetko, čo je na ňom. Platí to
   pre disk vybratý zo zariadenia, ukradnuté zariadenie, obraz disku,
   zariadenie poslané do opravy aj starý disk predaný alebo vyhodený bez
   vymazania. V každom z týchto prípadov si ten, kto ho číta, pripojí ESP,
   skopíruje kľúč z initrd a otvorí zväzok. Stačia na to
   bežné linuxové nástroje a pár minút. Odhalí to súbory vlastníka
   v LosOS Cloud, repozitáre v LosOS Git, admin kľúč a zvyšok `/persist`.
2. Kľúč úložiska pre mesh príde o svoj čip a vráti sa k obyčajnému súboru
   na `/var/secrets/losos-shared-key`, na tom istom zväzku ako dáta, ktoré
   chráni. Kópia `/persist`, ktorá opustí zariadenie, si kľúč vezme so sebou, a chyba, ktorá službe dovolí čítať
   ľubovoľný súbor ako root, prečíta obe polovice.

Sieťová strana sa nemení. LAN guard, prihlasovanie, tunel, hardening
a oddelenie oboch účtov fungujú rovnako s čipom aj bez neho.

## Pred čím TPM nechráni \{#what-a-tpm-does-not-protect-against}

LosOS zapečatí kľúč bez naviazania na merania firmvéru (PCR). Zariadenie
každú noc aktualizuje svoj bootloader a kernel bez toho, aby na to niekto
dohliadal. Kľúč naviazaný na tieto merania by po aktualizácii zamkol
zariadenie mimo jeho vlastného disku a zariadenie nemá shell, z ktorého by sa
z toho dalo zotaviť.

:::warning[Zlodej s celým zariadením]
Čip vydá kľúč čomukoľvek, čo sa na tom počítači naštartuje, vrátane
USB kľúča zlodeja. TPM zabezpečí, že samotný disk je nečitateľný. Nepomôže
však, keď má niekto celé zariadenie, pokiaľ zariadenie na otvorenie disku
nepotrebuje aj server vo vlastnej sieti (ďalšia časť).
[Bezpečnostný model](/reference/security-model.md) to uvádza ako známe
obmedzenie.
:::

## Disk sa otvorí iba doma \{#opening-the-disk-only-at-home}

Zariadenie sa dá nastaviť tak, aby samotný čip nestačil. Dajte inštalátoru
jeden alebo viac serverov Tang vo vlastnej sieti zariadenia:

```sh
losos-ctl install --tang 'http://192.168.1.1:7500#THUMBPRINT'
```

Tang je malý server kľúčov. OpenWrt má balík `tang` a NixOS má
`services.tang`. Odtlačok (thumbprint) vypíše na serveri
`tang-show-keys 7500`. Inštalátor podľa neho server overí, takže pri prvom
kontakte ničomu nedôveruje.

Inštalátor potom zapečatí diskový kľúč cez Clevis k TPM a serveru Tang
spolu, nie iba k TPM. Pri každom štarte si zariadenie vezme adresu v
káblovej sieti, opýta sa servera a disk otvorí iba s oboma odpoveďami.
Server kľúč nikdy nevidí. Dozvie sa iba to, že zariadenie naštartovalo.

Čo to mení:

- Zariadenie odnesené zo svojej siete sa neotvorí, rovnako ani zariadenie,
  ktorého server zmizol. Zlodej s celým zariadením uvidí výzvu na heslo.
- Zariadenie sa pýta servera najviac minútu, potom obrazovka vypýta heslo.
  Zadajte kód na obnovu zariadenia, ktorý ukáže Nastavenia, Záloha.
  Zariadenie si kód pridá ako heslo hneď, ako ho má, a vymení ho, keď sa
  kód zmení.
- Server musí odpovedať vždy, keď zariadenie štartuje, aj pri nočnom
  reštarte o 00:07. Ak je vtedy vypnutý, zariadenie čaká na výzve, kým
  niekto kód nezadá.
- Samotný disk zostáva nečitateľný ako na každom zariadení s TPM.

Ako samotné TPM, aj toto nastavenie je v `modules/install-target.nix`
(`tpm.tang`), takže zapnúť alebo vypnúť ho možno iba novou inštaláciou.

## Ako zistiť, v akom režime zariadenie je \{#how-to-tell-which-mode-a-box-is-in}

- Inštalátor vypíše `unlock: TPM2` alebo `unlock: keyfile in the initrd`
  a pri ceste s keyfile tesne pred dokončením aj varovanie.
- Obrazovka zariadenia zobrazuje varovanie pod jeho adresou.
- Admin stránky zobrazujú varovanie na paneli Nastavenia, Zabezpečenie a v sprievodcovi
  nastavením, keď je nastavené heslo.
- Nastavenia, Rozšírené uvádza `tpm.enable` a hodnotu, s ktorou zariadenie beží.

## Presun zariadenia na TPM \{#moving-a-box-to-a-tpm}

Inštalátor zapíše režim odomykania do `modules/install-target.nix`. Admin
stránky ho zmeniť nemôžu, pretože riadok preň v `overrides.nix` by sa
dostal do konfliktu s tým od inštalátora. Cesta k TPM vedie cez
preinštalovanie.

1. Skopírujte si súbory zo zariadenia. Preinštalovanie disk naformátuje.
2. Zapnite TPM v nastaveniach firmvéru. Hľadajte "TPM", "Security Device",
   "Intel PTT" alebo "AMD fTPM", zvyčajne v časti Security alebo Advanced.
3. Preinštalujte z [inštalačného ISO](/start/install.md). Inštalátor čip nájde
   a použije ho. `losos-ctl install --tpm` urobí z chýbajúceho čipu chybu
   namiesto tichej inštalácie s keyfile.

:::warning[Stav swtpm uchovávajte s obrazom disku]
VM potrebuje emulovaný čip pri inštalácii aj pri každom ďalšom štarte.
Riadky pre swtpm sú v [Inštalácii](/in-depth/install.md#try-it-in-a-vm-bios).
Adresár so stavom swtpm uchovávajte spolu s obrazom disku, inak VM svoj disk
neodomkne.
:::

Režim s keyfile je v poriadku pre VM, testovacie zariadenie alebo
zariadenie, na ktorom nie je nič súkromné. Pre skutočné súbory vlastníka na
zariadení, ktoré by niekedy mohlo opustiť domov, použite TPM.
