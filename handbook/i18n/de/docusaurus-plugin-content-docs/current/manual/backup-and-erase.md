---
title: Sicherung und Löschen
sidebar_position: 8
slug: /manual/backup-and-erase
---

# Sicherung und Löschen

Eine Box kann alles, was sie enthält, in einen Speicher-Bucket kopieren, den
du mietest, und sie kann sich selbst vollständig löschen. Beides arbeitet
zusammen: sichern, löschen, und der Wiederherstellungscode holt alles
zurück, auf dieselbe Box oder auf eine neue.

## Der Bucket \{#the-bucket}

**Einstellungen → Sicherung** nimmt jeden S3-kompatiblen Bucket: Amazon S3
(auch Glacier), Backblaze B2, Wasabi, Cloudflare R2 oder ein MinIO im
eigenen Netz. Vom Anbieter brauchst du fünf Dinge:

- die **Adresse**, etwa `https://s3.eu-central-1.amazonaws.com`;
- den Namen des **Buckets**;
- einen **Ordner** darin, wenn du einen möchtest;
- die **Region**, wenn der Anbieter eine verlangt;
- einen **Zugriffsschlüssel** und seinen **geheimen Schlüssel**, die diesen
  Bucket lesen und beschreiben dürfen.

Die Adresse muss mit `https://` beginnen. Einfaches `http://` nimmt die Box
nur für eine Adresse im eigenen Netz an, etwa ein NAS. Nach dem Speichern
zeigt sie den geheimen Schlüssel nie wieder. Willst du den Bucket später
ändern, lass das Feld für den geheimen Schlüssel leer, dann bleibt der
gespeicherte erhalten.

![Einstellungen, Sicherung: wohin Sicherungen gehen, mit Adresse und Schlüsseln des Buckets, und der Wiederherstellungscode hinter Anzeigen.](@site/docs/img/settings-backup-de.png)

### Glacier bei Amazon S3 \{#glacier-on-amazon-s3}

Mit einer Adresse von Amazon S3 (`https://s3.<region>.amazonaws.com`) legt
**Speicherklasse** die Sicherungen in eine der Glacier-Speicherklassen. Sie
sind günstiger zu lagern und teurer zurückzulesen:

| Speicherklasse | Eine Wiederherstellung beginnt | Gut für |
|---|---|---|
| Standard | sofort | Sicherungen, die du an jedem Tag brauchen könntest |
| Glacier Instant Retrieval | sofort, AWS berechnet jeden Abruf | eine günstigere Kopie, die du selten liest |
| Glacier Flexible Retrieval | nach 3 bis 5 Stunden | eine Kopie für den Fall, dass die Box weg ist |
| Glacier Deep Archive | nach bis zu 12 Stunden | die günstigste Kopie, für den schlimmsten Tag |

![Einstellungen, Sicherung: das Formular für den Bucket mit Glacier Deep Archive unter Speicherklasse gewählt.](@site/docs/img/settings-backup-glacier-de.png)

Nur die Dateien kommen in Glacier. Der kleine Index, der die Sicherungen
auflistet, bleibt in Standard, damit die Box einen Wiederherstellungscode
weiterhin sofort prüfen und alte Sicherungen sofort aufräumen kann. Bei
Flexible Retrieval und Deep Archive bittet eine Wiederherstellung AWS zuerst,
die Sicherung bereitzustellen, und wartet; LosOS Cloud und LosOS Git laufen
weiter, bis die Dateien zurück sind. AWS berechnet die Bereitstellung, und
Glacier-Klassen haben außerdem eine Mindestspeicherdauer, sodass eine früh
aufgeräumte Sicherung trotzdem für diese Dauer berechnet wird.

:::warning[Keine Lifecycle-Regel auf dem Bucket]
Wähle die Klasse hier und nicht mit einer Lifecycle-Regel auf dem Bucket.
Eine Regel für den ganzen Bucket verschiebt auch den Index nach Glacier, und
dann lässt sich keine Sicherung mehr auflisten oder öffnen, bis AWS sie
bereitstellt.
:::

## Der Wiederherstellungscode \{#the-recovery-code}

Die Box verschlüsselt jede Sicherung, bevor sie die Box verlässt, mit dem
**Wiederherstellungscode** der Box als Schlüssel. Der Betreiber des Buckets
sieht nur Daten, die ohne den Code nicht lesbar sind, und niemand kann eine
Sicherung ohne den Code öffnen, auch LosOS nicht. **Anzeigen** unter *Der
Schlüssel zu deinen Sicherungen* zeigt ihn an. Schreib ihn irgendwo
außerhalb der Box auf.

## Was eine Sicherung enthält \{#what-a-backup-holds}

