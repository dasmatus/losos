---
title: Eine Box mit der LosOS-Edge
sidebar_position: 2
---

# Eine Box mit der LosOS-Edge

Eine **Edge** ist ein Server im Internet, zu dem eine Box einen Tunnel offen
hält. Über sie bekommt die Box eine öffentliche Adresse, ohne zu Hause einen
Port zu öffnen, und tritt dem Mesh anderer Boxen bei. Die Box kann auch auf
dem Markt der Edge handeln. Das Projekt betreibt die **LosOS-Edge**, und nur
der Markt dieser Edge zahlt aus.

## Was die Edge für eine Box tut \{#what-the-edge-does-for-a-box}

| Funktion               | Wie                                                                                                        |
| ---------------------- | ---------------------------------------------------------------------------------------------------------- |
| **Fernzugriff**        | `https://<your-name>.<edge domain>` erreicht LosOS cloud und LosOS Git über einen verschlüsselten Tunnel, den die Box nach außen öffnet. Die Admin-Seiten veröffentlicht die Box nie; sie bleiben nur im LAN erreichbar, auch über den Tunnel. |
| **Das Mesh**           | Die Box tritt einem Kubernetes-Cluster bei, den die Edge betreibt. Ihre freie Festplatte wird Teil eines gemeinsamen, replizierten Pools; ihre freie CPU führt innerhalb der Stunden, die du erlaubst, Arbeit anderer Leute aus, und nur, solange die Box untätig ist. |
| **Der Markt**          | Du wirst für die Festplatte und die CPU bezahlt, die du teilst, und du kannst welche von anderen kaufen. Käufer zahlen in USDC, Bitcoin oder Monero, oder per Karte über Stripe, wo die Edge Karten annimmt, und die Edge behält eine Gebühr von 4 %. Eine Edge kann auch nur Coins annehmen, ohne Kartenzahlungen; dann braucht es für den Betrieb des Markts kein Geschäftskonto bei einem Kartenzahlungsanbieter. Der Bereich Markt liegt in der Seitenleiste unter Mesh. |
| **Box finden**         | Die Edge stellt die Seite bereit, die eine Box in deinem LAN aus einem Browser heraus findet.              |

![Der Bereich Markt: was diese Box gekauft hat, Speicher und Rechenleistung zum Kauf von anderen Boxen und Auszahlungen über Stripe.](@site/docs/img/market-open-de.png)

## Wie eine Box ihre Edge findet \{#how-a-box-finds-its-edge}

Alle 20 Sekunden sucht die Box an zwei Stellen nach einer Edge:

![Wie eine Box eine Edge findet: Sie sucht alle 20 Sekunden im LAN und unter der konfigurierten Adresse, fordert jede antwortende Edge heraus, und vier Prüfungen entscheiden, ob die Edge offiziell ist; keine Antwort heißt, dass das Teilen verweigert bleibt.](@site/docs/img/edge-discovery-de.svg)

- **Im lokalen Netz**, per DNS-SD. Eine Edge, die so eingerichtet ist, dass
  sie sich ankündigt, veröffentlicht `_losos-edge._tcp` per mDNS mit der
  Adresse ihrer API. Die Box findet sie auf dieselbe Weise, wie sie ihren
  eigenen `.local`-Namen findet.
- **Unter der konfigurierten Adresse**. Das ist die Control Plane der
  LosOS-Edge, sofern der Besitzer sie nicht unter Einstellungen → Erweitert
  in `losos.proxy.registrarUrl` geändert hat. Die Box fragt dort nach, egal
  ob **Auch von außerhalb erreichbar** an ist oder nicht.

Eine Edge gilt als gefunden, sobald ihr `/health` antwortet. Es können
mehrere zugleich erreichbar sein, zum Beispiel die Edge einer Firma im LAN
und die LosOS-Edge über das Internet. Der Bereich Mesh zeigt eine Zeile pro
Edge, markiert mit **In diesem Netzwerk** oder **Über das Internet**, und mit
einem Zeichen dafür, ob die Edge offiziell ist. Der nächste Abschnitt erklärt
dieses Zeichen. Antwortet nichts, zeigt der Bereich **Kein Edge-Proxy
gefunden** und was die Box versucht hat: „Diese Box hat ihr Netzwerk
durchsucht und `<address>` gefragt; nichts hat geantwortet.“

![Der Bereich Mesh mit der offiziellen LosOS-Edge, gefunden über das Internet, markiert mit einem Häkchen.](@site/docs/img/mesh-official-de.png)

Solange nichts antwortet, weigert sich die Box, Teilen einzuschalten, das vom
Netz abhängt. Das Umschalten des Speichers in den Mesh-Modus, **Dem Mesh
beitreten** und **Die Festplatte dieser Box mit dem Mesh teilen** sind
ausgegraut mit „Braucht einen erreichbaren Edge-Proxy.“, und die Box lehnt
jedes Anwenden, das eines davon trotzdem einschaltet, mit derselben
Begründung ab. Einstellungen, die schon an sind, lässt die Box in Ruhe. Eine
Box, deren Edge verschwunden ist, behält ihre Konfiguration, kann alles
andere weiterhin ändern und kann das Teilen immer *ausschalten*. Deine
eigenen Dateien und Apps hängen nie von der Edge ab. Auch der Schalter für
den Tunnel ist nicht gesperrt. Der Tunnel verbindet sich wieder, sobald die
Edge wieder antwortet.

