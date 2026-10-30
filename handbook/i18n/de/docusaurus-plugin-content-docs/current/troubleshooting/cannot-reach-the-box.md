---
title: Die Box ist nicht erreichbar
sidebar_position: 2
slug: /troubleshooting/cannot-reach-the-box
---

# Die Box ist nicht erreichbar

<div className="losos-symptom">

**Was du siehst:** Der Browser meldet, dass die Website nicht erreichbar ist
oder die Verbindung eine Zeitüberschreitung hatte, oder die Seite lädt
endlos. Nichts auf der Box antwortet.

</div>

## Aus dem LAN \{#from-the-lan}

![Das Banner, das die Box unter dem Lachs zeigt, wenn sie gestartet ist und ein Netzwerk hat, mit der Adresse zum Eintippen.](@site/docs/img/tty1-banner.png)

1. **Lies den Bildschirm.** Das Banner zeigt die Adresse, die die Box
   *jetzt* hat. Ein Router, der neu gestartet ist, hat ihr vielleicht eine
   neue gegeben, und das Banner zeichnet sich von selbst neu. Tipp diese
   Adresse ein, als `http://`, nicht den Namen und nicht `https://`.
2. **Dasselbe Netzwerk?** Ein Laptop im Gäste-WLAN oder in einem VPN ist in
   einem anderen Netzwerk. Die Box und der Rechner brauchen beide eine
   Adresse im selben Bereich, zum Beispiel beide `192.168.1.x`. Die
   Admin-Seiten lehnen außerdem jede Adresse ab, die nicht privat ist. Eine
   Anfrage, die über einen Tunnel oder aus dem Internet kommt, bekommt
   absichtlich **403**.
3. **Das Banner sagt „No network address yet.“** Steck das Ethernet-Kabel
   ein oder an einen anderen Port des Routers. Die Box hat keine
   WLAN-Einrichtung. Der Bildschirm aktualisiert sich von selbst, sobald
   das Kabel eine Verbindung hat.
4. **Kein Banner, schwarzer Bildschirm.** Gib ihr nach dem Einschalten zwei
   Minuten. Bleibt der Bildschirm schwarz, prüf die Betriebsleuchte und den
   Eingang des Monitors. Eine Login-Eingabe im Textmodus statt des Banners
   ist der eine Zustand, den der nächtliche Neustart behebt. Warte bis
   00:07 oder zieh den Stecker.
5. **Nur eine Seite ist weg.** `http://<address>/api/health` antwortet,
   LosOS Cloud aber nicht. Entweder startet die App noch, was nach jedem
   Neustart ein paar Minuten dauert, oder siehe
   [Eine App-Kachel liefert 404](app-tile-404).

## Von außerhalb des LAN \{#from-outside-the-lan}

Eine Box für sich allein ist von außen nicht erreichbar, und das ist kein
Fehler. Eine Box mit Edge ist unter ihrem öffentlichen Namen erreichbar,
aber nur für LosOS Cloud und LosOS Git. Die Admin-Seiten veröffentlicht die
Box nie. Wenn der öffentliche Name nicht mehr antwortet, ist der Tunnel
unterbrochen. Im LAN zeigt der Bereich Mesh, ob die Box die Edge findet,
und [Keine Edge gefunden](edge-not-found) behandelt den Fall, dass sie es
nicht tut. Die Box verbindet sich von selbst wieder, sobald sie die Edge
findet.

## Immer noch nichts \{#still-nothing}

:::tip[Neu laden ohne Browser-Cache]
Wenn das Banner eine Adresse zeigt und `http://<address>/api/health`
antwortet, die Admin-Seiten aber nicht laden, hat dein Browser vielleicht
die Seite von vor einem Update im Cache. Lade mit Strg+Umschalt+R neu, um
den Cache zu umgehen, oder öffne ein privates Fenster.
:::

Lädt die Seite, aber jeder Bereich wartet oder zeigt einen Fehler, läuft
der Steuerdienst vielleicht nicht. `https://<address>/basic/settings/logs`
öffnet sich auch ohne ihn und zeigt seine Zeilen, mit dem
Ersatzschlüssel, wenn auch LosOS Login nicht antwortet. Siehe
[Protokolle](../manual/logs).

Wenn das Banner eine Adresse zeigt und unter ihr gar nichts antwortet, ist
der Webserver der Box, nginx, ausgefallen, und der nächtliche Neustart
behebt das. Ist er am nächsten Morgen immer noch aus,
[hol dir Hilfe](getting-help) mit dem Text des Banners.
