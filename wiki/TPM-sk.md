[English](TPM) · **Slovenčina** · [Deutsch](TPM-de)

# TPM a odomykanie disku

Zariadenie bez TPM čipu je menej bezpečné ako zariadenie s ním. Táto stránka
vysvetľuje, čo je TPM, na čo ho LosOS používa a čo stráca zariadenie, ktoré
ho nemá.

## Čo je TPM

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

## Na čo ho LosOS používa

Dátový zväzok (`/persist`) je na každom zariadení šifrovaný cez LUKS.
Inštalátor vytvorí náhodný kľúč, naformátuje ním zväzok a potom kľúč
zapečatí do TPM pomocou `systemd-cryptenroll`. Pri štarte si initrd vypýta
kľúč od čipu a otvorí zväzok. Bootovací oddiel nenesie žiadne tajomstvo.

Zariadenie do čipu zapečatí aj dve menšie tajomstvá:

- Tajomstvá, ktoré si zariadenie vytvára samo, napríklad prvé
  administrátorské heslo Nextcloudu (`modules/keyring.nix`, cez
  `systemd-creds`).
- Kľúč, ktorý šifruje adresár úložiska pre mesh (`modules/fscrypt.nix`).

Kópia diskového kľúča zostáva aj vnútri šifrovaného zväzku na
`/etc/keys/persist-keyfile`. `losos-ctl grow` ju používa na zmenu veľkosti
zväzku. Zvonku zväzok otvoriť nedokáže, pretože je v ňom.

## Čo stráca zariadenie bez TPM

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
   v LosOS cloud, repozitáre v LosOS Git, admin kľúč a zvyšok `/persist`.
2. Vlastné tajomstvá zariadenia prídu o svoj čip. Keyring sa vráti
   k hostiteľskému kľúču `systemd-creds` a kľúč úložiska pre mesh
   k obyčajnému súboru na `/var/secrets/losos-shared-key`. Oba ležia na tom
   istom zväzku ako dáta, ktoré chránia. Kópia `/persist`, ktorá opustí
   zariadenie, si kľúče vezme so sebou, a chyba, ktorá službe dovolí čítať
   ľubovoľný súbor ako root, prečíta obe polovice.

Sieťová strana sa nemení. LAN guard, prihlasovanie, tunel, hardening
a oddelenie oboch účtov fungujú rovnako s čipom aj bez neho.

## Pred čím TPM nechráni

LosOS zapečatí kľúč bez naviazania na merania firmvéru (PCR). Zariadenie
každú noc aktualizuje svoj bootloader a kernel bez toho, aby na to niekto
dohliadal. Kľúč naviazaný na tieto merania by po aktualizácii zamkol
zariadenie mimo jeho vlastného disku a zariadenie nemá shell, z ktorého by sa
z toho dalo zotaviť.

Čip preto vydá kľúč čomukoľvek, čo sa na tom počítači naštartuje, vrátane
USB kľúča zlodeja. TPM zabezpečí, že samotný disk je nečitateľný. Nepomôže
však, keď má niekto celé zariadenie. [Bezpečnostný model](Security-Model-sk)
to uvádza ako známe obmedzenie.

## Ako zistiť, v akom režime zariadenie je

- Inštalátor vypíše `unlock: TPM2` alebo `unlock: keyfile in the initrd`
  a pri ceste s keyfile tesne pred dokončením aj varovanie.
- Obrazovka zariadenia zobrazuje varovanie pod jeho adresou.
- Admin stránky zobrazujú varovanie v Settings, Security a v sprievodcovi
  nastavením, keď je nastavené heslo.
- Settings, Advanced uvádza `tpm.enable` a hodnotu, s ktorou zariadenie beží.

## Presun zariadenia na TPM

Inštalátor zapíše režim odomykania do `modules/install-target.nix`. Admin
stránky ho zmeniť nemôžu, pretože riadok preň v `overrides.nix` by sa
dostal do konfliktu s tým od inštalátora. Cesta k TPM vedie cez
preinštalovanie.

1. Skopírujte si súbory zo zariadenia. Preinštalovanie disk naformátuje.
2. Zapnite TPM v nastaveniach firmvéru. Hľadajte "TPM", "Security Device",
   "Intel PTT" alebo "AMD fTPM", zvyčajne v časti Security alebo Advanced.
3. Preinštalujte z [inštalačného ISO](Install-sk). Inštalátor čip nájde
   a použije ho. `losos-ctl install --tpm` urobí z chýbajúceho čipu chybu
   namiesto tichej inštalácie s keyfile.

VM potrebuje emulovaný čip pri inštalácii aj pri každom ďalšom štarte.
Riadky pre swtpm sú v [Inštalácii](Install-sk#try-it-in-a-vm-bios).
Adresár so stavom swtpm uchovávajte spolu s obrazom disku, inak VM svoj disk
neodomkne.

Režim s keyfile je v poriadku pre VM, testovacie zariadenie alebo
zariadenie, na ktorom nie je nič súkromné. Pre skutočné súbory vlastníka na
zariadení, ktoré by niekedy mohlo opustiť domov, použite TPM.