- die Dateien von LosOS Cloud;
- die Repositorys von LosOS Git;
- die Datenbanken von beiden;
- die Postfächer von LosOS Mail, wo die Box Mail hat (der Mailserver
  pausiert für die paar Sekunden der Kopie);
- den Ordner, den die Box mit dem Mesh teilt, wenn das Teilen an ist (er wird
  für die Kopie geöffnet und danach wieder gesperrt);
- jede Einstellung und das Aussehen der Startseite.

Vorschaubilder, die LosOS Cloud neu zeichnen kann, bleiben draußen. **Jetzt
sichern** startet eine Sicherung. Die erste kopiert alles, spätere schicken
nur, was sich geändert hat. Der Bucket behält die letzten sieben.

## Wiederherstellen \{#restoring}

Gib unter **Wiederherstellen** den Wiederherstellungscode der Box ein, die
die Sicherung gemacht hat, und drücke **Wiederherstellen…**. Die neueste
Sicherung im Bucket kommt zurück:

- was jetzt auf der Box ist, wird durch den Inhalt der Sicherung ersetzt;
- LosOS Cloud und LosOS Git halten an, während die Dateien zurückgelegt
  werden, und starten dann wieder. Aus Glacier Flexible Retrieval oder Deep
  Archive wird die Sicherung zuerst bereitgestellt, und das dauert Stunden;
- die Einstellungen aus der Sicherung werden angewendet, und dabei baut sich
  die Box neu auf;
- die Box behält ab dann den eingegebenen Code, sodass ihre nächste
  Sicherung an denselben Ort geht.

Melde dich danach mit dem Passwort an, das du beim Erstellen der Sicherung
hattest. Ein falscher Code schlägt mit „the recovery code does not open
this backup“ fehl, und auf der Box ändert sich nichts.

:::tip[Wiederherstellen auf einer neuen Box]
Auf einer gelöschten oder neu installierten Box führst du zuerst den
Assistenten aus, richtest dann unter Sicherung denselben Bucket ein und
stellst von dort wieder her.
:::

## Die Box löschen \{#erasing-the-box}

**Einstellungen → Zurücksetzen → Löschen…** löscht die Daten des Besitzers,
nicht nur die Einstellungen: Dateien, Repositorys, den geteilten Ordner,
jede Einstellung, das Passwort, den Ersatzschlüssel und den
Wiederherstellungscode. LosOS bleibt installiert, und die Box begrüßt die
nächste Person mit dem Einrichtungsassistenten.

![Der Dialog zum Löschen: was gelöscht wird, was außerhalb der Box aufgegeben wird, und der Schalter Vorher sichern.](@site/docs/img/settings-erase-dialog-de.png)

Der Reihe nach tut die Box Folgendes:

1. Sie sichert zuerst in den Bucket, wenn **Vorher sichern** an ist.
   Schlägt die Sicherung fehl, endet das Löschen, und nichts wird gelöscht.
2. Sie zählt herunter, fünfzehn Minuten, sofern `losos.reset.graceMinutes`
   im Bereich Erweitert nichts anderes sagt.
   **Löschen abbrechen** hält bis zum Ende des Countdowns alles an, und bis
   dahin hat sich nichts geändert.
3. Sie gibt auf, was sie außerhalb von sich selbst hält: ihre eigenen
   Domains, ihre Marktangebote und ihren Platz auf der Edge.
4. Sie setzt jede Einstellung zurück, startet neu und löscht die Daten früh
   beim nächsten Start, bevor irgendeine App läuft.

![Der Countdown mit Löschen abbrechen.](@site/docs/img/settings-erase-countdown-de.png)

:::danger[Ab Schritt 3 kein Zurück]
Ab Schritt 3 lässt sich das Löschen nicht mehr aufhalten. Einen Schritt, den
die Edge nicht bestätigt, vermerkt die Box, und das Löschen geht weiter.
Nach dem Neustart zeigt Einstellungen → Zurücksetzen als Zahlen, was das
letzte Löschen aufgegeben hat, und sagt, wenn die Edge einen Schritt nicht
bestätigt hat.
:::

Sicherungen, die schon im Bucket liegen, bleiben dort. Das Löschen rührt den
Bucket nie an.

## Löschen, Zurücksetzen oder Neuinstallation \{#erase-reset-or-reinstall}

| | Einstellungen | Dateien und Repositorys | Passwort und Codes | Außerhalb der Box |
|---|---|---|---|---|
| **Zurücksetzen** | auf Standardwerte | behalten | behalten | behalten |
| **Löschen** | auf Standardwerte | gelöscht | gelöscht | aufgegeben |
| **Neuinstallation** | auf Standardwerte | gelöscht | gelöscht | bleiben, wie sie waren |

[Zurücksetzen und Neuinstallation](./reset-and-reinstall) behandelt die
anderen beiden.
