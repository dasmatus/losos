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

Hast du deinem Konto eine Authenticator-App oder einen Passkey hinzugefügt,
fragt LosOS Login nach dem Passwort danach. Siehe [Dein Konto](#your-account).

Eine Anmeldung gilt 12 Stunden und endet früher, wenn zwei Stunden lang
nichts passiert. Setzt du bei LosOS Login den Haken, damit es dich behält,
bleibst du an diesem Rechner einen Monat angemeldet. **Abmelden** in den
Admin-Seiten beendet sie sofort.

:::note[Falsche Versuche]
Nach einigen falschen Passwörtern hintereinander weist LosOS Login diesen
Namen ein paar Minuten lang ab. Warte und versuch es dann noch einmal.
:::

## LosOS cloud und LosOS Git \{#losos-cloud-and-losos-git}

LosOS cloud und LosOS Git melden dich ebenfalls über LosOS Login an, im
eigenen Netz der Box und über das Internet gleichermaßen. Öffnest du eine
der beiden abgemeldet, landest du bei LosOS Login; meldest du dich dort
einmal an, lassen dich beide Apps herein, ohne zweites Passwort. Keine der
beiden Apps zeigt eine eigene Anmeldeseite.

Wer die Box eingerichtet hat, kommt in beiden Apps als das Konto des
Eigentümers an, das die Dateien und Repositorys von vorher hält. Alle
anderen bekommen in jeder App beim ersten Öffnen ein eigenes Konto, benannt
nach ihrem Anmeldenamen. Beide Apps zeigen den vollständigen Namen und die
E-Mail-Adresse aus LosOS Login und ziehen nach, wenn du sie unter
[Dein Konto](#your-account) änderst.

Über das Internet antwortet LosOS Login unter der öffentlichen Adresse der
Box: ihrem Namen am Edge oder einer eigenen Domain, sobald sie zur Box
führt. Gleich nach einem Neustart, solange LosOS Login noch startet, zeigen
die Apps eine Seite, die das sagt, und versuchen es alle paar Sekunden von
selbst erneut.

Desktop- und Handy-Apps und `git` nutzen weiter App-Passwörter und Tokens.
Der Netzwerkordner von LosOS cloud nimmt im Netz der Box deinen Namen und
dein Passwort von LosOS Login, über das Internet ein App-Passwort; siehe
[Dateien auf deinem Computer](files-on-your-computer).

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
Benutzer, Erweitert, Historie, Über und Zurücksetzen öffnen, die Box löschen oder
wiederherstellen und den Wiederherstellungscode sehen. Wer kein
Administrator ist, bekommt die Bereiche, die ihm gegeben wurden, oder liest
alle Bereiche, ohne etwas ändern zu können. Die Übersicht sieht jeder, der
angemeldet ist. Die Admin-Seiten blenden die Bereiche aus, die eine Person
nicht benutzen kann.

Ein weiterer Teil, **Eigenes Konto**, öffnet den Bereich **Dein Konto**, in
dem eine Person ihren eigenen Namen, ihre E-Mail-Adresse, ihr Passwort und
ihre zweiten Faktoren pflegt. Neue Personen bekommen ihn von Anfang an.
Ohne ihn kann eine Person weder eine Authenticator-App noch einen Passkey
hinzufügen, und ein Administrator ändert ihr Passwort.

## Personen hinzufügen \{#adding-people}

Die erste Person auf einer Box ist immer Administrator. Alle weiteren fügt
ein Administrator unter **Einstellungen > Benutzer** hinzu. Den Bereich gibt
es, wenn die Box Personen selbst mit LosOS Login anmeldet. Eine Box, deren
Anmeldung woanders läuft, hat keinen Bereich Benutzer.

Um jemanden hinzuzufügen, füll **Person hinzufügen** aus: einen Anmeldenamen,
den vollständigen Namen, ein Passwort nach denselben Regeln wie deins und den
Zugriff.

- **Ausgewählte Teile**: sieht und ändert nur die eingeschalteten Teile, zum
  Beispiel Speicher und Sicherung.
- **Sieht alles**: sieht jeden Bereich und ändert nur die eingeschalteten
  Teile.
- **Administrator**: darf alles ändern, auch wer sich anmelden kann.

![Der Bereich Benutzer: drei Personen mit ihrem Zugriff, darunter das Formular Person hinzufügen.](@site/docs/img/settings-users-de.png)

Bei einer neuen Person ist **Eigenes Konto** eingeschaltet. Gib Namen und Passwort selbst weiter. Die Anmeldung läuft über LosOS Login.
LosOS cloud und LosOS Git legen beim ersten Öffnen jeweils ein eigenes Konto
an.

Wer eine Authenticator-App oder einen Passkey hat, trägt das Abzeichen
**Zwei-Faktor**. Wen eine Anmelderegel anhält, trägt das Abzeichen
**Angehalten**, mit dem Grund unter dem Namen.

Jede Person in der Liste hat diese Knöpfe:

- **Zugriff** ändert, was die Person tun darf. Das gilt ab dem nächsten
  Klick, ohne neue Anmeldung.
- **Neues Passwort** ist für jemanden, der seins verloren hat. Das alte gilt
  sofort nicht mehr.
- **Zwei-Faktor zurücksetzen** entfernt Authenticator-App und Passkeys,
  für jemanden, der Handy oder Schlüssel verloren hat. Die Person meldet
  sich dann nur mit dem Passwort an, bis sie neue hinzufügt. Den Knopf gibt
  es nur bei jemandem, der welche hat.
- **Entfernen** beendet sofort die Anmeldung an den Admin-Seiten und an
  beiden Apps. Konten und Dateien in LosOS cloud und LosOS Git bleiben auf
  der Box.

Deine eigene Zeile hat keine Knöpfe. Die Zeile des Besitzers hat nur
**Zwei-Faktor zurücksetzen**, für einen anderen Administrator. Den Besitzer
kann hier niemand entfernen, seinen Zugriff ändern oder sein Passwort
setzen; bei einem verlorenen Besitzer-Passwort siehe
[Passwort vergessen](../troubleshooting/forgot-password). Die Box behält
immer mindestens einen Administrator.

## Dein Konto \{#your-account}

**Einstellungen > Dein Konto** ist die eigene Seite jeder Person. Es gibt
sie für alle mit dem Teil Eigenes Konto, auch für Administratoren, und nie
für den Ersatzschlüssel, der kein Konto hat.

![Dein Konto: deine Angaben, dein Passwort, die Zwei-Faktor-Anmeldung und der Einmalcode.](@site/docs/img/settings-account-de.png)

- **Deine Angaben**: vollständiger Name und E-Mail-Adresse. LosOS cloud und
  LosOS Git zeigen dieselben. Nur ein Administrator kann deinen
  Anmeldenamen ändern.
- **Passwort**: wann du es zuletzt geändert hast und wann es abläuft, wenn
  die Box diese Regel hat. Ein neues Passwort folgt denselben Regeln wie das
  erste und öffnet auch LosOS cloud und LosOS Git.
- **Zwei-Faktor-Anmeldung**: ob du eine Authenticator-App hast und wie
  viele Passkeys. **LosOS Login öffnen** öffnet die eigene Seite von LosOS
  Login in einem neuen Tab, wo du sie hinzufügst oder entfernst. Mit einem
  davon fragt LosOS Login bei jeder Anmeldung nach dem Passwort danach.
- **Einmalcode**: LosOS Login verlangt einen Code, bevor es deine zweiten
  Faktoren ändert, und würde ihn sonst per E-Mail schicken. Die Box
  verschickt keine E-Mails, also fordere den Code auf der Seite von LosOS
  Login an, komm hierher zurück, gib dein Passwort ein und wähle **Code
  zeigen**.

## Anmelderegeln \{#sign-in-rules}

Ein Administrator setzt zwei Regeln für alle unter **Einstellungen >
Benutzer > Anmelderegeln**:

- **Zwei-Faktor-Anmeldung verlangen**: Alle fügen eine Authenticator-App
  oder einen Passkey hinzu.
- **Passwörter laufen ab**: Ein Passwort gilt die eingestellte Zahl von
  Tagen, von 1 bis 1200, gezählt ab dem Zeitpunkt, an dem die Person es
  zuletzt gesetzt hat.

Wen eine Regel erfasst, der ist angehalten. Die Person meldet sich weiter
an, aber die Admin-Seiten zeigen ihr nur **Dein Konto**, mit einem Hinweis,
was zu tun ist, und LosOS Login hält sie aus LosOS cloud und LosOS Git
heraus. Sobald sie ein neues Passwort wählt oder einen zweiten Faktor
hinzufügt, öffnet sich der Rest wieder. Nach einem neuen zweiten Faktor
fragt LosOS Login einmal danach, bevor die nächste Seite kommt. Wer
angehalten ist und LosOS cloud oder LosOS Git öffnet, sieht eine Seite, die
zum eigenen Konto schickt; das öffnet sich im eigenen Netz der Box.

Bevor du die Zwei-Faktor-Anmeldung verlangst, füge deinem eigenen Konto eine
Authenticator-App oder einen Passkey hinzu. Die Box lehnt die Regel ab,
solange du keinen hast, damit sie nicht auch dich anhält. Der
Ersatz-Admin-Schlüssel wird nie angehalten.

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

LosOS Login nimmt Passkeys an. Du fügst einen auf der Seite von LosOS Login
hinzu, die du aus [Dein Konto](#your-account) öffnest, über HTTPS, sobald
das Zertifikat der Box installiert ist. Auf der Anmeldeseite meldet dich ein
Handy oder Laptop dann mit seiner eigenen Entsperrung an statt mit Name und
Passwort, für die Admin-Seiten, LosOS cloud und LosOS Git gleichermaßen. Ein
Passkey ist kein zweiter Ersatzschlüssel: Er öffnet nur dein eigenes Konto.

## Das Zeitfenster zum Beanspruchen \{#the-claim-window}

Eine frische Box hat keinen Besitzer. Der erste Browser im LAN, der den
Assistenten abschließt, wird der Besitzer und ihr erster Administrator, und
das Fenster schließt sich. Führe den Assistenten also bald nach der
Installation aus, in einem Netz, dem du vertraust. Nichts außerhalb des LAN
kann eine Box beanspruchen, und erst eine Neuinstallation erlaubt es wieder.
