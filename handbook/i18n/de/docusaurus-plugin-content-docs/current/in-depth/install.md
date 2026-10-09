---
title: Installation im Detail
sidebar_position: 1
mdx:
  format: md
---

# Installation im Detail

## Den Installer besorgen

Lade das ISO und seine `.sha256` aus dem
[neuesten Release](https://github.com/dasmatus/losos/releases/latest) herunter, oder bau es:

```sh
nix build .#nixosConfigurations.iso.config.system.build.isoImage
```

Schreib es auf einen USB-Stick und boote den Zielrechner davon.
Die Datenträgerbezeichnung des Sticks ist `LOSOS_INSTALLER`. Das BIOS-Bootmenü,
der UEFI-Startbildschirm und das Konsolenbanner zeigen LosOS und den
Release-Tag, und der Hostname ist `losos-installer`. Darunter steckt NixOS, und
os-release sagt das auch mit `ID_LIKE=nixos`. Die installierte Box trägt
denselben Namen und Tag in ihrem Bootmenü, ihren Konsolenbannern und in
os-release (`NAME=LosOS`, `IMAGE_VERSION`) und behält `ID=nixos`.

## Bevor du bootest

- **Secure Boot: aus, oder das LosOS-Zertifikat einspielen.** Release-ISOs
  sind signiert. Der UEFI-Loader des Sticks ist ein einziges Unified Kernel
  Image (Kernel, initrd, Kommandozeile), signiert mit dem db-Zertifikat von
  LosOS, und dieser Loader prüft den Hash des Systemabbilds, bevor er es
  einhängt. Eine Firmware, die nur den Schlüsseln von Microsoft vertraut,
  lehnt den Stick ab, bis das Zertifikat aus `EFI/losos/` auf dem Stick oder
  von der Release-Seite in ihre `db` aufgenommen wurde, neben den
  Microsoft-Zertifikaten, nie an ihrer Stelle: Windows, der shim anderer
  Distributionen und die Option-ROMs von Grafikkarten brauchen diese. OVMF
  zeigt "Access Denied"; andere Firmware überspringt den Stick womöglich
  kommentarlos.
  Der Loader der installierten Box ist nicht signiert, die Box selbst braucht
  also abgeschaltetes Secure Boot. Die Seite *Secure Boot and signed media*
  im Handbuch enthält die Schritte und die Prüfung von `SHA256SUMS.sig`.
  `tests/secure-boot.nix` ist der Nachweis.
- **UEFI oder BIOS, beides funktioniert.** Auf dem Installer-ISO lässt dich
  ein Terminalmenü BIOS, UEFI oder automatische Erkennung (die Firmware, die
  das ISO gebootet hat) wählen. UEFI bekommt systemd-boot. Legacy-BIOS bekommt
  GRUB plus eine 1 MiB große BIOS-Boot-Partition auf der ersten Festplatte.
  Ohne Antwort innerhalb von 30 Sekunden wählt das Menü die automatische
  Erkennung, sodass auch ein unbeaufsichtigter Boot installiert.
  `losos-install --bios` und `--uefi` wählen einen Modus, ohne das Menü
  anzuzeigen. UEFI, ob aus dem Menü oder per `--uefi`, setzt voraus, dass der
  Stick selbst im UEFI-Modus gebootet wurde, weil systemd-boot den
  Booteintrag der Firmware schreibt. Von einem per BIOS gebooteten Stick
  lehnt der Installer das ab, bevor er irgendeine Festplatte anfasst. BIOS
  ist hauptsächlich zum Testen in einer VM gedacht.
  Das Menü gibt es nur auf dem Installer-ISO. Die installierte Appliance
  zeigt es nie.
- **Netzwerk anschließen.** Der Installer klont den Flake zur Laufzeit.

## Was der Installer tut

Nach der Firmware-Wahl läuft der Installer unbeaufsichtigt. Er findet jede
fest eingebaute Festplatte, fasst alle in einer LVM-Volume-Group zusammen,
verschlüsselt sie mit LUKS, formatiert `/persist` als ext4 und installiert.

- Auf einem Rechner mit TPM-2.0-Chip versiegelt der Installer den
  Festplattenschlüssel direkt nach dem Formatieren im Chip
  (`systemd-cryptenroll`). Die meisten Mini-PCs haben einen in der Firmware,
  Intel PTT oder AMD fTPM, meist eingeschaltet. Die Box bootet dann ab dem
  ersten Start unbeaufsichtigt, und auf der Boot-Partition liegt nichts
  Geheimes. Die Festplatte allein, ausgebaut oder geklont, ist unlesbar. Der
  Schlüssel ist nicht an Firmware-Messungen (PCRs) gebunden, weil die Box
  ihre Firmware und ihren Bootloader unbeaufsichtigt aktualisiert und keine
  Shell hat, um sich aus einer Aussperrung zu befreien. Der Preis ist, dass
  ein Dieb, der die ganze Box samt Chip mitnimmt, trotzdem an die Daten
  kommt. Derselbe Zufallsschlüssel, mit dem das Volume formatiert wurde,
  bleibt im verschlüsselten Volume unter `/etc/keys/persist-keyfile` als
  Wiederherstellungs-Slot.
- Ohne Chip, oder mit `losos-ctl install --no-tpm` aus der Shell des
  Installers, wird diese Schlüsseldatei stattdessen in die initrd
  eingebacken. Sie liegt dann auf der unverschlüsselten ESP, und jeder, der
  die Festplatte mitnimmt, kann die Daten lesen. Der Installer gibt aus,
  welche der beiden Varianten er gewählt hat. `--tpm` macht einen fehlenden
  Chip zu einem Fehler statt zu einer stillen Installation mit
  Schlüsseldatei.
  [TPM und der Festplattenschlüssel](/reference/tpm.md) erklärt, wofür der Chip da ist
  und worauf eine Box ohne ihn verzichtet.
- Gib einer VM ein TPM (swtpm, siehe unten), sonst installiert sie im
  Schlüsseldatei-Modus.

`/persist` ist ext4 mit dem Feature `encrypt`, weil fscrypt es braucht und
btrfs es nicht unterstützt. Du verzichtest auf Kompression und
Prüfsummen der Daten.

Nach dem Neustart zeigt der Bildschirm den Lachs, während die Box startet.
Sobald sie läuft, wandert der Lachs nach oben, und ein Feld darunter zeigt
die IP-Adresse der Box und `<hostname>.local`. Auf einem Rechner, auf dem
der Startbildschirm nicht rechtzeitig eine Anzeige findet, zeigt tty1
dieselben Zeilen als Textbanner. Öffne die IP-Adresse in einem
Browser auf einem beliebigen Rechner im selben Netzwerk. Der `.local`-Name
funktioniert ebenfalls überall, wo der Rechner mDNS-Namen auflöst. Windows,
macOS, Smartphones und die meisten Linux-Desktops tun das; der Host eines
libvirt- oder VirtualBox-Gasts hinter NAT meist nicht, verwende dort
also die Adresse. Beide führen zu denselben Seiten. Das Feld aktualisiert
sich, wenn sich die Adresse ändert.

Die erste Seite ist der Einrichtungsassistent. Sein erster Schritt ist, dem
eigenen Zertifikat der Box zu vertrauen, damit der Rest der Einrichtung und
jede spätere Anmeldung über HTTPS laufen und dein Browser einen Passkey
anbieten kann. Der Schritt zeigt eine Zeile zum Einfügen in ein Terminal,
passend zu dem Rechner, an dem du sitzt. Auf macOS und Linux ist das
`curl -fsSL http://<address>/setup/trust.sh | sh`, auf Windows
`irm http://<address>/setup/trust.ps1 | iex`. Die Box selbst liefert das
Skript als Klartext aus, öffne den Link also zuerst in einem Tab, um es
zu lesen. Das Skript fügt das eine Zertifikat den Speichern hinzu, die deine
Browser lesen, nur für deinen Benutzer: dem Anmeldeschlüsselbund (login
keychain) auf macOS, dem Trusted-Root-Speicher des Benutzers auf Windows, den
NSS-Speichern, die Chrome und Firefox auf Linux verwenden. Es installiert
nichts anderes, fragt nie nach Administratorrechten und gibt den
SHA-256-Fingerabdruck des Zertifikats aus, damit du ihn mit dem auf der Seite
vergleichen kannst. Smartphones bekommen stattdessen den einfachen Download.
Der manuelle Weg bleibt darunter: Lade `losos-ca.crt` herunter und füge
es selbst als vertrauenswürdige Zertifizierungsstelle hinzu.

Wenn das Ablesen eines Bildschirms und das Eintippen einer Adresse der
abschreckende Teil ist, öffne stattdessen
[losos-edge.dasmat.us/find](https://losos-edge.dasmat.us/find) in Chrome und
klicke auf **Find my box** (Meine Box finden). Chrome fragt einmal, ob die
Seite nach Geräten in deinem lokalen Netzwerk suchen darf (seine Berechtigung
*Local Network Access*, Chrome 142 oder neuer). Stimm zu, und die Seite
findet die Box über ihren Namen und verlinkt dich zu ihrer Einrichtung. Die
Suche läuft in deinem Browser, zwischen deinem Rechner und der Box. Die Seite
liest `/setup/state.json` der Box, das ihren Namen und den Fingerabdruck
ihres Zertifikats enthält und sonst nichts, und nichts über dein Netzwerk
verlässt deinen Rechner. Die Box lässt nur diese eine Seite das Dokument lesen
(`losos.setup.finderOrigins`). Firefox und Safari haben keine solche
Berechtigung und bekommen stattdessen die Anleitung mit eingetippter Adresse.

## In einer VM ausprobieren (BIOS) {#try-it-in-a-vm-bios}

Für den TPM-Weg braucht die VM einen emulierten Chip bei der Installation
*und* bei jedem Start, mit demselben Zustandsverzeichnis, sonst findet das
installierte System den Schlüssel nicht, den es versiegelt hat. Mit swtpm:

```sh
mkdir -p tpm
swtpm socket --tpmstate dir=tpm --ctrl type=unixio,path=tpm/sock --tpm2 --daemon
qemu-system-x86_64 ... \
  -chardev socket,id=chrtpm,path=tpm/sock -tpmdev emulator,id=tpm0,chardev=chrtpm \
  -device tpm-tis,tpmdev=tpm0
```

Füge diese drei Zeilen sowohl beim Lauf des Installers als auch bei den
späteren Starts hinzu. Ohne sie meldet der Installer
`unlock: keyfile in the initrd`, und die Box funktioniert im
Schlüsseldatei-Modus.

```sh
qemu-img create -f raw disk.img 40G
qemu-system-x86_64 -m 4096 -smp 2 -enable-kvm -machine q35 \
  -drive file=losos.iso,media=cdrom,readonly=on \
  -drive file=disk.img,format=raw,if=virtio \
  -nic user,hostfwd=tcp::8080-:80
```

Ohne `-bios` bootet QEMU SeaBIOS, also wählt der Installer GRUB. Sobald er
fertig ist, fahr die VM herunter und starte sie erneut ohne die
Zeile `-drive …media=cdrom`. Die Adresse der VM ist vom Host aus nur über die
Portweiterleitung erreichbar, öffne also `http://localhost:8080`.

## ISO mit der vollständigen Closure

```sh
nix build .#losos-disk-iso          # or: devenv shell build-media iso
```

Beim normalen ISO lädt und baut der Zielrechner das ganze System, etwa
5.8 GiB. Dieses ISO bringt die gebaute Closure mit, sodass `nixos-install` sie
vom Stick kopiert.

- Es braucht trotzdem ein Netzwerk, um den Flake zu klonen und nixpkgs zu
  holen.
- Bau es aus demselben Commit, den der Installer klonen wird
  (`LOSOS_FLAKE_URL`, `--depth 1`). Sonst unterscheiden sich die Store-Pfade
  und die Kopien bleiben ungenutzt.
- Es ist zu groß für ein Release-Asset auf GitHub. Bau es selbst.

## Demo-Image für eine VM

```sh
nix build .#losos-disk-qcow2        # or: devenv shell build-media qcow2
```

Ein vorinstalliertes QCOW2 für QEMU oder virt-manager. Getaggte Releases
veröffentlichen es auf GHCR. Es hat **keine Festplattenverschlüsselung** und
führt nie den Installer aus, verwende es also nur für Demos und
Entwicklung.

## Die Festplatte vergrößern

Das Logical Volume belegt 90 % der Volume Group (`losos.storage.fillPercent`).
Um den Rest zu nutzen:

```sh
losos-ctl grow
```

Das führt `lvextend`, `cryptsetup resize` und `resize2fs` aus, in dieser
Reihenfolge, bei eingehängtem `/persist`. `/persist` enthält `/nix` und füllt
sich daher mit der Zeit.

Wenn die Reserve aufgebraucht ist, füge eine Festplatte hinzu und
vergrößere erneut:

```sh
pvcreate /dev/sdX && vgextend persist-vg /dev/sdX && losos-ctl grow
```

`grow` schlägt fehl, wenn nichts mehr zu holen ist.
