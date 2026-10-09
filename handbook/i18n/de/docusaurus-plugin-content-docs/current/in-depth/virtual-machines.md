---
title: Virtuelle Maschinen
sidebar_position: 4.5
mdx:
  format: md
---

# Virtuelle Maschinen

Eine Box, die ihre Festplatte mit dem Mesh teilt, kann im Mesh virtuelle
Maschinen mieten und Maschinen anderer Boxen betreiben. Eine Maschine wird
**pro Replikat** verkauft: Jedes Replikat ist eine Kopie der Maschine mit
eigener Festplatte und läuft einen Monat auf der Box, die es betreibt. Bezahlt
wird über den [Marktplatz](/in-depth/market.md), und jeder Verkauf wird **zur Hälfte**
zwischen der Besitzerin oder dem Besitzer der betreibenden Box und LosOS
geteilt.

Der Hypervisor ist [KubeVirt](https://kubevirt.io), die Festplatten importiert
[CDI](https://github.com/kubevirt/containerized-data-importer), beides im
Mesh-Cluster des Edge. Der `losos-registrar` des Edge verkauft die Replikate
und legt die Maschinen an; in der Admin-Oberfläche erledigst du alles auf der
Seite **Maschinen** (in der Seitenleiste unter **Mesh**, unter `/machines`).

## Wer es nutzen kann

Maschinen, zum Mieten wie zum Betreiben, werden nur einer Box angeboten, die
ihre Festplatte teilt (`losos.sharingMyStorage`). lososd prüft das bei jeder
Anfrage: Eine Box, die nicht teilt, bekommt von `GET /api/vms` die Antwort
`{available: false, reason: "notSharing"}`, jede Aktion wird mit 409
abgelehnt, und die Seite zeigt statt des Formulars einen ausgegrauten Satz.

Der Schalter für die Festplattenfreigabe liegt im Bereich Marktplatz, und der
Marktplatz ist noch nicht geöffnet (siehe [Marktplatz](/in-depth/market.md)). Bis dahin
kann die Admin-Oberfläche die Freigabe nicht einschalten, und die Seite
Maschinen bleibt auf jeder Box grau, die `losos.sharingMyStorage` nicht auf
anderem Weg gesetzt hat.

Zwei weitere Gründe, die die Seite nennen kann:

| Grund            | Bedeutung                                                        |
| ---------------- | ---------------------------------------------------------------- |
| `noOfficialEdge` | Kein erreichbarer Edge wird von LosOS betrieben, also wird nichts verkauft. |
| `notOffered`     | Der Edge betreibt keine Maschinen (`losos.edge.vms.enable`).     |

## Was du starten kannst

| Quelle            | Was es ist                                                        |
| ----------------- | ----------------------------------------------------------------- |
| LosOS             | Die LosOS-Demo-Festplatte, geladen als KubeVirt-containerDisk     |
| Cloud-Images      | Von Quickemu unterstützte Systeme mit fertigem Cloud-Image        |
| Eigenes QCOW2     | Ein Image, das du auf der Seite Maschinen hochlädst               |

Der Katalog ist `backend-registrar/vm-images.json`, in den Registrar
eingebaut; Betreiber können ihn mit `--vm-catalogue` ersetzen. Er enthält
Ubuntu Server 26.04 und 24.04, Debian 13 und 12, Fedora 43 Cloud, AlmaLinux 10,
Rocky Linux 10, CentOS Stream 10, Arch Linux, openSUSE Tumbleweed, Alpine Linux
und FreeBSD, jeweils von der Download-Adresse des eigenen Projekts.

**Das LosOS-Image ist die Demo-Festplatte, nicht die Appliance.** Es ist
dasselbe `losos-disk-qcow2`, das das Release veröffentlicht, ohne
Festplattenverschlüsselung und ohne zustandslose Wurzel (siehe
`flake/disk-images.nix`). Es startet mit UEFI und braucht 4 GiB
Arbeitsspeicher, weil es zwei Cluster und LosOS Cloud betreibt. Der
Release-Job veröffentlicht es als
`ghcr.io/<owner>/<repo>-demo-containerdisk:<tag>` und verschiebt den Tag
`stable`, auf den sich der Katalog bezieht.

**Der Großteil der Quickemu-Liste wird nicht angeboten.** Diese Systeme gibt
es nur als Installations-ISO, und eine Installation braucht eine Konsole im
Browser, die die Seite noch nicht hat. Die Seite führt sie unter "Systeme noch
nicht im Angebot" auf, zusammen mit den wenigen, die aus einem anderen Grund
fehlen (macOS, dessen Lizenz nur Apple-Hardware erlaubt; Windows, das eine
Lizenz und eine Konsole braucht; und ein paar, deren Cloud-Images keine feste
Adresse haben oder in einem Archiv kommen, das CDI nicht lesen kann).

Ein Cloud-Image liest **cloud-init**, deshalb nimmt das Bestellformular ein
optionales `#cloud-config`-Dokument oder Skript (bis 16 KiB) an, das die
Maschine beim ersten Start ausführt, etwa um einen SSH-Schlüssel einzurichten.
Der Registrar gibt es nie zurück, weil es ein Passwort enthalten kann.

### Eigenes QCOW2

Die Seite Maschinen lädt eine QCOW2-Datei direkt auf den Edge und zeigt den
Fortschritt. lososd reicht sie durch, ohne sie zwischenzuspeichern und ohne
seine Sperre zu halten, und der Edge prüft sie schon beim Empfang:

- Sie muss mit dem QCOW2-Header beginnen, sonst antwortet er 415.
- Sie muss in das Limit passen (`losos.edge.vms.uploadMaxGiB`, standardmäßig
  32 GiB), sonst 413, und ihre virtuelle Festplatte darf höchstens 512 GiB
  groß sein.
- Sie muss in Bewegung bleiben, denn eine Minute ohne ein Byte bricht ab.

Eine Box kann drei Images behalten. Jedes liegt auf dem Edge unter
`/var/lib/losos-registrar/vm-images/` mit Modus 0600, und jedes Replikat, das
davon startet, importiert es über eine Adresse für genau diesen Zweck, die nur
die eigenen Maschinen der Käuferin oder des Käufers bekommen. Ein Replikat
eines Images bekommt eine Festplatte, die mindestens so groß ist wie die
virtuelle Größe des Images. Setz bei einem Image, das UEFI-Firmware braucht,
den Haken **Startet mit UEFI**; Secure Boot ist in beiden Fällen aus.

## Replikate und Preis

Eine betreibende Box bietet Maschinen mit einem Preis **pro Replikat-Monat**
an und mit der Zahl der Replikate, die sie betreiben will (höchstens 50). Wer
kauft, wählt ein System, einen Namen, ein Angebot und eine Zahl von Replikaten
(höchstens 10 und nicht mehr, als frei sind) und bezahlt auf der Seite von
Stripe in einem neuen Tab.

Alle Replikate auf einem Edge sind gleich groß: `losos.edge.vms.cpu` vCPU,
`memoryMiB` Arbeitsspeicher und `diskGiB` Festplatte (standardmäßig 1, 2048
und 20), oder mehr, wo ein Image mehr verlangt.

**Die Aufteilung ist fest auf 50 %.** Die Zahlung für eine Maschine ist eine
Destination Charge wie jeder andere Verkauf auf dem Marktplatz, mit
`application_fee_amount` genau der Hälfte des Betrags. Die
`losos.edge.market.feeBps` des Betreibers gilt für Maschinen nicht, und das
Stripe-Gate lehnt eine Maschinenzahlung mit jeder anderen Gebühr ab. Bei
6,00 EUR pro Replikat-Monat kosten drei Replikate 18,00 EUR; die betreibende
Box bekommt 9,00 und LosOS behält 9,00.

## Nach der Zahlung

Sobald Stripe die Zahlung bestätigt, macht der Reconciler des Registrars
Folgendes:

1. Er legt im Mesh den Namespace `market-<buyer id>` an, falls es ihn noch
   nicht gibt.
2. Er legt pro Replikat eine `VirtualMachine` an, `vm-<order>-0`, `-1` und so
   weiter, über das Node-Label `losos.dev/appliance` an die betreibende Box
   gebunden, jede mit eigener Festplatte, die CDI in die Speicherklasse
   `losos-vm-local` importiert.
3. Er legt vor den Replikaten einen `Service` auf Port 80 an.

Ist `losos.edge.vms.domain` gesetzt, stellt Traefik auf dem Edge diesen
Service unter `https://vm-<order>.<domain>` bereit, verteilt auf die
Replikate, mit einem Zertifikat von Let's Encrypt. Ohne die Option haben die
Maschinen keine öffentliche Adresse.

Die Seite Maschinen zeigt jede Maschine mit dem Zustand jedes Replikats
(läuft, startet, kopiert die Festplatte, pausiert, angehalten oder
fehlgeschlagen) und ihrer Adresse und fragt alle 15 Sekunden neu, solange ein
Replikat noch hochfährt.

**Wenn der Monat um ist**, wird jedes Replikat angehalten
(`runStrategy: Halted`), und sein Platz geht an das Angebot zurück. Die
Festplatten bleiben. Sie enthalten die Daten der Käuferin oder des Käufers,
und sie zu löschen entscheidet der Betreiber, wie beim Volume einer
Speicherbestellung.

## Maschinen betreiben

Eine Box betreibt Maschinen, solange sie im Mesh ist, ihre Festplatte teilt
und `losos.vms.host` an ist (Standard). Sie tritt dann mit `--host-vms true`
bei, und der Edge lässt sie Maschinen anbieten. Auf so einer Box werden die
Kernelmodule `kvm-intel`, `kvm-amd`, `tun` und `vhost_net` geladen, und die
Festplatten der Replikate liegen unter `losos.edge.vms.hostPath` (standardmäßig
`/home/shared/vms`), im Home des geteilten Benutzers und damit in der
geteilten Datendomäne.

Maschinen laufen rund um die Uhr. Der Node-Agent von KubeVirt und die
Importer von CDI tolerieren den Taint des Rechenfensters, weil eine für einen
Monat gekaufte Maschine nicht jeden Morgen anhalten kann. Biete nur so viele
Replikate an, wie die Box neben ihrer eigenen Arbeit tragen kann.

### Verschachtelte Virtualisierung

KubeVirt braucht auf der betreibenden Box `/dev/kvm`. Auf einem echten
Mini-PC ist das VT-x oder AMD-V des Prozessors. Eine Box, die selbst eine
virtuelle Maschine ist, braucht von ihrem Host **verschachtelte
Virtualisierung** (`kvm_intel nested=1` oder `kvm_amd nested=1` und
`-cpu host` in QEMU). Ohne sie kann der Betreiber
`losos.edge.vms.useEmulation` setzen, dann laufen die Replikate in Software.
Sie starten, aber langsam, und das ist zum Ausprobieren gedacht, nicht zum
Verkaufen.

## Einrichtung durch den Betreiber

Auf dem Edge:

```nix
losos.edge = {
  market.enable = true;   # machines are sold on the market
  cluster.enable = true;  # and run on the mesh
  vms = {
    enable = true;
    domain = "vms.example.net"; # optional: https://vm-<order>.vms.example.net
    # cpu = 1; memoryMiB = 2048; diskGiB = 20; uploadMaxGiB = 32;
    # useEmulation = true;      # only without /dev/kvm on the hosts
  };
};
```

`modules/edge-vms.nix` installiert KubeVirt v1.9.0 und CDI v1.66.1 aus ihren
festgelegten Release-Manifesten über das Manifest-Verzeichnis von rke2, dazu
einen local-path-Provisioner (rancher/local-path-provisioner v0.0.37) unter
eigenen LosOS-Namen für die Speicherklasse `losos-vm-local`, die eine
Festplatte erst anlegt, wenn ihre Maschine einen Node hat, und nie eine
löscht. Der Registrar darf `VirtualMachine`s lesen, anlegen und ändern und
`Service`s anlegen.

Für `domain` richte einen Wildcard-DNS-Eintrag (`*.vms.example.net`) auf den
Edge. Solange Maschinen an sind, ist das Lese-Timeout am öffentlichen
Einstiegspunkt von Traefik auf sechs Stunden erhöht, damit ein großer Upload
nicht abbricht.

## API

Auf der Box (mit Bearer-Token, hinter der Sperre nur für das LAN):

| Route                              | Macht                                          |
| ---------------------------------- | ---------------------------------------------- |
| `GET /api/vms`                     | Katalog, Angebote, Konto, Zustand der Maschinen |
| `POST /api/vms/orders`             | `{listing_id, quantity, image, name, user_data?}` |
| `POST /api/vms/listings`           | `{unit_price, capacity}`, pro Replikat-Monat   |
| `POST /api/vms/listings/close`     | `{listing_id}`                                 |
| `PUT /api/vms/images?name=&efi=1`  | Lädt ein QCOW2 hoch (der Body ist die Datei)   |
| `POST /api/vms/images/remove`      | `{upload_id}`                                  |

Auf dem Edge, neben dem übrigen `/market/*`: `GET /market/vm-images`
(öffentlich, wie das Regal), `POST /market/vms/status`,
`POST /market/vm-images/ticket`, `POST /market/vm-images/remove` und die
beiden Übertragungsrouten `PUT /market/vm-images/upload/<ticket>` und
`GET /market/vm-images/fetch/<token>`, die statt eines Mandanten-Tokens
Berechtigungen annehmen und höchstens vier Übertragungen gleichzeitig
zulassen.

## Sicherheitseigenschaften

- Eine Box, die ihre Festplatte nicht teilt, kann weder mieten noch
  betreiben; lososd lehnt ab, bevor der Edge gefragt wird.
- Der Edge nimmt Maschinenangebote nur von einer Box im Mesh an, die meldet,
  dass sie Maschinen betreibt.
- Die Gebühr eines Maschinenverkaufs ist die Hälfte, was das Stripe-Gate
  noch einmal prüft.
- Ein hochgeladenes Image ist nur über eine 64-stellige Berechtigung
  erreichbar, die nur in die eigenen Festplatten der Käuferin oder des Käufers
  geschrieben wird; das Upload-Ticket gilt einmal und eine Stunde lang.
- Das Upload-Ticket erreicht curl über eine temporäre Datei, nicht über die
  Befehlszeile.
- Nichts, was der Registrar tut, löscht eine Festplatte der Käuferin oder des
  Käufers.

## Nicht abgedeckt

- Keine Konsole im Browser, also keine Installations-ISOs und keine
  Anmeldung außer der, die cloud-init einrichtet.
- Keine Live-Migration: Ein Replikat läuft auf der Box, auf der es verkauft
  wurde.
- Keine Snapshots, keine Größenänderung einer laufenden Maschine, keine
  Größenwahl pro Bestellung.
- Die VM-Tests starten keine KubeVirt-Maschine; die Tests des Registrars
  spielen Bestellung, Upload und Bereitstellung gegen einen falschen
  Apiserver und Stripe durch.
