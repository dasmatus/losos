---
title: Autoprovision
sidebar_position: 5
mdx:
  format: md
---

# Autoprovision, einschließlich des Netboot-Mediums

Autoprovision ist die Variante des LosOS-Installateurs für Maschinen, die
das Mesh aus dem Netz bootet, und zugleich die Synchronisation, die die
veränderte Box-Konfiguration auf jeder Box des lokalen Mesh gleich hält.
Die drei Teile:

* `losos-ctl provision` — `losos-ctl install` mit einem Zusatzschritt: Die
  Maschine nimmt die Konfiguration des Meshs aus einem *Spiegel*, damit
  eine per Provision installierte Box läuft, was das Mesh läuft, nicht mit
  den Standardwerten.
* `nixosConfigurations.netboot` — die Netboot-Variante des
  Installationsmediums: Kernel, initrd und Squashfs-Store für einen
  PXE/iPXE-Client, dessen Autorun in `losos-ctl provision` endet.
* `losos.autoprovision.enable` — die Synchronisation auf der Box: lososd
  hält `modules/overrides.nix` dieser Box mit demselben Spiegel in Schritt,
  sodass eine auf einer Box angewandte Änderung die anderen erreicht.

## Der Spiegel

Ein Branch einer Git-Adresse, von jeder Box des Mesh ohne Prompt erreichbar
(gewöhnliches http(s), ein Bare-Repository auf einem Pfad, oder eine
SSH-Adresse, die der Daemon ohne Schlüsselfragen öffnet). Alles darauf ist
innerhalb des Mesh geteilt und nichts darauf ist geheim: LUKS-Schlüssel
und Token verlassen nie die Box, der sie gehören.

```
refs/heads/provision
├── modules/overrides.nix                die veränderte Konfiguration des Meshs
└── boxes/
    ├── box-9f13c4ad/install-target.nix  die Hardware der Maschine selbst
    ├── box-9f13c4ad/overrides.nix       die Konfiguration, mit der sie installiert wurde
    └── mattbox/…
```

Eine für Provision gebootete Maschine heißt `box-<first 8 hex of the
machine id>`, solange kein Operator einen Namen wünscht
(`losos-autoprovision --name lease-3`). Im LosOS Git einer Mesh-Box ist dies
ein weiteres privates Repository-klon: Lege es dort für den Spiegel an oder
zeige auf ein beliebiges Git-Remote-Repo, das das Mesh erreicht.

## Provision einer netbooten Maschine

Boote die Maschine mit `boot.ipxe` aus dem Netboot-Image, oder führe den
Befehl von Hand auf einem beliebigen Installationsmedium aus:

```
nix build .#nixosConfigurations.netboot.config.system.build.netbootImage
# serve the result over http, point iPXE at boot.ipxe, and
# replace SERVER in it with the netboot server
```

Was eine Provision tut, der Reihe nach:

1. Sie liest den Spiegel (`git ls-remote`, dann ein flacher Fetch des
   Branches `provision`) und nimmt sein `modules/overrides.nix`. Ein
   Spiegel ohne Branch, ein Branch ohne Konfiguration, oder eine
   Konfiguration, die dieselbe Formenprüfung nicht passiert, die ein Apply
   auf der Box durchläuft, hält hier an, vor den Disks.
2. Das Firmware-Menü, unverändert vom Stick: Es wartet seine 30 Sekunden
   ab und zeigt, was es tun wird.
3. Der normale Installationsplan mit einem Zusatzschritt: Das
   `modules/overrides.nix` des Meshs wird in den Flake-Klon neben das
   erzeugte `modules/install-target.nix` gelegt, sodass die Installation die
   Einstellungen des Meshs baut.
4. Nachdem der Baum auf der Platte ist, schreibt sich die Box selbst in den
   Spiegel, als `boxes/<name>/` auf einem neuen Commit über dem Kopf des
   Branches (nur fast-forward, sodass zwei Maschinen, die gleichzeitig
   provisionieren, widerlegen statt überschreiben). Ein Spiegel, der den
   Eintrag nicht nimmt, ist eine Warnung, nie eine fehlgeschlagene
   Installation.

