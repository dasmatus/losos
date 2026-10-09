[English](TPM) · [Slovenčina](TPM-sk) · **Deutsch**

# TPM und Entsperren der Festplatte

Eine Box ohne TPM-Chip ist weniger sicher als eine mit. Diese Seite erklärt,
was ein TPM ist, wofür LosOS es verwendet und worauf eine Box ohne TPM
verzichtet.

## Was ein TPM ist

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

## Wofür LosOS es verwendet

Das Datenvolume (`/persist`) ist auf jeder Box mit LUKS verschlüsselt. Der
Installer erzeugt einen zufälligen Schlüssel, formatiert das Volume damit und
versiegelt den Schlüssel anschließend mit `systemd-cryptenroll` im TPM. Beim
Start fragt die initrd den Chip nach dem Schlüssel und öffnet das Volume.
Die Boot-Partition enthält kein Geheimnis.

Die Box versiegelt außerdem zwei kleinere Geheimnisse im Chip:

- Die Geheimnisse, die die Box für sich selbst erzeugt, etwa das erste
  Admin-Passwort von Nextcloud (`modules/keyring.nix`, über
  `systemd-creds`).
- Den Schlüssel, der das Speicherverzeichnis des Mesh verschlüsselt
  (`modules/fscrypt.nix`).

Eine Kopie des Festplattenschlüssels bleibt außerdem im verschlüsselten
Volume unter `/etc/keys/persist-keyfile`. `losos-ctl grow` verwendet sie, um
das Volume zu vergrößern. Von außen kann sie das Volume nicht öffnen, weil
sie darin liegt.

## Worauf eine Box ohne TPM verzichtet

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
   Dateien des Besitzers in LosOS cloud, die Repositories in LosOS Git, der
   Admin-Schlüssel und der Rest von `/persist`.
2. Die eigenen Geheimnisse der Box verlieren ihren Chip. Der Keyring fällt
   auf den Host-Schlüssel von `systemd-creds` zurück, der Speicherschlüssel
   des Mesh auf eine einfache Datei unter `/var/secrets/losos-shared-key`.
   Beide liegen auf demselben Volume wie die Daten, die sie schützen. Eine
   Kopie von `/persist`, die die Box verlässt, nimmt die Schlüssel mit, und
   ein Fehler, der einen Dienst beliebige Dateien als root lesen lässt, liest
   beide Hälften.

Die Netzwerkseite ändert sich nicht. Der LAN-Guard, die Anmeldung, der
Tunnel, die Härtung und die Trennung der beiden Konten funktionieren mit und
ohne Chip gleich.

## Wogegen ein TPM nicht schützt

LosOS versiegelt den Schlüssel, ohne ihn an Firmware-Messwerte (PCRs) zu
binden. Die Box aktualisiert ihren Bootloader und Kernel jede Nacht, ohne
dass jemand zusieht. Ein an diese Messwerte gebundener Schlüssel würde die
Box nach einem Update von ihrer eigenen Festplatte aussperren, und die Box
hat keine Shell, um sich davon zu erholen.

Der Chip gibt den Schlüssel also an alles heraus, was auf diesem Rechner
startet, auch an den USB-Stick eines Diebes. Ein TPM macht die Festplatte
allein unlesbar. Es hilft nicht mehr, sobald jemand die ganze Box hat. Das
[Sicherheitsmodell](Security-Model-de) führt das als bekannte Einschränkung.

## Wie man erkennt, in welchem Modus eine Box läuft

- Der Installer gibt `unlock: TPM2` oder `unlock: keyfile in the initrd`
  aus, und auf dem Keyfile-Pfad kurz vor dem Abschluss eine Warnung.
- Der Bildschirm der Box zeigt unter ihrer Adresse eine Warnung.
- Die Admin-Seiten zeigen eine Warnung unter Settings, Security und im
  Einrichtungsassistenten, sobald das Passwort gesetzt ist.
- Settings, Advanced führt `tpm.enable` und den Wert, mit dem die Box läuft.

## Eine Box auf TPM umstellen

Der Installer schreibt den Entsperrmodus in `modules/install-target.nix`.
Die Admin-Seiten können ihn nicht ändern, weil eine Zeile dafür in
`overrides.nix` mit der des Installers kollidieren würde. Der Weg zum TPM
ist eine Neuinstallation.

1. Kopieren Sie Ihre Dateien von der Box. Die Neuinstallation formatiert die
   Festplatte.
2. Schalten Sie das TPM im Firmware-Setup ein. Suchen Sie nach "TPM",
   "Security Device", "Intel PTT" oder "AMD fTPM", meist unter Security oder
   Advanced.
3. Installieren Sie neu vom [Installer-ISO](Install-de). Der Installer findet
   den Chip und verwendet ihn. `losos-ctl install --tpm` macht einen
   fehlenden Chip zu einem Fehler statt zu einer stillen Keyfile-Installation.

Eine VM braucht einen emulierten Chip bei der Installation und bei jedem
späteren Start. Die swtpm-Zeilen stehen unter
[Installation](Install-de#try-it-in-a-vm-bios). Bewahren Sie das
swtpm-Zustandsverzeichnis zusammen mit dem Festplatten-Image auf, sonst kann
die VM ihre Festplatte nicht entsperren.

Der Keyfile-Modus ist in Ordnung für eine VM, eine Testbox oder eine Box,
die nichts Privates enthält. Für echte Dateien eines Besitzers auf einer
Box, die jemals das Haus verlassen könnte, verwenden Sie das TPM.