## Offiziell oder nicht \{#official-or-not}

Jede Edge, die antwortet, öffnet das Teilen. Nur die LosOS-Edge darf den
Markt betreiben, also muss eine Box sie von jeder anderen Edge
unterscheiden. Eine Edge zu finden und ihr Geld anzuvertrauen sind zwei
verschiedene Fragen. Jede Box trägt einen öffentlichen Root-Schlüssel von
LosOS. Eine offizielle Edge weist sich bei jedem Suchlauf aus. Sie legt ein
Zertifikat vor, das mit dem passenden privaten Schlüssel signiert ist, den
nur das Projekt hält, und zwar offline. Außerdem signiert sie mit dem
eigenen Schlüssel des Zertifikats eine frische Herausforderung, die sich die
Box gerade ausgedacht hat. Die Box macht vier Prüfungen, und die Edge gilt
nur dann als offiziell, wenn alle vier bestehen.

Der Bereich Mesh zeigt das Ergebnis als Zeichen neben dem Namen jeder Edge.
Eine offizielle Edge bekommt ein Häkchen. Jede andere Edge bekommt eine
Warnung, mit einem Tooltip, der auflistet, was diese Edge für diese Box nicht
tun kann: auf dem Markt kaufen oder den freien Speicher und die freie
Rechenleistung dieser Box verkaufen. Eine Box, die die Signatur nicht prüfen
kann, behandelt die Edge als [Firmen-Edge](company-edge). Erkennung,
Fernzugriff und Teilen funktionieren, Handel nicht, und der Bereich Markt
sagt: „Der Edge-Proxy, den diese Box gefunden hat, wird nicht von LosOS
betrieben.“ Nichts, was eine Firma konfiguriert, kann ihre eigene Edge
offiziell machen. Das ist Absicht.

![Das Warnzeichen neben einer Edge, die nicht offiziell ist, mit seinem Tooltip, der auflistet, was diese Edge für diese Box nicht tun kann.](@site/docs/img/mesh-warning-tooltip-de.png)

:::note[Bis der Schlüssel veröffentlicht ist]
Jede Box wird mit einer leeren Root-Schlüsseldatei ausgeliefert, bis das
Projekt den öffentlichen Schlüssel veröffentlicht. Bis dahin ist keine Edge
offiziell, jede Edge trägt das Warnzeichen, und der Markt ist auf jeder Box
aus. Das ist die sichere Richtung, in die es scheitern kann.
:::

Der Projektinhaber bewahrt den Root-Schlüssel offline auf, nie auf einer Box
oder einer Edge. Jede offizielle Edge hat einen eigenen Schlüssel, der die
Edge nie verlässt, und ein Zertifikat, das der Root-Schlüssel für sie
signiert hat.

Die Aufnahme bei der Edge, die entscheidet, welche Box durch sie tunneln
darf, ist eine eigene Sache. Die Edge führt eine Zulassungsliste von Boxen,
jede mit einem Token, und wer die Edge betreibt, trägt eine Box dort ein.

## Deine eigene Domain \{#your-own-domain}

Eine Box hinter der LosOS-Edge kann auch unter einer Domain antworten, die
ihr Besitzer schon hat, etwa `cloud.novak-family.eu`. Die Edge erledigt die
Arbeit, die das Internet braucht. Sie betreibt einen kleinen DNS-Server für
ihre eigene Zone, gibt der Box dort einen Namen, prüft die Einträge des
Besitzers und besorgt das Zertifikat.

Nur eine offizielle Edge vergibt Domains, und nur an eine Box, deren
Stripe-Konto Stripe vollständig geprüft hat. Das ist das Konto, an das der
Markt auszahlt. Es bindet die Box an eine Person, die Stripe verifiziert
hat, und ohne ein solches Konto kann sich kein Fremder die Zertifikate der
Edge ausleihen. Solange die Box kein solches Konto hat, sagt der Abschnitt
im Bereich Netzwerk, was fehlt, und bietet nichts zum Ausfüllen an.

![Der Abschnitt Eigene Domain im Bereich Netzwerk: der Name der Box auf der Edge, eine Domain aktiv, eine, die auf ihren CNAME wartet, mit den zwei zu veröffentlichenden Einträgen, und das Feld, um eine weitere hinzuzufügen.](@site/docs/img/custom-domain-de.png)

Sobald das Konto bereit ist, zeigt der Bereich Netzwerk **Name dieser Box
auf dem Edge**, etwa `3f9a1c0e7b2d4a55.boxes.losos.dasmat.us`. Der Name wird
aus der Identität der Box gebildet, verrät sie aber nicht. So fügst du eine
Domain hinzu:

