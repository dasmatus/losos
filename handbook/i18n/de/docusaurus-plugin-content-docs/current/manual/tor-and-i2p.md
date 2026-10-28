---
title: Tor und I2P
sidebar_position: 4.52
slug: /manual/tor-and-i2p
---

# Tor und I2P

Die Box kann eine Adresse im Tor-Netz haben, einen Namen auf `.onion`, und
eine im I2P-Netz, einen Namen auf `.b32.i2p`. Wer den Tor Browser oder einen
für I2P eingerichteten Browser hat, öffnet dort von überall LosOS Login,
LosOS Cloud und LosOS Git. Die Box braucht dafür keinen Edge und keine
Domain. An deinem Router öffnet sich auch nichts, denn Tor und I2P verbinden
sich von der Box aus nach draußen.

Die Verwaltungsseiten antworten unter diesen Adressen nie. Sie bleiben im
Netz der Box, wie bei jeder anderen Adresse.

## Einschalten

Eine Person mit Adminrechten schaltet jedes Netz unter **Einstellungen** >
**Netzwerk** > **Tor und I2P** ein und übernimmt die Änderung. Beide sind
anfangs aus.

Sobald die Box neu gebaut ist, erstellt Tor seine Adresse in wenigen
Sekunden und I2P in etwa einer Minute. Bis dahin steht in der Zeile **Wird
erstellt…**. Danach erscheinen die Adressen auf derselben Seite, jede mit
einem Knopf, der sie kopiert.

![Einstellungen, Netzwerk: Tor und I2P, die eigenen .onion- und .b32.i2p-Adressen der Box und die zwei von ihrem Edge, jede mit einem Knopf zum Kopieren.](@site/docs/img/tor-and-i2p-de.png)

Die Box behält ihre Adressen. Schaltest du ein Netz aus und wieder ein,
kommt dieselbe Adresse zurück, und Updates und Neustarts ändern sie nicht.

## Die Box unter ihrer Adresse öffnen

- **Tor.** Füge die Adresse in den Tor Browser ein.
- **I2P.** Starte einen I2P-Router auf deinem Computer, etwa i2pd oder den
  Java-I2P-Router, und stelle deinen Browser auf dessen Proxy ein.

Beginne die Adresse mit `https://`. LosOS Login meldet nur über https an.
Das Zertifikat für ihre Adressen erstellt die Box selbst und signiert es mit
ihrem eigenen Zertifikat. Ein Gerät, das der Box seit der Ersteinrichtung
vertraut, vertraut also auch diesen Adressen. Andere Geräte warnen beim
ersten Mal, wie bei den meisten .onion-Seiten. `http://` geht auch, aber
anmelden kannst du dich darüber nicht.

Unter diesen Adressen antwortet die Box mit dem, was ihr öffentlicher Name
über einen Edge zeigt: LosOS Login unter `/auth/`, LosOS Cloud unter
`/cloud/` und LosOS Git unter `/git/`. Die Verwaltungsseiten, `/api/` und
die Ersteinrichtung antworten mit `403`, egal welchen Namen der Browser
schickt.

## Adressen von deinem Edge

Ein Edge kann jeder Box, die er bedient, eine eigene Tor- und I2P-Adresse
geben. Diese Adressen entstehen und bleiben auf dem Edge, die Box selbst
startet dafür weder Tor noch I2P. Sie führen durch den Tunnel der Box zu
ihr, so wie ihr öffentlicher Name, und die Box zeigt sie unter
**Einstellungen** > **Netzwerk** als **Tor, über deinen Edge** und **I2P,
über deinen Edge**. Das schaltet die Person ein, die den Edge betreibt, für
den ganzen Edge. Auf der Box gibt es dafür keine Einstellung.

Der Edge antwortet unter diesen Adressen über https mit einem Zertifikat,
das er selbst erstellt. Dein Browser warnt deshalb beim ersten Öffnen.

Der Unterschied liegt darin, wer den Verkehr trägt. Die eigenen Adressen
der Box führen von Tor oder I2P direkt zur Box. Die Adressen eines Edge
laufen über den Edge, der die Anfragen so sieht wie die für den
öffentlichen Namen der Box.

## Was auf der Box läuft

Mit Tor startet die Box Tor, das ihren Onion-Dienst veröffentlicht. Mit I2P
startet sie i2pd mit einem Tunnel für ihre Adresse. Keiner von beiden bietet
Programmen auf der Box einen Proxy an, und keiner lauscht im Netz der Box.
i2pd trägt außerdem ein wenig Verkehr für andere I2P-Nutzer, höchstens
32 KB/s, wie jeder I2P-Router.
