---
title: Der erste Start
sidebar_position: 3
---

# Der erste Start

Wenn du die Adresse der Box zum ersten Mal öffnest, zeigen die Admin-Seiten
einen Assistenten mit drei Schritten. Geh ihn bald nach der Installation
durch, von einem Rechner in deinem eigenen Netzwerk aus. Wer den Assistenten
zuerst abschließt, dem gehört die Box.

## Schritt 1: Dieser Box vertrauen \{#step-1-trust-this-box}

Die Box liefert HTTPS mit einem Zertifikat aus, das sie selbst erstellt hat.
Dein Browser hat dieses Zertifikat noch nie gesehen und vertraut ihm deshalb
noch nicht. Alles nach diesem Schritt soll über die verschlüsselte
Verbindung laufen, auch das Passwort, das du gleich festlegst. Deshalb bittet
dich der Assistent, zuerst das Zertifikat zu installieren.

<img src={require("@site/docs/img/wizard-step1-trust-de.png").default} alt="Schritt 1 des Assistenten: die einzeiligen Installer für macOS, Linux und Windows, der Download des Zertifikats und der Fingerabdruck zum Vergleichen." width="520" />

Am schnellsten geht es mit einer Zeile im Terminal, angezeigt für den
Rechner, an dem du sitzt:

```bash title="macOS und Linux"
curl -fsSL http://<address>/setup/trust.sh | sh
```

```powershell title="Windows (PowerShell)"
irm http://<address>/setup/trust.ps1 | iex
```

Was diese Zeile tut und was nicht:

- Sie fügt **ein Zertifikat** zu den Speichern hinzu, die deine Browser
  lesen, nur für deinen Benutzer: dem Anmelde-Schlüsselbund unter macOS, dem
  Speicher für vertrauenswürdige Stammzertifizierungsstellen des Benutzers
  unter Windows, den NSS-Speichern, die Chrome und Firefox unter Linux
  verwenden. Sie installiert keine Software und fragt nie nach
  Administratorrechten. Wenn du diesen einen Eintrag löschst, ist alles
  rückgängig gemacht.
- Sie kommt **von der Box, in deinem eigenen Netzwerk**, nicht aus dem
  Internet. Sie ist kurz, und du kannst den Link in einem Tab öffnen und sie
  vorher lesen.
- Sie gibt den **Fingerabdruck** des Zertifikats aus, den du mit dem auf der
  Seite des Assistenten vergleichst.
- macOS fragt einmal nach deinem Kontopasswort. Es fragt der
  Schlüsselbund, nicht das Skript. Windows zeigt einen eigenen Dialog
  „Dieses Stammzertifikat installieren?“.

:::note[Das Zertifikat auf dem Handy]
Handys haben kein Terminal. Lade auf einem Handy `losos-ca.crt` von derselben
Seite herunter und füge es in den Einstellungen des Handys als
vertrauenswürdiges Zertifikat hinzu. Auf einem Rechner kannst du dasselbe
von Hand machen.
:::

Starte dann den Browser neu und öffne die Seite erneut unter
`https://<name>.local` oder `https://<address>`. Du kannst diesen Schritt
auch überspringen und später darauf zurückkommen. Wenn etwas schiefgeht,
siehe die Seite [Zertifikatswarnung](../troubleshooting/certificate-warning).

## Schritt 2: Wähle, wie du dich anmeldest \{#step-2-choose-how-you-sign-in}

Wähle einen **Anmeldenamen** und lege das **Passwort des Besitzers** fest.
Der Name ist kleingeschrieben: Er beginnt mit einem Buchstaben, danach
folgen Buchstaben, Ziffern, `.`, `_` oder `-`, höchstens 32 Zeichen. Das
Passwort braucht mindestens 12 Zeichen mit einem Kleinbuchstaben, einem
Großbuchstaben, einer Ziffer und einem Sonderzeichen wie `-` oder `!`. Der
Assistent hakt jede Regel ab, während du tippst. Du wirst der erste
Administrator der Box und meldest dich an den Admin-Seiten mit diesem Namen
und Passwort bei LosOS Login an, der Anmeldeseite der Box unter
`https://<box>/auth/`. Dasselbe Passwort öffnet LosOS Cloud und LosOS Git.

