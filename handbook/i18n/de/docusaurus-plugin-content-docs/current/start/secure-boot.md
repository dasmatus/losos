---
title: Secure Boot und signierte Medien
sidebar_position: 5
---

# Secure Boot und signierte Medien

Das Installer-ISO jedes Release ist signiert, und ein Rechner mit
eingeschaltetem Secure Boot kann es prüfen, bevor auch nur ein Byte von
LosOS läuft. Diese Seite sagt, was die Firmware prüft, wie du deine Firmware
LosOS vertrauen lässt und wie du einen Download von Hand prüfst.

## Was geprüft wird und von wem \{#what-is-verified-and-by-whom}

Mit eingeschaltetem Secure Boot prüft eine UEFI-Firmware genau eine Sache:
das Programm, das sie gleich startet, gegen die Zertifikate in ihrer Liste
`db`. LosOS sorgt dafür, dass diese eine Prüfung den ganzen Start abdeckt:

1. **Die Firmware prüft den Bootloader.** Der UEFI-Bootloader des Sticks,
   `EFI/BOOT/BOOTX64.EFI`, ist ein einzelnes *Unified Kernel Image*. Es
   enthält den Linux-Kernel, die initiale Ramdisk und die
   Kernel-Kommandozeile in einer Datei, signiert mit dem LosOS-Zertifikat.
   Ein geändertes Byte in einem der drei Teile bricht die Signatur, und die
   Firmware weigert sich, es zu starten. Vor dieser Prüfung läuft nichts vom
   Stick.
2. **Der Bootloader prüft das System.** Der Rest des Sticks ist ein
   System-Image von 1,5 GB, `nix-store.squashfs`. Sein SHA-256-Hash ist Teil
   der signierten Kommandozeile, und die initiale Ramdisk berechnet den Hash
   des Images auf dem Stick, bevor sie es einhängt. Ein verändertes Image
   lehnt sie ab, zeigt *THE MEDIUM HAS BEEN ALTERED* auf dem Bildschirm und
   schaltet den Rechner aus.

![Ein Stick, dessen System-Image nach dem Signieren verändert wurde: Der Bootloader lehnt ihn mit THE MEDIUM HAS BEEN ALTERED ab und gibt den erwarteten und den gefundenen Hash aus.](@site/docs/img/secure-boot-refused-tampered.png)

Der Installer sagt dann zusammen mit seinem Firmware-Menü, was passiert ist:

```
Secure Boot: enabled. The firmware verified this medium's signature.
```

oder `Secure Boot: disabled ...`, wenn die Firmware nichts geprüft hat, oder
`Secure Boot: not available (legacy BIOS boot)`. Secure Boot ist eine
UEFI-Funktion. Einen im BIOS-Modus gestarteten Stick prüft nichts, genau wie
bei jedem anderen BIOS-Start.

![Der erste Bildschirm des Installers auf einem signierten Stick: Secure Boot eingeschaltet, die Firmware hat die Signatur dieses Mediums geprüft.](@site/docs/img/installer-menu.png)

:::warning[Die installierte Box braucht Secure Boot aus]
**Nicht** abgedeckt ist das installierte System. Die Box startet mit
systemd-boot von ihrer eigenen Festplatte. Weder dieser Bootloader noch die
Kernel, die ihre nächtlichen Neubauten installieren, sind signiert, also
braucht die Box selbst Secure Boot **ausgeschaltet**, um zu starten. Heute
sieht der Ablauf so aus: Zertifikat eintragen, den Stick mit eingeschaltetem
Secure Boot starten, damit die Firmware das Medium prüft, installieren, dann
Secure Boot vor dem ersten Start von der Festplatte ausschalten. Oder lass
es die ganze Zeit aus und prüfe stattdessen den Download von Hand, wie unten
beschrieben. Die Bootkette des installierten Systems zu signieren, würde
einen Signaturschlüssel auf der Box für jeden Neubau erfordern. Das ist ein
eigener Entwurf und nicht Teil dieses.
:::

