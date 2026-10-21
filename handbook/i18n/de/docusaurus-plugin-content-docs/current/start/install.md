---
title: Installation
sidebar_position: 2
---

# Installation

Für die Installation brauchst du einen USB-Stick, ein Netzwerkkabel und etwa
eine halbe Stunde, die du größtenteils mit Warten verbringst. Der Installer
stellt eine Frage und läuft sonst von allein. **Er löscht jede fest
eingebaute Festplatte im Rechner.**

## Was du brauchst \{#what-you-need}

- Einen Rechner mit einem 64-Bit-Prozessor von Intel oder AMD (x86_64) und
  UEFI- oder BIOS-Firmware, mindestens 4 GiB Arbeitsspeicher und einer
  Festplatte mit 40 GiB oder mehr. Widgets mit KI bauen braucht 8 GiB
  Arbeitsspeicher, siehe [Widget-Baukasten](/in-depth/widget-builder.md).
  Mini-PCs für Büros eignen sich gut. Die meisten haben einen TPM-2.0-Chip in
  der Firmware, Intel PTT oder AMD fTPM genannt, und der Installer bewahrt den
  Festplattenschlüssel darin auf. LosOS läuft nur auf x86_64. Auf einem
  ARM-Rechner kannst du es nicht installieren, insbesondere **nicht auf einem
  Mac mit Apple Silicon (M-Serie)**.
- Einen USB-Stick mit 2 GiB oder mehr.
- Eine kabelgebundene Netzwerkverbindung mit Internetzugang. Der Installer
  lädt das System herunter, das er installiert.
- Einen Bildschirm und eine Tastatur für den Rechner, nur für die
  Installation. Danach zeigt der Bildschirm ein Banner mit der Adresse der
  Box, und die Box braucht keins von beiden.

:::tip[Entscheide jetzt über den Terminal-Client]
Wenn du die Box später mit `losos` aus einem Terminal verwalten willst,
entscheide das jetzt. Die Box vertraut nur einem GPG-Schlüssel, der auf dem
Stick war, von dem sie installiert wurde, und der Release-Stick enthält
keinen. [Der Terminal-Client](/manual/terminal.md) zeigt, wie du einen Stick
mit deinem Schlüssel darauf baust.
:::

## Schritte \{#steps}

