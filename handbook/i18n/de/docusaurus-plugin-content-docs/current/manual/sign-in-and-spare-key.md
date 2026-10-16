---
title: Anmeldung und der Ersatzschlüssel
sidebar_position: 2
slug: /manual/sign-in-and-spare-key
---

# Anmeldung und der Ersatzschlüssel

Jede Person meldet sich an den Admin-Seiten bei LosOS Login, der
Anmeldeseite der Box, mit ihrem eigenen Anmeldenamen und Passwort an. Die
Box hat außerdem einen Ersatz-Admin-Schlüssel, der bei der Einrichtung
einmal gezeigt wird.

## Dein Anmeldename und dein Passwort \{#your-sign-in-name-and-password}

Der Passwort-Schritt des Assistenten fragt nach einem Anmeldenamen und
einem Passwort. Wer ihn abschließt, wird der erste Administrator der Box.
Dasselbe Passwort wird auch das Passwort des Besitzers für LosOS cloud, ein
Passwort öffnet also beides.

Ein Anmeldename ist kleingeschrieben. Er beginnt mit einem Buchstaben,
danach folgen Buchstaben, Ziffern, `.`, `_` oder `-`, höchstens 32 Zeichen.
Ein neues Passwort braucht mindestens 12 Zeichen, einen Kleinbuchstaben,
einen Großbuchstaben, eine Ziffer und ein Sonderzeichen.

Die Box behält jedes Passwort nur als Hash. Einen Link zum Zurücksetzen gibt
es nicht, weil die Box keine Mails verschickt. Verlierst du deins, sieh
unter [Passwort vergessen](../troubleshooting/forgot-password) nach.

## LosOS Login \{#losos-login}

LosOS Login ist die Anmeldeseite der Box unter `https://<box>/auth/`, in den
Farben von LosOS. Öffnest du die Admin-Seiten abgemeldet, hat ein Dialog mit
dem Titel **An dieser Box anmelden** eine Schaltfläche **Anmelden**. Sie
bringt dich zu LosOS Login und nach der Anmeldung zurück. Solange LosOS
Login noch startet, gleich nach einem Neustart, zeigen die Admin-Seiten
„LosOS Login startet“ mit einem Ladekreis und ein paar Fakten über LosOS und
gehen dann von selbst weiter.

![Der Dialog An dieser Box anmelden mit der Schaltfläche Anmelden und dem Link zum Ersatz-Admin-Schlüssel.](@site/docs/img/unlock-dialog-de.png)

LosOS Login funktioniert nur über HTTPS. Eine einfache
`http://`-Adresse dafür wird an `https://` weitergeleitet. Solange das
Zertifikat der Box nicht installiert ist, warnt der Browser zuerst; siehe
[Zertifikatswarnung](../troubleshooting/certificate-warning).

Eine Anmeldung gilt 12 Stunden und endet früher, wenn zwei Stunden lang
nichts passiert. Setzt du bei LosOS Login den Haken, damit es dich behält,
bleibst du an diesem Rechner einen Monat angemeldet. **Abmelden** in den
Admin-Seiten beendet sie sofort.

:::note[Falsche Versuche]
Nach einigen falschen Passwörtern hintereinander weist LosOS Login diesen
Namen ein paar Minuten lang ab. Warte und versuch es dann noch einmal.
:::

## LosOS cloud und LosOS Git \{#losos-cloud-and-losos-git}

Im eigenen Netz der Box melden dich LosOS cloud und LosOS Git ebenfalls
über LosOS Login an. Öffnest du eine der beiden abgemeldet, landest du bei
LosOS Login; meldest du dich dort einmal an, lassen dich beide Apps herein,
ohne zweites Passwort.

Wer die Box eingerichtet hat, kommt in beiden Apps als das Konto des
Eigentümers an, das die Dateien und Repositorys von vorher hält. Alle
anderen bekommen in jeder App beim ersten Öffnen ein eigenes Konto, benannt
nach ihrem Anmeldenamen.

Die Apps behalten ihre eigene Anmeldeseite dort, wo LosOS Login nicht helfen
kann:

- über das Internet (die öffentliche Adresse der Box), wo LosOS Login nicht
  antwortet;
- solange LosOS Login nach einem Neustart noch startet;
- in LosOS cloud unter `/nextcloud/login?direct=1`, wenn du danach fragst.

Dort meldet sich der Eigentümer wie bisher mit dem eigenen Passwort an. Desktop-
und Handy-Apps, WebDAV und `git` nutzen weiter App-Passwörter und Tokens.