## Die Firmware LosOS vertrauen lassen \{#letting-your-firmware-trust-losos}

Ein PC vertraut ab Werk den Zertifikaten von Microsoft und sonst nichts,
also lehnt er bei eingeschaltetem Secure Boot den LosOS-Stick ab. OVMF und
die meisten Firmwares geben *Access Denied* aus, manche überspringen den
Stick ohne Meldung. Diese Ablehnung bedeutet, dass die Signaturprüfung
funktioniert. Um den Stick mit eingeschaltetem Secure Boot zu starten, füge
das LosOS-Zertifikat zur `db` der Firmware hinzu, neben das von Microsoft:

![Eine Firmware, die nur den Schlüsseln von Microsoft vertraut, lehnt den LosOS-Stick ab: Access Denied, wahrscheinlich von Secure Boot abgewiesen.](@site/docs/img/secure-boot-refused-microsoft-keys.png)

1. Besorg dir das Zertifikat. Es liegt auf dem Stick selbst unter
   `EFI/losos/losos-secure-boot.cer` im DER-Format und als `.pem`, und in
   jedem [Release](https://github.com/dasmatus/losos/releases) als
   `losos-secure-boot-db.cer`. Die Release Notes geben seinen
   SHA-256-Fingerabdruck an, und `openssl x509 -in losos-secure-boot-db.pem -noout
   -fingerprint -sha256` gibt denselben für die Datei aus, die du hast.
2. Stell in den Secure-Boot-Einstellungen der Firmware vom Modus *Standard*
   auf *Custom* um. Wähl *Append to db*, *Enroll key* oder *Add signature
   from file* und such die `.cer` auf der EFI-Partition des Sticks aus. Die
   Bezeichnungen sind je nach Hersteller verschieden. Die EFI-Partition des
   Sticks ist ein einfaches FAT-Volume, in dem jede Firmware blättern kann.
3. Starte vom Stick. Die erste Zeile des Installers lautet *Secure Boot:
   enabled*.

### Behalte die Zertifikate von Microsoft \{#keep-microsofts-certificates}

Füge LosOS zur Liste hinzu. Ersetze die Liste nicht durch LosOS. Drei Dinge
auf einem normalen PC sind unter den Zertifikaten von Microsoft signiert und
starten nicht mehr, sobald diese Zertifikate aus der `db` verschwinden:

- Der eigene Bootmanager von Windows.
- Andere Linux-Distributionen, deren Bootloader der ersten Stufe (shim)
  Microsoft signiert.
- Die Firmware-Treiber auf Grafik- und Netzwerkkarten, sogenannte Option
  ROMs. Auf einem Desktop mit eigener Grafikkarte kann ihr Verlust bedeuten,
  dass du überhaupt kein Bild bekommst, auch nicht im Setup der Firmware.

:::warning[Die Schlüsselliste nicht leeren]
Meide *Delete all keys*, *Clear Secure Boot keys* und *Reset to Setup
Mode*, außer der nächste Schritt ist *Restore factory keys*. Wenn deine
Firmware einen Schlüssel nur hinzufügen kann, nachdem sie alle gelöscht hat,
stell zuerst die Werksschlüssel wieder her und füge dann das
LosOS-Zertifikat hinzu.
:::

Aus einem laufenden Linux im Modus *Setup* fügt `efi-updatevar -a -c
losos-secure-boot-db.pem db` aus efitools das Zertifikat hinzu. Gib sbctl
`--microsoft` mit: `sbctl enroll-keys --microsoft` behält die Zertifikate von
Microsoft neben denen, die sbctl einträgt, und ohne den Schalter wirft es sie
hinaus. Füge dann das LosOS-Zertifikat zur db von sbctl hinzu.

Die Testsuite prüft genau diesen Zustand. Ihre Firmware enthält das
LosOS-Zertifikat neben den fünf `db`-Zertifikaten und zwei `KEK`-Zertifikaten
von Microsoft. Der LosOS-Stick startet, und der von Microsoft signierte shim
von Ubuntu auch. Nimm die Zertifikate von Microsoft weg, und derselbe shim
wird abgelehnt:

![Die Firmware mit eingetragenen Zertifikaten von LosOS und Microsoft: Aus dem gestarteten LosOS-Stick heraus führt KEK den LosOS-Test-KEK und die beiden KEK-CAs von Microsoft auf, und db das LosOS-Testzertifikat und die fünf CAs von Microsoft.](@site/docs/img/secure-boot-cosigned-db.png)

Das Eintragen ist eine einmalige Änderung an diesem Rechner.

:::note[Noch kein von Microsoft signierter shim]
Der LosOS-Bootloader ist von LosOS signiert, nicht von Microsoft. Ein Stick,
der auf einem PC im Auslieferungszustand ohne eingetragenes Zertifikat
startet, bräuchte einen shim, den Microsoft für LosOS signiert hat, nach dem
öffentlichen shim-Review und dem Signaturverfahren von Microsoft, die LosOS
nicht durchlaufen hat. Bis dahin trägt jeder Rechner das LosOS-Zertifikat
einmal ein.
:::

## Einen Download von Hand prüfen \{#verifying-a-download-by-hand}

Jedes Release enthält `SHA256SUMS` und eine abgetrennte Signatur,
`SHA256SUMS.sig`, erstellt mit demselben Zertifikat, das das Medium signiert:

```sh
R=https://github.com/dasmatus/losos/releases/download/<tag>
curl -fsSLO $R/SHA256SUMS $R/SHA256SUMS.sig $R/losos-secure-boot-db.pem
openssl dgst -sha256 -verify <(openssl x509 -pubkey -noout -in losos-secure-boot-db.pem) \
  -signature SHA256SUMS.sig SHA256SUMS
sha256sum -c --ignore-missing SHA256SUMS
```

Der erste Befehl gibt `Verified OK` aus. Der zweite prüft die Dateien des
Release, die du heruntergeladen hast. Um die Signatur des Bootloaders in
einem ISO zu prüfen, das du schon hast, lass das Werkzeug des Repositorys
direkt darauf laufen:

```sh
nix run github:dasmatus/losos#losos-sign-iso -- verify --cert losos-secure-boot-db.pem losos-installer-<tag>.iso
```

Vergleiche den Fingerabdruck des Zertifikats mit dem in den Release Notes und
mit `nix/keys/secure-boot-db.pem` im Repository. Alle drei sind dieselbe Datei.

## Wo der Schlüssel liegt \{#where-the-key-lives}

Der private Schlüssel kommt nie in einen Nix-Build oder auf eine Box. Der
Projektinhaber erzeugt ihn einmal auf seinem eigenen Rechner mit
`docs/provisioning/secure-boot/keygen.sh` und bewahrt ihn offline auf. Die einzige
andere Kopie ist das Repository-Secret, mit dem CI signiert. Das Zertifikat
liegt im Repository als `nix/keys/secure-boot-db.pem`. Solange diese Datei keins
enthält, steht bei Releases *Not signed*, und der Stick braucht
ausgeschaltetes Secure Boot. Die Testsuite prüft den Mechanismus mit einem
Wegwerfschlüssel, den sie erzeugt und wieder verwirft.
`nix/tests/secure-boot.nix` startet das signierte Medium unter dem
Secure-Boot-Build von OVMF und zeigt von innen das eigene Flag *enabled* der
Firmware. Die Firmware in diesem Test enthält die Zertifikate von Microsoft
neben dem Testschlüssel, und ein von Microsoft signierter shim startet
darauf weiterhin. Der Test sieht außerdem zu, wie die Firmware dasselbe
Medium unter reinen Microsoft-Schlüsseln ablehnt, ebenso eine manipulierte
Kopie und den unsignierten Build, und wie sie den shim ablehnt, sobald die
Zertifikate von Microsoft weg sind.