1. Lade das Installer-ISO und seine `.sha256` aus dem
   [neuesten Release](https://github.com/dasmatus/losos/releases)
   herunter und schreib es mit balenaEtcher, Rufus im DD-Modus oder `dd` auf
   den Stick.
2. **Schalte Secure Boot** in der Firmware des Rechners **aus**, oder füge
   das LosOS-Zertifikat neben dem von Microsoft hinzu, damit die Firmware den
   Stick prüfen kann. Lösche dafür nicht die Zertifikate von Microsoft.
   [Secure Boot und signierte Medien](secure-boot) zeigt, wie es geht. Ein
   neuer PC vertraut nur den Schlüsseln von Microsoft, also lehnt die
   Firmware den Stick bei eingeschaltetem Secure Boot und ohne eingetragenes
   Zertifikat ab, manchmal ohne Meldung. Die installierte Box braucht Secure
   Boot so oder so ausgeschaltet, weil ihr eigener Bootloader nicht signiert
   ist.
3. Starte vom Stick. Beim Hochfahren zeigt er das LosOS-Logo, auf einem
   BIOS-Rechner über dem Bootmenü und auf einem UEFI-Rechner allein. Danach
   bietet ein Menü unter dem Logo **BIOS**, **UEFI** oder **autodetect** auf
   den Tasten 1, 2 und 3 an und nimmt nach 30 Sekunden autodetect, damit auch
   ein unbeaufsichtigter Start installiert. Autodetect wählt den Modus, in dem
   der Stick selbst gestartet wurde, und das ist für fast alle richtig.
   [Installationsvarianten](../types/install-variants) erklärt die Wahl.
4. Warte. Der Installer findet jede fest eingebaute Festplatte, fasst sie zu
   einem verschlüsselten Volume zusammen, lädt das System herunter und
   installiert es, und der Bildschirm markiert jeden Schritt, während er
   läuft. Auf einem Rechner mit TPM-Chip versiegelt ein Schritt den
   Festplattenschlüssel darin; ohne Chip warnt der Bildschirm, dass der
   Schlüssel auf der Boot-Partition liegt.
   [Installationsvarianten](../types/install-variants#how-the-disk-is-unlocked)
   erklärt, was das bedeutet.
5. Wenn der Bildschirm meldet, dass LosOS installiert ist, zieh den Stick ab
   und drück eine beliebige Taste für den Neustart. Nach dem Neustart zeigt
   der Bildschirm den Lachs mit einem Banner darunter, das die Adresse der Box
   enthält. Öffne diese Adresse in einem Browser auf irgendeinem Rechner im
   selben Netzwerk und folge [Der erste Start](first-run).

![Das Menü des Installers unter dem Lachs: 1 für BIOS, 2 für UEFI oder 3 für autodetect, das nach 30 Sekunden von selbst startet.](@site/docs/img/installer-screen-menu.png)

![Die Schritte des Installers unter dem Lachs, der laufende markiert. Dieser Rechner hat noch kein Netzwerk, also sagt der Bildschirm, dass ein Kabel eingesteckt werden soll, und er hat keinen TPM-Chip, also sagt er, wohin der Festplattenschlüssel kommt.](@site/docs/img/installer-screen-steps.png)

![Das Ende einer Installation: LosOS ist installiert, Stick abziehen und eine beliebige Taste für den Neustart drücken.](@site/docs/img/installer-screen-done.png)

![Der Bildschirm der Box nach dem ersten Start: der Lachs und darunter die Meldung, dass LosOS v0.4.0 bereit ist, mit den Adressen der Box und ihrem .local-Namen.](@site/docs/img/tty1-banner.png)

## In einer virtuellen Maschine ausprobieren \{#trying-it-in-a-virtual-machine}

Die Box läuft in QEMU, virt-manager oder VirtualBox. Zwei Dinge solltest du
wissen:

- Gib der VM ein emuliertes TPM, `swtpm`, wenn du den Entsperrweg über das
  TPM willst. Ohne TPM installiert sich die Box im Keyfile-Modus, der
  ebenfalls funktioniert.
- Der VM-Host löst den `.local`-Namen der Box meist nicht auf. Nimm die
  IP-Adresse aus dem Banner.
  [Installation im Detail](/in-depth/install.md#try-it-in-a-vm-bios) enthält
  die genauen QEMU-Zeilen. Zum Vorführen der Box gibt es außerdem ein
  fertiges Demo-Festplatten-Image, `losos-disk-qcow2`, ohne Verschlüsselung
  und ohne Installer.

### Mit quickemu \{#with-quickemu}

Das Repository enthält eine Konfiguration für
[quickemu](https://github.com/quickemu-project/quickemu), `docs/vm/losos.conf`.
Sie gibt der VM 4 Kerne, 8 GiB Arbeitsspeicher und eine Festplatte mit
64 GiB, UEFI-Firmware mit ausgeschaltetem Secure Boot und ein emuliertes
TPM, sodass der Installer den Festplattenschlüssel so versiegelt wie auf
einem Mini-PC. Die Werte liegen über dem Minimum, weil die Box LosOS cloud
in ihrem eigenen Cluster betreibt und sich jede Nacht auf
dieselbe Festplatte neu baut.

1. Installiere quickemu und `swtpm`.
2. Lade das Installer-ISO aus einem Release herunter und speichere es als
   `docs/vm/losos/losos-installer.iso`.
3. Führe `quickemu --vm docs/vm/losos.conf` aus. Die Festplatte, die
   Firmware-Variablen und der TPM-Zustand liegen in `docs/vm/losos/`, sodass
   spätere Starts den versiegelten Schlüssel finden.
4. Wenn der Installer meldet, dass LosOS installiert ist, drück eine Taste
   für den Neustart. Der Installer setzt den Bootloader der Box an die erste
   Stelle der Bootreihenfolge der Firmware, also startet die VM von ihrer
   Festplatte. Spätere Läufe von quickemu lassen das ISO weg, weil die
   Festplatte nicht mehr leer ist.
5. Öffne `http://localhost:8080` auf dem Host. quickemu leitet Port 8080 an
   Port 80 der Box weiter und 8443 an 443.

:::note[quickemu braucht KVM]
quickemu braucht Hardware-Virtualisierung, unter Linux also KVM, und
startet auf einem Host ohne sie nicht.
:::