1. Gib die Domain unter **Eine eigene Domain hinzufügen** ein und drück
   **Hinzufügen**.
2. Trag bei der Firma, bei der du die Domain gekauft hast, die beiden
   Einträge ein, die der Bereich darunter zeigt. Der eine ist ein **CNAME**
   von deiner Domain auf den Namen der Box auf der Edge. Der andere ist ein
   **TXT**-Eintrag unter `_losos-challenge.<your domain>` mit dem angezeigten
   Wert. Dieser Wert beweist, dass die Domain dir gehört. Er wird aus der
   Domain, der Identität der Box und ihrem Stripe-Konto gebildet und verrät
   nichts davon.
3. Warte. Die Edge schlägt die Einträge jede halbe Minute selbst nach und
   markiert jeden mit **✓ gefunden**, sobald sie ihn findet. Wenn beide
   stimmen, wird die Domain **Aktiv**, die Edge holt ein Zertifikat für sie,
   und LosOS cloud antwortet unter `https://<your domain>/nextcloud`.

:::note[Eine Domain, vor der nichts steht]
Eine Domain, vor der nichts steht, etwa `novak-family.eu` selbst, kann keinen
CNAME tragen. Verwende stattdessen die A- und AAAA-Adressen, die der Bereich
auflistet.
:::

Eine Box kann bis zu fünf Domains haben, und eine Domain gehört immer nur zu
einer Box. Entfernst du sie im Bereich, hört die Edge beim nächsten Durchlauf
auf, sie auszuliefern. Die Edge prüft eine aktive Domain jede Stunde erneut.
Verschwinden die Einträge, liefert sie die Domain nach drei fehlgeschlagenen
Prüfungen in Folge nicht mehr aus, damit ein einzelner DNS-Aussetzer keine
Website lahmlegt. Ist das Stripe-Konto nicht mehr bereit, hört sie sofort
auf. In beiden Fällen sagt der Bereich, warum.
LosOS Git baut seine Links weiterhin mit dem Edge-Namen der Box.

Steht deine Box zu Hause hinter deiner eigenen Edge, funktionieren deine
Domains trotzdem, solange die Box dem Mesh beigetreten ist. Die LosOS-Edge
schickt den Verkehr an deine Edge, und deine Edge reicht ihn an die Box
weiter. Deine Box teilt der LosOS-Edge mit, hinter welcher Heim-Edge sie
steht, sodass keine fremde Edge deine Domain beanspruchen kann. Dafür musst
du nichts einrichten.

## Einstellungen, auf die es ankommt \{#settings-that-matter}

| Bereich   | Einstellung                            | Was sie bewirkt                                                         |
| --------- | -------------------------------------- | ----------------------------------------------------------------------- |
| Netzwerk  | Auch von außerhalb erreichbar          | öffnet den Tunnel zur Edge                                              |
| Netzwerk  | Eigene Domain                          | fügt eine Domain hinzu, die dir gehört; braucht eine offizielle Edge und ein Stripe-Konto, das Stripe geprüft hat |
| Erweitert | `losos.proxy.registrarUrl`             | die Adresse der Edge, die neben dem lokalen Netz über das Internet gefragt wird |
| Mesh      | Dem Mesh beitreten                     | nimmt diese Box in den Cluster der Edge auf; braucht eine erreichbare Edge, den Tunnel an und einen eigenen Namen |
| Mesh      | Diese Box verleihen, während ich schlafe | verleiht CPU innerhalb der Stunden darunter, bei Leerlauf; braucht den Beitritt zum Mesh |
| Mesh      | Stunden                                | das tägliche Zeitfenster, in der eigenen Zeitzone dieser Box            |
| Markt     | Die Festplatte dieser Box mit dem Mesh teilen | leiht dem Pool freien Platz; braucht eine erreichbare Edge       |

## Was die Edge sieht \{#what-the-edge-sees}

Die Edge sieht den Namen der Box, ihren Heartbeat, welche Funktionen sie
eingeschaltet hat, und den Verkehr, den sie durch den Tunnel leitet. Dieser
Verkehr ist durchgehend verschlüsselt, mit einem Schlüssel, den sich die Box
beim ersten Kontakt merkt. Die Edge sieht weder die Admin-Seiten noch das
Passwort des Besitzers noch den Festplattenschlüssel. Das
[Sicherheitsmodell](../reference/security-model) listet die
Vertrauensgrenzen auf.

:::note[Die Demo-Edge]
Für Präsentationen läuft die Control Plane der Edge auch auf Vercel unter
`losos-edge.dasmat.us`, mit ihrem Verzeichnis in einer kleinen Datenbank.
Dieser Host kann Boxen registrieren und auf einer Statusseite zeigen, aber
keinen Tunnel tragen, kein Mesh betreiben und keine Zahlungen annehmen;
dafür braucht es den echten Edge-Server.
:::
