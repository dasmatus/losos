---
title: Speichermodi
sidebar_position: 6
---

# Speichermodi

Die Speicherkarte der Übersicht zeigt einen von zwei Zuständen, und der
Bereich Über zeigt dasselbe als Plakette: **nur für sich**, gefüllt
gezeichnet, oder **mit dem Mesh geteilt**, schraffiert gezeichnet. Wo sich
zwei Dinge durch die Textur statt durch die Farbe unterscheiden, verwenden
die Admin-Seiten dasselbe Paar: gefüllt für diese Box, schraffiert für das
Mesh.

## Nur für sich (Standard) \{#kept-to-itself-default}

Alles, was du auf der Box ablegst, bleibt auf der Box. Die Box kopiert nichts
woandershin. Die Festplatte hat zwei Teile:

![Der Bereich Speicher: die Festplatte der Box und der für später zurückgehaltene Platz, mit Reserve nutzen.](@site/docs/img/storage-de.png)

- der **belegte Platz** von LosOS Cloud, LosOS Git und dem System;
- die **Reserve**, der Teil, den der Installer zurückgehalten hat. Der
  Bereich Speicher kann ihn jederzeit belegen; siehe
  [Festplatte vergrößern](../manual/disk-growth).

## Mit dem Mesh geteilt \{#shared-with-the-mesh}

Die Box leiht dem Mesh Platz, den sie nicht nutzt. Dieser Platz wird Teil
eines replizierten Pools, den Longhorn über die Boxen einer Edge hinweg
betreibt. Im Gegenzug kann die Box selbst Platz im Pool belegen. Drei Dinge
gelten:

![Das Mesh: Drei Boxen behalten jeweils die Dateien ihres Besitzers für sich und leihen den Platz, den sie nicht nutzen, einem gemeinsamen replizierten Pool; darunter ein Zeitfenster von 23:00 bis 07:00, in dem freie Zeit verliehen wird, nur bei Leerlauf.](@site/docs/img/mesh-de.svg)

![Der Bereich Speicher auf einer Box, die teilt: der an das Mesh verliehene Platz schraffiert neben dem eigenen der Box.](@site/docs/img/storage-shared-de.png)

- Geteilte Daten liegen in einem eigenen Konto auf der Box, `shared`, in
  einem Verzeichnis, das mit einem im TPM versiegelten Schlüssel
  verschlüsselt ist. Ist das Teilen aus, lädt die Box den Schlüssel nicht,
  und das Verzeichnis ist selbst für die Box unlesbar. Deine eigenen Dateien
  unter `notshared` und die geteilten Daten können einander nicht lesen.
- Teilen braucht eine erreichbare Edge. Ohne eine verweigert die Box den
  Schalter.
- Der Schalter sitzt im Bereich **Markt**, weil Festplatte verleihen und
  dafür bezahlt werden eine einzige Entscheidung ist. Er funktioniert
  unabhängig davon, ob die Edge dieser Box den Markt anbietet.

## Nur mit diesem Netzwerk geteilt \{#shared-with-this-network-only}

Zwischen „nur für sich“ und „mit dem Mesh geteilt“ gibt es eine dritte
Wahl, für eine Box im Netz einer Firma oder in einem Zuhause mit eigenem
Gateway: mit den lokalen Gateways dieses Netzwerks teilen und mit nichts
darüber hinaus.

- Die Daten liegen in einem dritten Konto, `kindashared`, in einem eigenen
  Verzeichnis mit eigenem Schlüssel verschlüsselt. Es öffnet sich nur,
  solange diese Art des Teilens an ist. `notshared`, `shared` und
  `kindashared` können einander nicht lesen.
- Es braucht ein erreichbares **lokales Gateway**: einen Edge, den die Box
  in ihrem eigenen Netzwerk erreicht und dessen Verbindung sekundär ist
  (siehe [Edge-Föderation](../in-depth/edge-federation)). Ein offizieller
  Edge allein reicht nicht, und die Box lehnt den Schalter ab.
- Kein Markt. Nichts, was `kindashared` hält, wird angeboten, verkauft oder
  bezahlt, und eine Box, die nur so teilt, kann weder Platz noch Zeit auf
  den Markt bringen. Kaufen auf dem Markt geht weiter.
- Antwortet zehn Minuten lang kein lokales Gateway, schaltet die Box es
  selbst aus und eine Minute, nachdem wieder eines antwortet, wieder ein.
- Solange es an ist und die Box nicht im Mesh ist, laufen Chat, Mail,
  Anrufe und Mesh CI als `kindashared`, mit ihren Daten in dessen
  Verzeichnis. Auch Apps lassen sich als `kindashared` installieren. Tritt
  die Box später dem Mesh bei, laufen sie als `shared` und fangen dort neu
  an; was `kindashared` hält, bleibt in seinem eigenen Verzeichnis.
- Der Schalter **Mit den Gateways dieses Netzwerks teilen** ist im Bereich
  **Mesh**, zusammen mit den erreichbaren Gateways und dem Konto, als das
  die geteilte Seite läuft.

## Freie Zeit verleihen (Rechenleistung) \{#lending-spare-time-compute}

Das ist von der Festplatte getrennt und liegt im Bereich Mesh. Eine Box
führt Arbeit anderer Leute nur aus, wenn beides gilt:

![Der Bereich Mesh mit beigetretener Box: Diese Box verleihen, während ich schlafe ist eingeschaltet, und die Stunden sind auf 23:00 bis 07:00 gesetzt.](@site/docs/img/mesh-two-edges-de.png)

1. die Uhrzeit liegt im Zeitfenster, das du festgelegt hast, einem Beginn
   und einem Ende in deiner Zeitzone;
2. die Box ist untätig.

Auslastung kann das Zeitfenster früher schließen; außerhalb der Stunden kann
nichts es öffnen. Alles Unbekannte zählt als ausgelastet. Die Edge beurteilt
das Zeitfenster in der eigenen Zeitzone deiner Box, sodass ein Fenster von
23:00 bis 07:00 auch über die Umstellung auf Sommer- oder Winterzeit hinweg
23:00 bis 07:00 bleibt.
