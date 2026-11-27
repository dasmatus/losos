---
title: Der Terminal-Client
sidebar_position: 10
slug: /manual/terminal
---

# Der Terminal-Client

![Die Übersicht im Browser; der Terminal-Client zeigt dieselben Bereiche.](@site/docs/img/overview-de.png)

`losos` ist ein Terminalprogramm, das eine Box von einem anderen Rechner aus
verwaltet. Es zeigt dieselben Bereiche wie die Admin-Seiten, mit denselben
Worten und Farben, und braucht kein SSH, weil die Box keines hat.

Es meldet sich mit LosOS Login an, der eigenen Anmeldung der Box, so wie die
Admin-Seiten, LosOS Cloud und LosOS Git. Du gibst es im Browser frei. Danach
handelt das Terminal als du: deine Rolle und die Teile der Box, die dir
gegeben wurden. Es gibt kein eigenes Terminal-Passwort und kein Zugangstoken
zum Abschreiben aus dem Bereich Zugangstokens.

Es läuft auf 64-Bit-x86-Linux.

## Den Client installieren

```sh
curl -fsSLO https://nightly.link/dasmatus/losos/workflows/ci/main/losos-pages-updates.zip
unzip -o losos-pages-updates.zip -d losos-pages-updates
node losos-pages-updates/install.mjs
```

Die Zip-Datei ist der neueste Bau des Clients von LosOS auf GitHub, mit dem
Programm, seiner Prüfsumme und dem Skript. Das Skript prüft das Programm
gegen die Prüfsumme und legt `losos` nach `~/.local/bin`, oder nach
`/usr/local/bin`, wenn du es als root ausführst. Es fragt nie nach einem
Passwort. Lies es vorher, wenn du magst. Es ist kurz und sagt oben, was es
tut.

`losos` hält die Anmeldung im Schlüsselbund des Desktops (GNOME Keyring,
KWallet oder KeePassXC) über `secret-tool`. Unter Debian und Ubuntu ist das
das Paket `libsecret-tools`, unter Fedora und Arch `libsecret`. Die Token
schreibt es nie in eine einfache Datei.

## Anmelden

```sh
losos login https://mattbox.local
losos
```

`login` nimmt die Adresse der Box von ihrem Bildschirm. Nimm `https://`.
LosOS Login antwortet nicht auf bloßem `http://`; die Box leitet diese
Adresse nach `https://` weiter.

Beim ersten Mal, wenn das Zertifikat der Box ihr eigenes ist, zeigt `losos`
den SHA-256-Fingerabdruck des Zertifikats. Vergleiche ihn mit dem, den dein
Browser für die Box zeigt, und sag ja, wenn sie übereinstimmen. Danach
spricht `losos` nur mit einer Box mit diesem Zertifikat. Zeigt die Box
später ein anderes, hält `losos` an und sagt es. Führe `login` erneut aus,
sobald du weißt, warum es sich geändert hat.

Dann meldet sich `losos` mit LosOS Login an. Es druckt einen Link und einen
Code:

```text
losos meldet sich bei https://mattbox.local mit LosOS Login an. Öffne diesen Link im Browser, melde dich an und erlaube das Terminal:
  https://mattbox.local/auth/consent/openid/device-authorization?user_code=WDJBMJHT
Die Seite zeigt den Code WDJBMJHT. Warte auf die Freigabe…
```

Öffne den Link, melde dich bei LosOS Login an wie immer, mit dem zweiten
Schritt, wenn du einen hast, prüfe, dass die Seite denselben Code zeigt, und
wähle **Accept**. Das Terminal macht allein weiter.

Du tippst dein Passwort nie ins Terminal. LosOS Login gibt ihm zwei Token:
eines, das die Box eine Stunde öffnet, und eines, das ein neues Paar holt,
wenn das abläuft. Wird das Terminal 30 Tage nicht benutzt, muss es sich neu
anmelden. Beide bleiben im Schlüsselbund. Ein zweites `losos` auf diesem
Rechner erneuert sie gemeinsam, damit eines das andere nicht aus Versehen
abmeldet.

Das Terminal handelt als du. Ein Administrator sieht jeden Bereich, den
es hat. Alle anderen sehen die Teile, die ihnen gegeben wurden, dieselben,
die die Admin-Seiten ihnen zeigen, und eine Änderung, die sie nicht machen
dürfen, wird abgelehnt. Eine Person, die LosOS Login auf die eigene Kontoseite
hält, kann das Terminal nicht benutzen, bis das unter **Dein Konto** auf den
Admin-Seiten erledigt ist.

