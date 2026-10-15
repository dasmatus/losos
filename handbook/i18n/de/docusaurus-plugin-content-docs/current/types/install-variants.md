---
title: Installationsvarianten
sidebar_position: 4
---

# Installationsvarianten

Der Installer trifft zwei Entscheidungen, und du triffst eine. Er hält alle
drei in einer Datei auf der Box fest, `modules/install-target.nix`. Eine
Neuinstallation vom selben Medium wiederholt sie, und ein nächtliches Update
macht sie nie rückgängig.

## Wie die Festplatte entsperrt wird \{#how-the-disk-is-unlocked}

Jede Box hat eine verschlüsselte Festplatte. Der Installer formatiert sie
ohne Aufsicht, mit einem zufälligen Schlüssel, den er erzeugt. Die Varianten
unterscheiden sich darin, wo dieser Schlüssel beim Start liegt.

| Variante                   | Wann                                                     | Was das bedeutet                                                                                                                      |
| -------------------------- | -------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| **TPM** (der Standard)     | Der Installer hat einen TPM-2.0-Chip gefunden             | Der Schlüssel ist im Chip versiegelt. Die Boot-Partition enthält kein Geheimnis. Die Festplatte allein, ausgebaut oder geklont, ist unlesbar. Die Box startet trotzdem ohne Aufsicht. |
| **Schlüsseldatei**         | Kein Chip, oder `losos-ctl install --no-tpm`              | Der Schlüssel liegt auf der unverschlüsselten Boot-Partition, damit die Box ohne Aufsicht starten kann. Wer die Festplatte mitnimmt, hat die Daten. Für eine VM oder eine Testbox in Ordnung. |

Der Installer gibt aus, welche er gewählt hat: `unlock: TPM` oder `unlock: keyfile
in the initrd`. Der TPM-Schlüssel ist mit Absicht **nicht** an Messwerte der
Firmware gebunden. Die Box aktualisiert Firmware, Bootloader und Kernel
selbst und hat keine Shell, um sich aus einer Aussperrung zu befreien. Der
Preis dafür: Ein Dieb, der die ganze Box samt Chip mitnimmt, kann sie
trotzdem starten. Das [Sicherheitsmodell](../reference/security-model) sagt
das offen. [TPM und der Festplattenschlüssel](../reference/tpm) erklärt,
worauf eine Box ohne Chip verzichtet und wie du sie auf ein TPM umstellst.

![Nachweis aus einer Installation in einer virtuellen Maschine mit emuliertem TPM: Das verschlüsselte Volume hat ein systemd-tpm2-Token in Schlüsselplatz 1, und die Schlüsseldatei bleibt nur auf dem verschlüsselten Volume.](@site/docs/img/install-done-tpm.png)

Eine Box fragt beim Start nie nach einer Passphrase. Tut deine es doch,
stimmt etwas nicht; siehe
[Hängt an einer Passphrase-Abfrage](../troubleshooting/stuck-at-passphrase).

## UEFI oder BIOS \{#uefi-or-bios}

Das ist die einzige Frage, die der Installer stellt. **UEFI** bekommt
systemd-boot. **BIOS** bekommt GRUB und eine 1 MiB große Boot-Partition auf
der ersten Festplatte. **Autodetect** wählt den Modus, in dem der Stick
gestartet wurde, und das Menü nimmt ihn nach 30 Sekunden selbst, sodass ein
Start ohne Aufsicht trotzdem installiert. UEFI setzt voraus, dass der Stick
selbst im UEFI-Modus gestartet wurde. Auf einem im BIOS-Modus gestarteten
Stick lehnt der Installer UEFI ab, bevor er irgendeine Festplatte anfasst.
BIOS ist vor allem für virtuelle Maschinen gedacht. Unter UEFI ist der Stick
signiert, und eine Firmware, in der das LosOS-Zertifikat hinterlegt ist,
prüft ihn. Der eigene Bootloader der installierten Box ist nicht signiert,
deshalb braucht die Box ausgeschaltetes Secure Boot. Siehe
[Secure Boot und signierte Medien](../start/secure-boot).

![Das Firmware-Menü des Installers unter dem Lachs: 1 BIOS, 2 UEFI, 3 Autodetect, das nach 30 Sekunden von selbst startet.](@site/docs/img/installer-screen-menu.png)

## Die drei Medien \{#the-three-media}

| Medium                       | Größe     | Verwendung                                                                                   |
| ---------------------------- | --------- | -------------------------------------------------------------------------------------------- |
| **Installer-ISO**            | klein     | Das normale, an jedes Release angehängt. Das Zielsystem lädt das System herunter, das es installiert. |
| **ISO mit dem vollständigen System** | groß | Trägt das gebaute System, sodass die Installation kopiert statt herunterlädt. Du baust es selbst. |
| **Demo-Festplatten-Image** (QCOW2) | groß | Eine vorinstallierte virtuelle Festplatte für QEMU oder virt-manager. Keine Verschlüsselung, kein Installer. Für Vorführungen. |

## In den Rest der Festplatte wachsen \{#growing-into-the-rest-of-the-disk}

Der Installer lässt mit Absicht 10 % der Festplatte ungenutzt. Die Box kann
später in diese Reserve hineinwachsen, ohne dass jemand das Gehäuse öffnet.
Die Schaltfläche **Reserve nutzen** im Bereich Speicher belegt sie; siehe
[Festplatte vergrößern](../manual/disk-growth).

![Die Festplatte von außen nach innen: eine Volume Group mit dem logischen Volume, darin die Verschlüsselungsschicht und darin das Dateisystem, daneben eine Reserve von 10 %.](@site/docs/img/disk-growth-de.svg)
