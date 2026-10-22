---
title: Der Name wird nicht aufgelöst
sidebar_position: 3
slug: /troubleshooting/name-does-not-resolve
---

# Der Name wird nicht aufgelöst

<div className="losos-symptom">

**Was du siehst:** `http://<name>.local` liefert „Server nicht gefunden“,
„Die Website ist nicht erreichbar“ oder „Hmm … diese Seite wurde nicht
gefunden“, während `http://<address>` funktioniert.

</div>

## Warum \{#why}

`<name>.local` ist ein mDNS-Name. Die Box kündigt ihn im lokalen Netzwerk
an, und dein Rechner muss darauf hören. Windows, macOS, iPhones, Android und
die meisten Linux-Desktops tun das. Diese nicht:

- **Der Host einer virtuellen Maschine** in einem NAT-Netzwerk, was sowohl
  bei libvirt als auch bei VirtualBox die Voreinstellung ist. Der Host hört
  die Ankündigung des Gasts nie. Das ist der übliche Fall bei einer
  Vorführung.
- **Ein Rechner in einem anderen Netzwerk** oder hinter einem Router, der
  Multicast zwischen WLAN- und kabelgebundenen Geräten blockiert, wie es
  manche Einstellungen zur „AP-Isolation“ tun.
- **Linux ohne mDNS-Resolver**, also weder mit Avahi noch mit
  systemd-resolved mit eingeschaltetem mDNS, oder ohne `.local` in der
  `nsswitch.conf`.

## Was du tun kannst \{#what-to-do}

![Das Banner unter dem Lachs auf dem Bildschirm der Box: Die IP-Adresse funktioniert, wo der .local-Name es nicht tut.](@site/docs/img/tty1-banner.png)

- **Nimm die Adresse.** Alles antwortet darauf, die Apps eingeschlossen.
  Der Bereich Über zeigt, welche Adresse dein Browser verwendet. Nur das
  `https://`-Zertifikat braucht den Namen, weil die Box es für den Namen
  ausstellt.
- **Auf einem VM-Host** fügst du der `hosts`-Datei des Hosts eine Zeile
  hinzu. Diese Datei ist `/etc/hosts` unter Linux und macOS und
  `C:\Windows\System32\drivers\etc\hosts` unter Windows. Die Zeile lautet
  `192.168.122.56  <name>.local`. Dann funktionieren der Name und das
  Zertifikat.
- **Unter Linux ohne mDNS-Resolver** installierst du `avahi` und
  `nss-mdns`, oder du setzt `MulticastDNS=yes` in der `resolved.conf` von
  systemd-resolved und an der Netzwerkverbindung, oder du nimmst die
  hosts-Datei wie oben.

## Nicht diese Seite \{#not-this-page}

Wenn auch die Adresse nicht funktioniert, ist es
[Die Box ist nicht erreichbar](cannot-reach-the-box). Wenn der Name
funktioniert, aber `https://` warnt, ist es
[Zertifikatswarnung](certificate-warning).
