---
title: Die Box erreichen
sidebar_position: 4
---

# Die Box erreichen

Alles auf einer LosOS-Box ist eine Webseite unter einer Adresse. Diese
Adresse lässt sich auf zwei Arten schreiben, und die beiden sind nicht gleich
zuverlässig.

## Über die IP-Adresse (funktioniert immer) \{#by-ip-address-always-works}

Das Banner auf dem Bildschirm der Box zeigt `http://192.168.x.y`, die
Adresse, die dein Router der Box gegeben hat. Jede Seite antwortet darunter:
die Admin-Seiten unter `/`, LosOS cloud unter `/nextcloud`, LosOS Git unter
`/forgejo/`, dieses Handbuch unter `/handbook/`. Die Box zeichnet das Banner
neu, wenn sich die Adresse ändert.

![Das Banner unter dem Lachs auf dem Bildschirm der Box, mit der Adresse zum Eintippen in einen Browser und dem .local-Namen der Box.](@site/docs/img/tty1-banner.png)

Wenn an der Box kein Bildschirm hängt, zeigt die Geräteliste deines Routers
die Box unter dem Namen, den du ihr gegeben hast, oder vor der Einrichtung
als `losos`.

## Über den Namen (funktioniert auf den meisten Rechnern) \{#by-name-works-on-most-computers}

Die Box kündigt `<name>.local` über mDNS an. Windows, macOS, iPhones,
Android-Handys und die meisten Linux-Desktops lösen den Namen auf, also
öffnet `http://<name>.local` dieselben Seiten. Zwei Arten von Rechnern tun
das nicht:

- Der Host einer virtuellen Maschine in einem NAT-Netzwerk, etwa bei libvirt
  oder VirtualBox. Der Host sieht die Ankündigungen des Gasts nie.
- Ein Rechner in einem anderen Netzwerk oder hinter einem Router, der
  Multicast filtert.

Der Bereich **Über** der Admin-Seiten zeigt die Adresse, die dein Browser
gerade benutzt, und den `.local`-Namen in einer eigenen Zeile, sodass du
siehst, welche du hast. Links, die die Apps herausgeben, etwa Freigabelinks
und Clone-URLs, verwenden den Namen. Der Name bleibt gleich, wenn der Router
eine neue Adresse vergibt.

![Der Bereich Über: die Adresse, die dieser Browser benutzt hat, der .local-Name, der Speichermodus, der Zustand des Mesh und der Link zu diesem Handbuch.](@site/docs/img/settings-about-de.png)

## Über HTTPS \{#over-https}

Die Box liefert `https://` auf Port 443 mit einem eigenen Zertifikat für
`<name>.local` aus, das zwei Jahre gültig ist. Nachdem du dieses Zertifikat
installiert hast, wie unter
[Erster Start, Schritt 1](first-run#step-1-trust-this-box) beschrieben,
zeigt `https://<name>.local` keine Warnung. Das Zertifikat enthält die
IP-Adresse nicht, also warnt `https://<address>` immer. Nimm stattdessen
einfaches `http://<address>` oder den Namen.

## „Find my box“ \{#find-my-box}

Wenn du lieber keinen Bildschirm ablesen und keine Adresse tippen willst,
öffne [losos-edge.dasmat.us/find](https://losos-edge.dasmat.us/find) in
Chrome 142 oder neuer und drück **Find my box**. Chrome fragt einmal, ob die
Seite in deinem lokalen Netzwerk nach Geräten suchen darf. Sag ja, dann
findet die Seite die Box an ihrem Namen und verlinkt dich dorthin. Die Suche
läuft in deinem Browser, und nichts über dein Netzwerk verlässt deinen
Rechner. Firefox und Safari haben keine solche Berechtigung, dort zeigt die
Seite stattdessen, wie du die Adresse eintippst.

<img src={require("@site/docs/img/find-my-box.png").default} alt="Die Seite Find my box, nachdem Chrome im lokalen Netzwerk suchen durfte: die Box an ihrem Namen gefunden, mit einem Link zum Öffnen." width="440" />

## Von außerhalb deines Netzwerks \{#from-outside-your-network}

Standardmäßig gar nicht. Eine Box ist nur aus dem lokalen Netzwerk
erreichbar. Die Einrichtung mit [offizieller Edge](../types/official-edge)
gibt ihr über einen Tunnel eine öffentliche Adresse. Selbst dann bleiben die
Admin-Seiten nur im LAN erreichbar, und nur LosOS cloud und LosOS Git gehen
durch den Tunnel nach außen.

![Einstellungen, Netzwerk: der Name, Verbindung verschlüsseln und Auch von außerhalb erreichbar, der Schalter, der den Tunnel zu einer Edge öffnet.](@site/docs/img/settings-network-de.png)
