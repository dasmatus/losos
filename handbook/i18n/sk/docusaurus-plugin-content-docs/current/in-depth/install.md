---
title: Inštalácia podrobne
sidebar_position: 1
mdx:
  format: md
---

# Inštalácia podrobne

## Získajte inštalátor

Stiahnite si ISO a jeho `.sha256` z
[najnovšieho vydania](https://github.com/dasmatus/losos/releases/latest), alebo ho zostavte:

```sh
nix build .#nixosConfigurations.iso.config.system.build.isoImage
```

Zapíšte ho na USB kľúč a nabootujte z neho cieľový počítač. Menovka zväzku
na kľúči je `LOSOS_INSTALLER`. Bootovacie menu BIOS-u, úvodná obrazovka UEFI
a banner na konzole ukazujú LosOS a značku vydania a názov hostiteľa je
`losos-installer`. Pod tým je NixOS a os-release to aj uvádza cez
`ID_LIKE=nixos`. Nainštalované zariadenie nesie rovnaký názov a značku vo svojom
bootovacom menu, v bannerch na konzole aj v os-release (`NAME=LosOS`,
`IMAGE_VERSION`) a ponecháva si `ID=nixos`.

## Skôr než nabootujete

- **Secure Boot: vypnite ho, alebo zaregistrujte certifikát LosOS.** ISO
  vydaní sú podpísané. UEFI zavádzač na kľúči je jeden zjednotený obraz jadra
  (jadro, initrd, príkazový riadok) podpísaný db certifikátom LosOS a tento
  zavádzač pred pripojením obrazu systému overí jeho hash. Firmvér, ktorý
  dôveruje iba kľúčom Microsoftu, kľúč odmietne, kým do jeho `db` nepridáte
  certifikát z adresára `EFI/losos/` na kľúči alebo zo stránky vydania, a to
  vedľa certifikátov Microsoftu, nikdy namiesto nich: potrebuje ich Windows,
  shim iných distribúcií aj option ROM grafických kariet. OVMF zobrazí
  "Access Denied"; iný firmvér môže kľúč bez slova preskočiť.
  Zavádzač nainštalovaného zariadenia podpísaný nie je, takže samotné zariadenie potrebuje
  Secure Boot vypnutý. Postup a kontrolu `SHA256SUMS.sig` nájdete na stránke
  príručky *Secure Boot and signed media*. Dôkazom je `tests/secure-boot.nix`.
- **UEFI aj BIOS, funguje oboje.** Na inštalačnom ISO vám textové menu
  umožní zvoliť BIOS, UEFI alebo autodetekciu (firmvér, ktorý ISO
  nabootoval). UEFI dostane systemd-boot. Starší BIOS dostane GRUB a 1 MiB
  bootovací oddiel BIOS na prvom disku. Ak do 30 sekúnd neodpoviete, menu
  zvolí autodetekciu, takže aj bezobslužné bootovanie nainštaluje systém.
  `losos-install --bios` a `--uefi` zvolia režim bez zobrazenia menu. UEFI,
  z menu aj cez `--uefi`, vyžaduje, aby bol samotný kľúč nabootovaný v režime
  UEFI, pretože systemd-boot zapisuje bootovaciu položku firmvéru. Z kľúča
  nabootovaného cez BIOS to inštalátor odmietne skôr, než sa dotkne
  akéhokoľvek disku. BIOS slúži hlavne na testovanie vo VM.
  Menu je súčasťou iba inštalačného ISO. Nainštalované zariadenie ho nikdy
  nezobrazí.
- **Pripojte sieť.** Inštalátor klonuje flake počas behu.

## Čo inštalátor robí

Po výbere firmvéru inštalátor beží bez obsluhy. Nájde všetky pevné disky,
spojí ich do jednej skupiny zväzkov LVM, zašifruje ju cez LUKS, naformátuje
`/persist` ako ext4 a nainštaluje systém.

- Na počítači s čipom TPM 2.0 inštalátor hneď po formátovaní zapečatí kľúč
  disku do čipu (`systemd-cryptenroll`). Väčšina mini-PC ho má vo firmvéri,
  Intel PTT alebo AMD fTPM, zvyčajne zapnutý. Zariadenie potom od prvého štartu
  bootuje bez obsluhy a na bootovacom oddiele nie je nič tajné. Samotný disk,
  vytiahnutý alebo naklonovaný, sa nedá prečítať. Kľúč nie je viazaný na
  merania firmvéru (PCR), pretože zariadenie aktualizuje svoj firmvér aj zavádzač
  bez obsluhy a nemá shell, z ktorého by sa dalo zotaviť po zablokovaní.
  Cenou je, že zlodej, ktorý vezme celé zariadenie aj s čipom, sa k dátam stále
  dostane. Ten istý náhodný kľúč, ktorým bol zväzok naformátovaný, zostáva
  vnútri šifrovaného zväzku v `/etc/keys/persist-keyfile` ako záchranný slot.
- Bez čipu, alebo s `losos-ctl install --no-tpm` zo shellu inštalátora, sa
  namiesto toho tento súbor s kľúčom zapečie do initrd. Leží potom na
  nešifrovanom ESP a ktokoľvek, kto vezme disk, si dáta prečíta. Inštalátor
  vypíše, ktorú z dvoch možností zvolil. `--tpm` spraví z chýbajúceho čipu
  chybu namiesto tichej inštalácie so súborom s kľúčom.
  [TPM a kľúč disku](/reference/tpm.md) vysvetľuje, na čo čip slúži a čoho sa zariadenie
  bez neho vzdáva.
- Vo VM mu dajte TPM (swtpm, pozri nižšie), inak sa nainštaluje v režime so
  súborom s kľúčom.

`/persist` je ext4 s funkciou `encrypt`, pretože ju fscrypt potrebuje a btrfs
ju nepodporuje. Prichádzate o kompresiu a kontrolné súčty dát.

Po reštarte obrazovka ukazuje lososa, kým zariadenie štartuje. Keď je
hotové, losos sa presunie hore a panel pod ním ukáže IP adresu zariadenia a
`<hostname>.local`. Na stroji, kde úvodná obrazovka nenájde displej včas,
ukáže tty1 tie isté riadky ako textový banner. Otvorte IP adresu v prehliadači na ľubovoľnom počítači v
tej istej sieti. Meno `.local` funguje tiež všade, kde počítač prekladá mená
mDNS. Windows, macOS, telefóny a väčšina linuxových desktopov to robí;
hostiteľ hosťa libvirt alebo VirtualBox za NAT zvyčajne nie, takže tam
použite adresu. Obe vedú na tie isté stránky. Panel sa aktualizuje, keď sa
adresa zmení.

Prvá stránka je sprievodca nastavením. Jeho prvým krokom je dôverovať
vlastnému certifikátu zariadenia, aby zvyšok nastavenia aj každé neskoršie
prihlásenie šli cez HTTPS a váš prehliadač mohol ponúknuť passkey. Krok
ukazuje jeden riadok na vloženie do terminálu, vybraný pre počítač, na ktorom
ste. Na macOS a Linuxe je to `curl -fsSL http://<address>/setup/trust.sh | sh`,
na Windows `irm http://<address>/setup/trust.ps1 | iex`. Skript servíruje
samotné zariadenie ako čistý text, takže si odkaz najprv otvorte v karte a prečítajte
si ho. Skript pridá ten jeden certifikát do úložísk, ktoré čítajú vaše
prehliadače, iba pre vášho používateľa: do prihlasovacej kľúčenky (login
keychain) na macOS, do používateľského úložiska Trusted Root na Windows a do
úložísk NSS, ktoré na Linuxe používajú Chrome a Firefox. Nič iné neinštaluje,
nikdy nežiada práva správcu a vypíše odtlačok SHA-256 certifikátu, aby ste ho
mohli porovnať s tým na stránke. Telefóny dostanú namiesto toho obyčajné
stiahnutie. Ručná cesta zostáva pod tým: stiahnite `losos-ca.crt` a pridajte
ho sami ako dôveryhodnú autoritu.

Ak je strašidelnou časťou čítanie obrazovky a písanie adresy, otvorte radšej
[losos-edge.dasmat.us/find](https://losos-edge.dasmat.us/find) v Chrome a
stlačte **Find my box** (Nájsť moje zariadenie). Chrome sa raz opýta, či stránka smie
hľadať zariadenia vo vašej lokálnej sieti (jeho povolenie *Local Network
Access*, Chrome 142 alebo novší). Povoľte to a stránka nájde zariadenie podľa mena a
nasmeruje vás na jeho nastavenie. Vyhľadávanie beží vo vašom prehliadači,
medzi vaším počítačom a zariadením. Stránka číta `/setup/state.json` zariadenia, ktorý
obsahuje jeho meno a odtlačok certifikátu a nič viac, a nič o vašej sieti
neopustí váš počítač. Zariadenie dovolí čítať tento dokument iba tejto jednej stránke
(`losos.setup.finderOrigins`). Firefox a Safari také povolenie nemajú a
dostanú namiesto toho návod s písaním adresy.

## Vyskúšajte to vo VM (BIOS) {#try-it-in-a-vm-bios}

Pre cestu s TPM potrebuje VM emulovaný čip pri inštalácii *aj* pri každom
štarte, s tým istým adresárom stavu, inak nainštalovaný systém nenájde kľúč,
ktorý zapečatil. So swtpm:

```sh
mkdir -p tpm
swtpm socket --tpmstate dir=tpm --ctrl type=unixio,path=tpm/sock --tpm2 --daemon
qemu-system-x86_64 ... \
  -chardev socket,id=chrtpm,path=tpm/sock -tpmdev emulator,id=tpm0,chardev=chrtpm \
  -device tpm-tis,tpmdev=tpm0
```

Pridajte tieto tri riadky do spustenia inštalátora aj do neskorších štartov.
Bez nich inštalátor napíše `unlock: keyfile in the initrd` a zariadenie funguje, v
režime so súborom s kľúčom.

```sh
qemu-img create -f raw disk.img 40G
qemu-system-x86_64 -m 4096 -smp 2 -enable-kvm -machine q35 \
  -drive file=losos.iso,media=cdrom,readonly=on \
  -drive file=disk.img,format=raw,if=virtio \
  -nic user,hostfwd=tcp::8080-:80
```

Bez `-bios` QEMU nabootuje SeaBIOS, takže inštalátor zvolí GRUB. Keď skončí,
VM vypnite a spustite znova bez riadku `-drive …media=cdrom`. Adresa VM je z
hostiteľa dosiahnuteľná iba cez presmerovanie portu, takže otvorte
`http://localhost:8080`.

## ISO s celým closure

```sh
nix build .#losos-disk-iso          # or: devenv shell build-media iso
```

Bežné ISO necháva cieľový počítač stiahnuť a zostaviť celý systém, približne
5.8 GiB. Toto ISO nesie zostavený closure, takže `nixos-install` ho skopíruje
z kľúča.

- Stále potrebuje sieť na naklonovanie flaku a stiahnutie nixpkgs.
- Zostavte ho z toho istého commitu, ktorý bude inštalátor klonovať
  (`LOSOS_FLAKE_URL`, `--depth 1`). Inak sa cesty v store líšia a kópie
  zostanú nevyužité.
- Je príliš veľké na asset vydania na GitHube. Zostavte si ho sami.

## Demo obraz pre VM

```sh
nix build .#losos-disk-qcow2        # or: devenv shell build-media qcow2
```

Predinštalovaný QCOW2 pre QEMU alebo virt-manager. Označené vydania ho
publikujú na GHCR. **Nemá šifrovanie disku** a nikdy nespúšťa inštalátor,
takže ho používajte iba na demá a vývoj.

## Zväčšenie disku

Logický zväzok používa 90 % skupiny zväzkov (`losos.storage.fillPercent`).
Ak chcete využiť zvyšok:

```sh
losos-ctl grow
```

Toto spustí `lvextend`, `cryptsetup resize` a `resize2fs`, v tomto poradí, s
pripojeným `/persist`. `/persist` obsahuje `/nix`, takže sa časom zapĺňa.

Keď sa rezerva minie, pridajte disk a zväčšite znova:

```sh
pvcreate /dev/sdX && vgextend persist-vg /dev/sdX && losos-ctl grow
```

`grow` zlyhá, ak už nie je čo zabrať.
