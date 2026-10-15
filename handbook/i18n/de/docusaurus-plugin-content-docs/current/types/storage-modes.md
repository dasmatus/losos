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

![Der Bereich Speicher auf einer Box, die ihre Festplatte für sich behält: belegter Platz, freier Platz und die Reserve, mit Reserve nutzen.](@site/docs/img/storage.png)

- der **belegte Platz** von LosOS cloud, LosOS Git und dem System;
- die **Reserve**, der Teil, den der Installer zurückgehalten hat. Der
  Bereich Speicher kann ihn jederzeit belegen; siehe
  [Festplatte vergrößern](../manual/disk-growth).

## Mit dem Mesh geteilt \{#shared-with-the-mesh}

Die Box leiht dem Mesh Platz, den sie nicht nutzt. Dieser Platz wird Teil
eines replizierten Pools, den Longhorn über die Boxen einer Edge hinweg
betreibt. Im Gegenzug kann die Box selbst Platz im Pool belegen. Drei Dinge
gelten:

![Das Mesh: Drei Boxen behalten jeweils die Dateien ihres Besitzers für sich und leihen den Platz, den sie nicht nutzen, einem gemeinsamen replizierten Pool; darunter ein Zeitfenster von 23:00 bis 07:00, in dem freie Zeit verliehen wird, nur bei Leerlauf.](@site/docs/img/mesh-de.svg)

![Der Bereich Speicher auf einer Box, die teilt: der an das Mesh verliehene Platz schraffiert neben dem eigenen der Box.](@site/docs/img/storage-shared.png)

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

## Freie Zeit verleihen (Rechenleistung) \{#lending-spare-time-compute}

Das ist von der Festplatte getrennt und liegt im Bereich Mesh. Eine Box
führt Arbeit anderer Leute nur aus, wenn beides gilt:

![Der Bereich Mesh mit beigetretener Box: Diese Box verleihen, während ich schlafe ist eingeschaltet, und die Stunden sind auf 23:00 bis 07:00 gesetzt.](@site/docs/img/mesh-two-edges.png)

1. die Uhrzeit liegt im Zeitfenster, das du festgelegt hast, einem Beginn
   und einem Ende in deiner Zeitzone;
2. die Box ist untätig.

Auslastung kann das Zeitfenster früher schließen; außerhalb der Stunden kann
nichts es öffnen. Alles Unbekannte zählt als ausgelastet. Die Edge beurteilt
das Zeitfenster in der eigenen Zeitzone deiner Box, sodass ein Fenster von
23:00 bis 07:00 auch über die Umstellung auf Sommer- oder Winterzeit hinweg
23:00 bis 07:00 bleibt.