Die Flags, die die Installation nimmt, nimmt `provision` ebenso: die
Wahlen für die Plattenöffnung (`--tpm`/`--no-tpm`,
`--tang`/`--tang-edge`, `--bios`/`--uefi`, die Drive-Liste) und
`--no-install`. Die Testpfade, aus denen ein vorgefertigtes
Disko-Skript oder `--emit-target` gebildet wird, lehnt es ab, weil diese
den Baum der Box nicht bauen.

Das Medium kann die eigenen Wahlen des Meshs vorgeben, damit niemand an der
Box tippt: `losos.autoprovision.tang` (oder `LOSOS_TANG=URL#THUMBPRINT[,more]`
in der Umgebung des Builds) lässt den Autorun mit `--tang` installieren —
der Disk-Schlüssel ist an den TPM2 *und* den genannten Clevis-Tang-Server
gesiegelt (`tang-show-keys` auf dem Server druckt den Fingerabdruck). Eine
so provisionierte Box lässt sich vom ersten Boot an nicht allein aus ihrer
Platte öffnen; `--tpm`, `--tang-edge` und jedes andere getippte Flag
überschreiben weiterhin die Vorgabe.

## Wie die Boxen im Schritt bleiben

Das Einschalten (`modules/autoprovision.nix`):

```nix
losos = {
  autoprovision = {
    enable = true;
    mirror = "https://git.home.example/mesh-config.git";
    # box = "mattbox";  # defaults to losos.hostName
  };
};
```

lososds Konfigurations-Reconciler — derselbe Thread, der das
Konfigurations-Repository auf der eigenen LosOS Git der Box hält — schaut
auch jede halbe Minute in den Spiegel und entscheidet mit zwei Fakten: der
Text der Box und der Text des Spiegelkopfs gegen das, was die Box von beiden
zuletzt aufgeschrieben hat. Beide bewegt sich → der neuere Schreiber
gewinnt (die Commit-Zeit des Spiegelkopfs gegen den Moment, in dem diese
Box zuletzt pushte). Nur eine Seite bewegt sich → diese Seite ist die
Wahrheit; die Box pusht oder nimmt.

Ein Nehmen geht durch die Tore, durch die jeder Apply geht — die
Formenprüfung, das Options-Dokument und das Edge-Tor — und wenn etwas
verweigert wird, bleibt es verweigert mit dem Grund in der `mesh`-Zeile des
History-Panels (`GET /api/config`, Feld `mesh`). Keine Box pusht je mit
Kraft. Wenn der Spiegel noch keinen Branch hat, pusht die erste Box, die
hinsieht, ihre eigene Konfiguration als Seed. Und wenn eine
provisionierte Box eine Konfiguration installiert hat, die sich später
bewegt, pusht sie sie beim nächsten Sync zurück und das ganze Mesh baut
nach; genau das ist der Punkt.

`losos-ctl config` — das History-Panel — trägt die Antwort des Spiegels
neben die des LosOS Git-Syncs und des veröffentlichten Branches
(`mesh.sync.state`).

## Das Zusammenspiel

Alle drei Hüter von `/etc/nixos` laufen auf einem Thread des Daemons und
nehmen eine State-Lock, damit sie nicht gegenseitig gleichzeitig
schreiben können:

* LosOS Git — die eigene Konfiguration der Box, privat
  (`losos.configRepo.enable`), deckt jede Datei, die es führt.
* das veröffentlichte Branch — neue LosOS-Versionen
  ([Install in depth](install.md)).
* der Spiegel — nur das `modules/overrides.nix` des *Meshs*; die Fakten
  einer Maschine kommen nie aus einem `boxes/`-Eintrag zurück.

Ein Spiegel zu veröffentlichen, der für Fremde öffentlich ist, ist genau so
bewusst wie Speicher verleihen: `losos.autoprovision.enable` ist
standardmäßig aus, und die Konfiguration des Mesh zu nehmen bedeutet, das
zu laufen, was das Mesh committet.
