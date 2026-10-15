---
title: Anmeldung und der Ersatzschlüssel
sidebar_position: 2
slug: /manual/sign-in-and-spare-key
---

# Anmeldung und der Ersatzschlüssel

Eine Box hat ein Passwort und einen Ersatzschlüssel.

## Das Passwort \{#the-password}

Das im Assistenten gesetzte Passwort ist das Passwort des Besitzers für
LosOS cloud. Es entsperrt auch die Admin-Seiten. Öffnest du sie, fragt ein
Dialog mit dem Titel **Diese Box entsperren** danach, und die Box lässt
LosOS cloud es über ihre eigene Loopback-Verbindung prüfen. Eine zweite
Kopie behält die Box nicht. Ändere das Passwort in LosOS cloud unter
Einstellungen → Persönlich → Sicherheit, und die Admin-Seiten nehmen das neue
sofort an.

![Der Dialog Diese Box entsperren fragt nach dem Passwort des Besitzers, mit dem Link zum Ersatz-Admin-Schlüssel.](@site/docs/img/unlock-dialog.png)

Ein neues Passwort braucht mindestens 12 Zeichen, einen Kleinbuchstaben,
einen Großbuchstaben, eine Ziffer und ein Sonderzeichen. Ein Passwort, das
gesetzt wurde, bevor es diese Regel gab, funktioniert bei der Anmeldung
weiterhin.

![Ein falsches Passwort: Der Dialog sagt, dass das Passwort nicht akzeptiert wurde, und bleibt offen.](@site/docs/img/unlock-refused.png)

Der Tab bleibt entsperrt, bis du ihn schließt. Nach zehn falschen Versuchen
von einem Rechner sperrt die Box diesen Rechner aus. Die Sperre beginnt bei
einer Sekunde und verdoppelt sich bis zu fünf Minuten.

## Der Ersatz-Admin-Schlüssel \{#the-spare-admin-key}

Was die Admin-Seiten tatsächlich entsperrt, ist ein Schlüssel mit 64
Zeichen, den die Box bei ihrem ersten Start erzeugt hat. Das Passwort ist
der Weg, an ihn zu kommen. Der Assistent hat dir diesen Schlüssel einmal
gezeigt, nachdem du das Passwort gesetzt hattest, mit den Schaltflächen
**Schlüssel kopieren** und **Drucken**. Er ist der Ersatz für den einen
Fall, den das Passwort nicht abdeckt. Läuft LosOS cloud nicht, kann nichts
das Passwort prüfen. Der Dialog zum Entsperren sagt das dann und bietet
**Stattdessen den Ersatz-Admin-Schlüssel verwenden** an.

![Der Dialog zum Entsperren, während LosOS cloud nicht läuft: Nichts kann das Passwort prüfen, also bietet der Dialog den Ersatz-Admin-Schlüssel an.](@site/docs/img/unlock-dialog-cloud-down.png)

Bewahre das gedruckte Blatt nicht bei der Box auf. Wer den Schlüssel hat,
kann jede Einstellung ändern, und das kommt root gleich. Die Box kann den
Schlüssel weder noch einmal anzeigen noch austauschen, weil sie keine Shell
hat. Verlierst du das Passwort *und* den Ersatzschlüssel, musst du vom Stick
neu installieren, und das löscht die Festplatte.

![Der Dialog zum Entsperren fragt nach dem Ersatz-Admin-Schlüssel mit 64 Zeichen.](@site/docs/img/unlock-spare-key.png)

## Passkeys \{#passkeys}

Sobald das Zertifikat installiert ist und du über HTTPS verbindest, bieten
der Assistent und LosOS cloud einen Passkey an. Ein Handy oder Laptop meldet
sich dann mit seiner eigenen Entsperrung an statt mit dem Passwort. Ein
Passkey ist kein zweiter Ersatzschlüssel. LosOS cloud prüft ihn genauso, wie
es das Passwort prüft.

## Das Zeitfenster zum Beanspruchen \{#the-claim-window}

Eine frische Box hat keinen Besitzer. Der erste Browser im LAN, der den
Assistenten abschließt, wird der Besitzer, und das Fenster schließt sich.
Führe den Assistenten also bald nach der Installation aus, in einem Netz,
dem du vertraust. Nichts außerhalb des LAN kann eine Box beanspruchen, und
erst eine Neuinstallation erlaubt es wieder.