```sh
losos logout https://mattbox.local
```

`logout` gibt beide Token an LosOS Login zurück und vergisst sie. Die Box
nimmt sie sofort nicht mehr an.

`losos` allein öffnet die volle Ansicht: die Seitenleiste links, der Bereich
rechts, die Tasten unten. Die Kopfzeile nennt, wer angemeldet ist. Pfeile
oder `j` und `k` bewegen, Enter ändert eine Einstellung, Tab wechselt
zwischen Seitenleiste und Bereich, `/` sucht die Bereiche, `?` listet den
Rest und `q` beendet. Eine Änderung bleibt ein Entwurf, bis du `a` für
Übernehmen drückst, und die Box fragt, bevor sie sich neu baut, wie die
Admin-Seiten.

:::note[Was auf den Admin-Seiten bleibt]
Zugangstokens, der Markt, Diagramme und alles, was Daten löscht, bleiben auf
den Admin-Seiten.
:::

## Eine alte Anmeldung

Ein Rechner, der sich früher mit einem Zugangstoken und einem GPG-Schlüssel
angemeldet hat, hat diese Anmeldung noch im Schlüsselbund. `losos` benutzt
sie nicht. Es sagt, die Anmeldung sei das alte Zugangstoken, und dass
`losos login` stattdessen mit LosOS Login anmeldet. `losos logout` entfernt
die alte Anmeldung. Es gibt einen Weg hinein.

## Im eigenen Netz der Box

Die Aufrufe des Terminals bleiben im eigenen Netz der Box, wie die
Admin-Seiten. LosOS Login, LosOS Cloud und LosOS Git antworten über das
Internet. Das Terminal schickt seine Aufrufe nicht auf diesem Weg hinaus,
weil diese Aufrufe die Anmeldung tragen.

## LosOS Cloud und LosOS Git von diesem Rechner \{#use-losos-cloud-and-losos-git-from-this-computer}

```sh
losos cloud setup
losos git setup
```

Jedes fragt nach deinem LosOS-Passwort, dem, mit dem du dich bei LosOS Login
anmeldest, und richtet den Rechner ein, auf dem du es ausführst. Der Bereich
**Dieser Computer** der vollen Ansicht tut dasselbe mit einer Taste. Ein
Administrator kann sie ausführen.

`cloud setup` bittet die Box um ein App-Passwort nur für diesen Rechner und
hält es im Schlüsselbund, wo der Dateimanager danach sucht. LosOS Cloud
erscheint dann in der Seitenleiste des Dateimanagers (GNOME Files und andere,
die GTK-Lesezeichen lesen), und das Öffnen zeigt deine Dateien, ohne nach
einem Passwort zu fragen. Der Dateimanager fragt einmal, das eigene
Zertifikat der Box anzunehmen, wenn er sich das erste Mal verbindet.

`git setup` bittet die Box um ein LosOS-Git-Token nur für diesen Rechner und
hält es im Schlüsselbund. git holt es dann von `losos`, wann immer es mit der
Box spricht, sodass Klonen und Schieben kein Passwort brauchen:

```sh
git clone https://mattbox.local/git/notshared/notes.git
```

Hat die Box ein eigenes Zertifikat, sagt der Client git, genau den Schlüssel
dieses Zertifikats anzunehmen, dieselbe Prüfung, die `losos` macht.

Beide heißen nach dem Rechner, etwa „LosOS on laptop“. `--device` wählt einen
anderen Namen. Um einem Rechner den Zugang zu nehmen, lösche seinen Eintrag
unter **Sicherheit** in den Einstellungen von LosOS Cloud oder unter
**Anwendungen** in LosOS Git. Ein erneutes Einrichten ersetzt das Passwort
oder Token dieses Rechners.

## In einem Skript

`losos call` sendet eine Anfrage und druckt die Antwort:

```sh
losos call GET /api/state
```

Es endet mit 1, wenn die Box mit einem Fehler antwortet, sodass ein Skript
das prüfen kann. `losos --help` listet jeden Befehl.

`login` in einem Skript druckt denselben Link und wartet. Ist das Zertifikat
das eigene der Box, gib seinen SHA-256 als `--fingerprint` an, den dein
Browser zeigt, damit das Skript nicht fragen muss.
