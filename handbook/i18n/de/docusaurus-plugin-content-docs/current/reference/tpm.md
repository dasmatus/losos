---
title: TPM und der Festplattenschlüssel
sidebar_position: 4.5
slug: /reference/tpm
---

# TPM und der Festplattenschlüssel

Eine Box ohne TPM-Chip ist weniger sicher als eine mit. Diese Seite erklärt,
was ein TPM ist, wofür LosOS es verwendet und worauf eine Box ohne TPM
verzichtet.

## Was ein TPM ist \{#what-a-tpm-is}

Ein TPM (Trusted Platform Module) ist ein kleiner Sicherheitschip auf dem
Mainboard. Auf den meisten neueren Rechnern ist es statt eines eigenen Chips
Teil der Firmware des Prozessors. Intel nennt das PTT, AMD fTPM. Windows 11
setzt TPM 2.0 voraus, ein mit Windows 11 verkaufter Rechner hat also eines.
Ältere Mini-PCs haben oft ebenfalls eines, manchmal im Firmware-Setup
abgeschaltet.

Der Chip enthält einen eigenen Schlüssel, der ihn nie verlässt. Software
kann dem Chip ein Geheimnis zum Versiegeln übergeben, und nur derselbe Chip
kann es wieder entsiegeln. Ein versiegeltes Geheimnis, das auf einen anderen
Rechner kopiert oder von der Festplatte gelesen wird, ist dort nutzlos.

Genau das braucht eine Box ohne Aufsicht. Sie muss ihre verschlüsselte
Festplatte bei jedem Start entsperren, ohne dass jemand an der Tastatur eine
Passphrase eintippt. Der Schlüssel muss an einem Ort liegen, den die Box
selbst erreicht, und das TPM ist der einzige solche Ort, der nicht die
Festplatte selbst ist.

## Wofür LosOS es verwendet \{#what-losos-uses-it-for}

Das Datenvolume (`/persist`) ist auf jeder Box mit LUKS verschlüsselt. Der
Installer erzeugt einen zufälligen Schlüssel, formatiert das Volume damit und
versiegelt den Schlüssel anschließend mit `systemd-cryptenroll` im TPM. Beim
Start fragt die initrd den Chip nach dem Schlüssel und öffnet das Volume.
Die Boot-Partition enthält kein Geheimnis.

Die Box versiegelt außerdem kleinere Geheimnisse im Chip:

- Den Schlüssel, der das Speicherverzeichnis des Mesh verschlüsselt
  (`modules/fscrypt.nix`).
- Jedes Geheimnis in ihrem Speicher für Zugangsdaten: den Ersatzschlüssel,
  die Schlüssel von LosOS Login und die Schlüssel des Sicherungs-Buckets
  (siehe [Zugangsdaten](credentials.md)).

:::note[Die Wiederherstellungskopie des Schlüssels]
Eine Kopie des Festplattenschlüssels bleibt außerdem im verschlüsselten
Volume unter `/etc/keys/persist-keyfile`. `losos-ctl grow` verwendet sie, um
das Volume zu vergrößern. Von außen kann sie das Volume nicht öffnen, weil
sie darin liegt.
:::

## Worauf eine Box ohne TPM verzichtet \{#what-a-box-without-a-tpm-gives-up}

Der Installer verschlüsselt das Volume trotzdem auf dieselbe Weise. Was sich
ändert, ist der Ort, an dem der Schlüssel beim Start liegt. Ohne Chip legt
der Installer den Schlüssel als `/crypto_keyfile.bin` in die initrd. Die
initrd liegt auf der Boot-Partition (der ESP), und die Boot-Partition ist
nicht verschlüsselt. Der Schlüssel liegt direkt neben dem Schloss.

Das hat zwei Folgen.

1. Wer die Festplatte in die Hände bekommt, kann alles darauf lesen. Das
   gilt für eine aus der Box ausgebaute Festplatte, eine gestohlene Box, ein
   Festplatten-Image, eine zur Reparatur eingeschickte Box und eine alte
   Festplatte, die ohne Löschen verkauft oder weggeworfen wurde. In jedem
   Fall hängt der Leser die ESP ein, kopiert den Schlüssel aus der initrd und
   öffnet das Volume. Dafür braucht es
   gewöhnliche Linux-Werkzeuge und ein paar Minuten. Offengelegt werden die
   Dateien des Besitzers in LosOS Cloud, die Repositories in LosOS Git, der
   Admin-Schlüssel und der Rest von `/persist`.
2. Der Speicherschlüssel des Mesh verliert seinen Chip und fällt auf eine
   einfache Datei unter `/var/secrets/losos-shared-key` zurück, auf demselben
   Volume wie die Daten, die er schützt. Eine Kopie von `/persist`, die die
   Box verlässt, nimmt den Schlüssel mit, und
   ein Fehler, der einen Dienst beliebige Dateien als root lesen lässt, liest
   beide Hälften.

Die Netzwerkseite ändert sich nicht. Der LAN-Guard, die Anmeldung, der
Tunnel, die Härtung und die Trennung der beiden Konten funktionieren mit und
ohne Chip gleich.

## Wogegen ein TPM nicht schützt \{#what-a-tpm-does-not-protect-against}