<img src={require("@site/docs/img/wizard-step2-password-de.png").default} alt="Schritt 2 mit eingetipptem Anmeldenamen und Passwort: Jede der vier Passwortregeln wird abgehakt, sobald sie erfüllt ist." width="520" />

Der Schritt wartet, bis LosOS Cloud seinen eigenen ersten Start beendet hat,
was auf einer frischen Box ein paar Minuten dauert. Solange zeigt die Seite
„Warten, bis diese Box fertig gestartet ist“ und den Grund. Sie macht von
selbst weiter.

<img src={require("@site/docs/img/wizard-step2-waiting-de.png").default} alt="Schritt 2, während LosOS Cloud sich noch installiert: Die Seite sagt, worauf sie wartet, und macht von selbst weiter." width="520" />

:::warning[Der Ersatzschlüssel erscheint nur einmal]
Sobald das Passwort gesetzt ist, zeigt die Seite einmal den
**Ersatz-Admin-Schlüssel**: 64 Zeichen, mit den Schaltflächen **Schlüssel
kopieren** und **Drucken**. Druck ihn aus oder schreib ihn auf und bewahre
ihn getrennt von der Box auf. Er öffnet die Admin-Seiten, wenn LosOS Login
nicht antwortet. Die Seite kann ihn nicht noch einmal anzeigen. Siehe
[Anmeldung und der Ersatzschlüssel](../manual/sign-in-and-spare-key).
:::

Sobald das Passwort gesetzt ist, zeigt der Schritt außerdem zwei Zeilen, und
beide nehmen das Passwort, das du gerade festgelegt hast:

- **Dein Name bei LosOS Login**: dein neuer Anmeldename. Er öffnet die
  Admin-Seiten.
- **Dein Konto in LosOS Cloud**: das Konto, das du in Schritt 3 und in den
  Apps für Computer und Handy eingibst.

<img src={require("@site/docs/img/wizard-step2-spare-key-de.png").default} alt="Schritt 2 nach dem Setzen des Passworts: dein Name bei LosOS Login, dein Konto in LosOS Cloud und der einmal angezeigte Ersatz-Admin-Schlüssel mit Schlüssel kopieren und Drucken." width="520" />

Einen **Passkey** für LosOS Cloud, mit dem sich ein Handy oder Laptop bei
deinen Dateien anmeldet, ohne das Passwort zu tippen, kannst du später in den
Sicherheitseinstellungen von LosOS Cloud hinzufügen.

## Schritt 3: Anmelden \{#step-3-sign-in}

Der Assistent öffnet LosOS Cloud innerhalb der Seite, und du meldest dich
mit deinem Konto in LosOS Cloud und dem Passwort an, das du gerade
festgelegt hast. Dann bringt dich der Assistent zur Übersicht der
Admin-Seiten. Ab hier geht es weiter unter [Die
Admin-Seiten](../manual/admin-pages).

Beim ersten Mal öffnet sich über der Übersicht eine kurze Tour. Sie hat sechs
Schritte, und jeder zeigt ein kleines bewegtes Bild der Seite mit dem
Eintrag in der Seitenleiste, unter dem die jeweilige Funktion liegt. Die
Schritte behandeln die Apps, das Hinzufügen weiterer Apps, den Speicher, das
Verleihen der Box in der Nacht, das nächtliche Update und wo du die Tour
wiederfindest. **Tour überspringen** schließt sie, und dieser Browser öffnet
sie nicht mehr von selbst. **Tour starten** unter Einstellungen, Über holt
sie zurück.

![Der erste Schritt der Tour über der Übersicht: ein Bild der Seite mit geöffneter Dateien-App, „1 von 6“ sowie Tour überspringen und Weiter.](@site/docs/img/tour-de.png)

![Schritt 3: Du bist angemeldet. Das war der letzte Schritt.](@site/docs/img/wizard-step3-sign-in-de.png)