**Abmelden** in LosOS cloud meldet dich auch bei LosOS Login ab. **Abmelden**
in LosOS Git tut das nicht: Solange du bei LosOS Login angemeldet bist,
meldet LosOS Git dich gleich wieder an. Melde dich bei LosOS Login ab (in den
Admin-Seiten oder in LosOS cloud), um beide zu verlassen. Ein schon offener
App-Tab bleibt angemeldet, bis du dich auch in dieser App abmeldest.

## Wer was sieht \{#who-sees-what}

Die Admin-Seiten sind in Bereiche aufgeteilt: Speicher, Sicherung, Apps,
Markt (mit virtuellen Maschinen), Netzwerk (eigene Domains), Aussehen, das
Labor und Zugriffstokens. Ein Administrator kann alle benutzen. Nur ein
Administrator kann Einstellungen anwenden, Mesh, Hardware, Sicherheit,
Erweitert, Historie, Über und Zurücksetzen öffnen, die Box löschen oder
wiederherstellen und den Wiederherstellungscode sehen. Wer kein
Administrator ist, bekommt die Bereiche, die ihm gegeben wurden, oder liest
alle Bereiche, ohne etwas ändern zu können. Die Übersicht sieht jeder, der
angemeldet ist. Die Admin-Seiten blenden die Bereiche aus, die eine Person
nicht benutzen kann.

Die erste Person auf einer Box ist immer Administrator. Eine Seite zum
Hinzufügen weiterer Personen haben die Admin-Seiten noch nicht.

## Der Ersatz-Admin-Schlüssel \{#the-spare-admin-key}

Die Box hat bei ihrem ersten Start einen Schlüssel mit 64 Zeichen erzeugt.
Der Assistent hat ihn dir einmal gezeigt, nachdem du das Passwort gesetzt
hattest, mit den Schaltflächen **Schlüssel kopieren** und **Drucken**. Der
Schlüssel öffnet jeden Bereich als Administrator, ohne LosOS Login. Er
funktioniert also auch an einem Tag, an dem LosOS Login nicht funktioniert.

Wähle im Dialog **An dieser Box anmelden** die Option **Stattdessen den
Ersatz-Admin-Schlüssel verwenden** und füge den Schlüssel ein. Der Tab
bleibt entsperrt, bis du ihn schließt.

![Der Dialog fragt nach dem Ersatz-Admin-Schlüssel mit 64 Zeichen.](@site/docs/img/unlock-spare-key-de.png)

![Ein falscher Ersatzschlüssel: Der Dialog sagt, dass der Schlüssel nicht akzeptiert wurde, und bleibt offen.](@site/docs/img/unlock-refused-de.png)

Nach zehn falschen Schlüsseln von einem Rechner sperrt die Box diesen
Rechner aus. Die Sperre beginnt bei einer Sekunde und verdoppelt sich bis zu
fünf Minuten.

:::warning[Ersatzschlüssel nicht bei der Box aufbewahren]
Bewahre das gedruckte Blatt nicht bei der Box auf. Wer den Schlüssel hat,
kann jede Einstellung ändern, und das kommt root gleich. Die Box kann den
Schlüssel weder noch einmal anzeigen noch austauschen, weil sie keine Shell
hat. Verlierst du das Passwort *und* den Ersatzschlüssel, musst du vom Stick
neu installieren, und das löscht die Festplatte.
:::

## Eine früher eingerichtete Box \{#a-box-set-up-earlier}

Eine Box, die eingerichtet wurde, bevor Personen eigene Anmeldenamen hatten,
hat niemanden, als den man sich anmelden könnte. Ihr Dialog sagt **Lege
deine Anmeldung an** und fragt nach einem Anmeldenamen, einem Passwort und
dem Ersatz-Admin-Schlüssel. Der Schlüssel beweist, dass die Box dir gehört,
und du wirst ihr erster Administrator. Ab dann meldest du dich mit diesem
Namen und Passwort an.

## Passkeys \{#passkeys}

Sobald das Zertifikat installiert ist und du über HTTPS verbindest, bietet
LosOS cloud einen Passkey an. Ein Handy oder Laptop meldet sich dann bei
LosOS cloud mit seiner eigenen Entsperrung an statt mit dem Passwort. LosOS
Login nimmt keine Passkeys an und fragt immer nach Name und Passwort. Ein
Passkey ist kein zweiter Ersatzschlüssel.

## Das Zeitfenster zum Beanspruchen \{#the-claim-window}

Eine frische Box hat keinen Besitzer. Der erste Browser im LAN, der den
Assistenten abschließt, wird der Besitzer und ihr erster Administrator, und
das Fenster schließt sich. Führe den Assistenten also bald nach der
Installation aus, in einem Netz, dem du vertraust. Nichts außerhalb des LAN
kann eine Box beanspruchen, und erst eine Neuinstallation erlaubt es wieder.