LosOS versiegelt den Schlüssel, ohne ihn an Firmware-Messwerte (PCRs) zu
binden. Die Box aktualisiert ihren Bootloader und Kernel jede Nacht, ohne
dass jemand zusieht. Ein an diese Messwerte gebundener Schlüssel würde die
Box nach einem Update von ihrer eigenen Festplatte aussperren, und die Box
hat keine Shell, um sich davon zu erholen.

:::warning[Ein Dieb mit der ganzen Box]
Der Chip gibt den Schlüssel an alles heraus, was auf diesem Rechner
startet, auch an den USB-Stick eines Diebes. Ein TPM macht die Festplatte
allein unlesbar. Es hilft nicht mehr, sobald jemand die ganze Box hat, außer
die Box braucht zum Öffnen der Festplatte auch einen Server in ihrem eigenen
Netz (nächster Abschnitt). Das [Sicherheitsmodell](/reference/security-model.md)
führt das als bekannte Einschränkung.
:::

## Die Festplatte nur zu Hause öffnen \{#opening-the-disk-only-at-home}

Du kannst eine Box so einrichten, dass der Chip allein nicht reicht. Gib dem
Installer einen oder mehrere Tang-Server im eigenen Netz der Box:

```sh
losos-ctl install --tang 'http://192.168.1.1:7500#THUMBPRINT'
```

Tang ist ein kleiner Schlüsselserver. OpenWrt hat ein Paket `tang`, NixOS
hat `services.tang`. Den Fingerabdruck (Thumbprint) zeigt
`tang-show-keys 7500` auf dem Server. Der Installer prüft den Server daran,
vertraut also beim ersten Kontakt nichts.

Der Installer versiegelt den Festplattenschlüssel dann mit Clevis an das TPM
und den Tang-Server zusammen, statt nur an das TPM. Bei jedem Start holt
sich die Box eine Adresse im kabelgebundenen Netz, fragt den Server und
öffnet die Festplatte nur mit beiden Antworten. Der Server sieht den
Schlüssel nie. Er erfährt nur, dass die Box gestartet ist.

Was sich dadurch ändert:

- Eine Box, die aus ihrem Netz weggetragen wird, öffnet sich nicht, und
  eine Box, deren Server fehlt, auch nicht. Ein Dieb mit der ganzen Box
  bekommt eine Passwortabfrage.
- Die Box fragt den Server etwa eine halbe Minute lang, dann fragt der
  Bildschirm nach einem Passwort. Gib den Wiederherstellungscode der Box
  ein, den Einstellungen, Sicherung anzeigt. Die Box fügt den Code als
  Passwort hinzu, sobald sie einen hat, und tauscht ihn aus, wenn er sich
  ändert.
- Der Server muss antworten, wann immer die Box startet, auch beim
  nächtlichen Neustart um 00:07. Ist er dann aus, wartet die Box an der
  Abfrage, bis jemand den Code eingibt.
- Die Festplatte allein bleibt unlesbar wie auf jeder Box mit TPM.

Wie das TPM selbst steht die Einstellung in `modules/install-target.nix`
(`tpm.tang`). Ein- oder ausschalten lässt sie sich also nur durch eine
Neuinstallation.

## Wie man erkennt, in welchem Modus eine Box läuft \{#how-to-tell-which-mode-a-box-is-in}

- Der Installer gibt `unlock: TPM2` oder `unlock: keyfile in the initrd`
  aus, und auf dem Keyfile-Pfad kurz vor dem Abschluss eine Warnung.
- Der Bildschirm der Box zeigt unter ihrer Adresse eine Warnung.
- Die Admin-Seiten zeigen eine Warnung unter Einstellungen, Sicherheit und im
  Einrichtungsassistenten, sobald das Passwort gesetzt ist.
- Einstellungen, Erweitert führt `tpm.enable` und den Wert, mit dem die Box läuft.

## Eine Box auf TPM umstellen \{#moving-a-box-to-a-tpm}

Der Installer schreibt den Entsperrmodus in `modules/install-target.nix`.
Die Admin-Seiten können ihn nicht ändern, weil eine Zeile dafür in
`overrides.nix` mit der des Installers kollidieren würde. Der Weg zum TPM
ist eine Neuinstallation.

1. Kopiere deine Dateien von der Box. Die Neuinstallation formatiert die
   Festplatte.
2. Schalte das TPM im Firmware-Setup ein. Suche nach "TPM",
   "Security Device", "Intel PTT" oder "AMD fTPM", meist unter Security oder
   Advanced.
3. Installiere neu vom [Installer-ISO](/start/install.md). Der Installer findet
   den Chip und verwendet ihn. `losos-ctl install --tpm` macht einen
   fehlenden Chip zu einem Fehler statt zu einer stillen Keyfile-Installation.

:::warning[Den swtpm-Zustand beim Image aufbewahren]
Eine VM braucht einen emulierten Chip bei der Installation und bei jedem
späteren Start. Die swtpm-Zeilen stehen unter
[Installation im Detail](/in-depth/install.md#try-it-in-a-vm-bios). Bewahre das
swtpm-Zustandsverzeichnis zusammen mit dem Festplatten-Image auf, sonst kann
die VM ihre Festplatte nicht entsperren.
:::

Der Keyfile-Modus ist in Ordnung für eine VM, eine Testbox oder eine Box,
die nichts Privates enthält. Für echte Dateien eines Besitzers auf einer
Box, die jemals das Haus verlassen könnte, verwende das TPM.
