---
title: In VMs ausprobieren
sidebar_position: 6
---

# In VMs ausprobieren

Eine Zeile auf einem Linux-Rechner richtet zwei LosOS-Boxen mit einer Edge
dazwischen ein, als virtuelle Maschinen in einem privaten Netz, und zeigt,
was sie zusammen tun:

```sh
curl -fsSL https://raw.githubusercontent.com/dasmatus/losos/main/nix/demo/box-edge-box/bootstrap.sh | sh
```

Außer [Nix](https://nixos.org/download) muss vorher nichts installiert
sein. Fehlt Nix, gibt das Skript die Zeile aus, die es installiert, und
hört auf. Nichts braucht root, nichts wird außerhalb von `/nix` und dem
Demo-Ordner (`~/.local/share/losos-demo`) installiert, und die VMs sind nur
über `127.0.0.1` erreichbar.

## Was das Skript tut \{#what-the-script-does}

Zuerst sieht es sich den Rechner an: Linux auf x86_64, KVM, Prozessoren,
Arbeitsspeicher, freier Plattenplatz und Nix. Dann schreibt es ein zweites
Skript, `stage2.sh` im Demo-Ordner, mit dem Gefundenen am Anfang, und
startet es. Das zweite Skript baut oder lädt alles und startet die VMs.
Später lässt es sich auch allein wieder starten.

Die Demo gibt es in zwei Größen, und das Skript wählt die größere, die der
Rechner tragen kann:

| | Voll | Leicht |
| --- | --- | --- |
| Braucht | KVM, 14 GiB Arbeitsspeicher, 60 GB freien Platz | 6 GiB Arbeitsspeicher, 15 GB freien Platz |
| Die Boxen | vom Installations-ISO installiert, mit LosOS cloud | die Admin-Seiten und die Teile, die mit der Edge sprechen |
| Die Edge | eine Edge-VM, die auch der Router des Netzes ist | das [Edge-Gateway](../in-depth/edge-federation.md) |
| Dauer | etwa eine halbe Stunde mit KVM | etwa 15 Minuten, mit oder ohne KVM, nach dem ersten Bauen |

`--full` oder `--light` nach `sh -s --` wählt die Größe selbst, zum Beispiel
`... | sh -s -- --light`.

## Was Sie sehen \{#what-you-see}

1. Zwei Boxen und keine Edge. Jede sucht eine und findet nichts. Gebeten,
   ihre Platte zu teilen, lehnt jede ab, weil es keine Edge gibt, über die
   sie teilen könnte.
2. Die Edge wird eingeschaltet. Beide Boxen finden sie im Netz von selbst.
3. Die Edge veröffentlicht beide Boxen unter ihren öffentlichen Namen, mit
   je einem Tunnel.
4. Box A öffnet Box B über die Edge unter ihrem öffentlichen Namen, und Box
   B öffnet Box A. Die Admin-Seiten werden auf diesem Weg abgelehnt. Sie
   bleiben im lokalen Netz.
5. Beide Besitzer erlauben der Edge, ihre Box zu verwalten, und die eigene
   Seite der Edge zeigt beide.
6. Mit der Edge in Reichweite nehmen beide Boxen das Teilen ihrer Platte an.
7. Die Edge wird ausgeschaltet. Beide Boxen bemerken es, schließen ihre
   Tunnel, lehnen das Teilen wieder ab, und jede bedient weiter ihr eigenes
   Netz.

Um es jemandem Schritt für Schritt zu zeigen, fügen Sie `--pause` hinzu. Die
leichte Demo wartet dann bei jedem Schritt und gibt aus, wo im Browser die
Mesh-Seite jeder Box und die Seite der Edge zu öffnen sind.

## Was anders ist als im echten Einsatz \{#how-it-differs-from-a-real-setup}

- Die Edge hat kein Zertifikat, weil ein privates Netz keinen öffentlichen
  Namen hat, den es nachweisen könnte. Eine echte Edge mit öffentlichem
  Namen bekommt eines von Let's Encrypt.
- Die öffentlichen Namen der Boxen richtet die Demo auf die Edge. In einem
  echten Netz tut das DNS.
- Die Edge ist eine Firmen-Edge, keine offizielle, daher bleibt der Markt
  aus.
- In der leichten Demo läuft LosOS cloud nicht. Ein kleiner Platzhalter
  antwortet an seiner Stelle, hinter dem echten Webserver der Box.
