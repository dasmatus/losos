---
title: Der Terminal-Client
sidebar_position: 10
slug: /manual/terminal
---

# Der Terminal-Client

![Die Übersicht im Browser; der Terminal-Client zeigt dieselben Bereiche.](@site/docs/img/overview-de.png)

`losos` ist ein Terminalprogramm, das eine Box von einem anderen Rechner aus
verwaltet. Es zeigt dieselben Bereiche wie die Admin-Seiten, mit denselben
Worten und Farben, und braucht kein SSH, weil die Box keines hat. Jede
Anfrage, die es sendet, ist mit deinem eigenen GPG-Schlüssel signiert und an
die Box verschlüsselt. Die Box antwortet auf dieselbe Weise, sodass nur die
Box die Anfrage lesen kann und nur du die Antwort.

Es läuft auf 64-Bit-x86-Linux.

## Bevor du die Box installierst: deinen Schlüssel auf den Stick legen \{#before-you-install-the-box-put-your-key-on-the-stick}

Die Box nimmt nur Anfragen an, die mit einem GPG-Schlüssel signiert sind,
den sie bei der Installation bekommen hat. Nichts auf einer laufenden Box
kann später einen hinzufügen. Könnte es das, dann könnte jeder, der in die
Admin-Seiten gelangt, seinen eigenen Schlüssel hinzufügen und dafür ein
Token erstellen. Der Terminal-Client braucht deshalb ein wenig Planung: Der
Stick, von dem du installierst, muss deinen Schlüssel tragen.

:::warning[Der Release-Stick trägt keinen Schlüssel]
Der Stick von der Release-Seite trägt keinen Schlüssel. Eine davon
installierte Box funktioniert in jeder anderen Hinsicht, und ihr Bereich
Zugriffstokens sagt, dass kein Schlüssel hinterlegt wurde, und erstellt keine
Tokens. Die einzige Möglichkeit, dieser Box einen Schlüssel zu geben, ist
eine erneute Installation von einem Stick, der einen trägt, und die
Installation löscht sie.
:::

Du brauchst einen GPG-Schlüssel mit einem Unterschlüssel zum Verschlüsseln,
weil die Box ihre Antworten an ihn verschlüsselt. `gpg --full-generate-key`
erzeugt standardmäßig einen solchen. Bei einem Schlüssel, an den die Box
nicht antworten könnte, bricht der Installer ab, bevor er eine Festplatte
anrührt. Exportiere die öffentliche Hälfte in einen neuen, leeren Ordner:

```sh
mkdir my-losos-stick && cd my-losos-stick
gpg --armor --export you@example.com > owner.asc
```

Speichere daneben Folgendes als `flake.nix`:

```nix
{
  inputs.losos.url = "github:dasmatus/losos";
  outputs = { losos, ... }:
    let
      key = builtins.readFile ./owner.asc;
      stick = losos.nixosConfigurations.iso.extendModules {
        modules = [ { losos.access.gpgKeys = [ key ]; } ];
      };
    in {
      packages.x86_64-linux.default =
        stick.config.system.build.isoImage;
    };
}
```

Baue dann den Stick mit [Nix](https://nixos.org/download/) auf einem
64-Bit-x86-Linux-Rechner:

```sh
nix --extra-experimental-features 'nix-command flakes' build
```

Der Build kompiliert die LosOS-Programme, also dauert der erste Lauf eine
Weile. Das ISO landet in `result/iso/`. Schreib es auf den Stick und
[installiere](/start/install.md) wie gewohnt. Nach dem ersten Start listet der
Bereich Zugriffstokens deinen Schlüssel unter **Hinterlegte GPG-Schlüssel**.
Zwei Schlüssel gehen auch: Führe beide auf, wie in
`[ key (builtins.readFile ./other.asc) ]`.

:::note[Selbst gebaute Sticks sind nicht signiert]
Ein selbst gebauter Stick ist nicht signiert, also schalte Secure Boot in der
Firmware aus, um von ihm zu starten.
:::

## Ein Token erstellen \{#make-a-token}

Öffne **Einstellungen → Zugriffstokens** auf den Admin-Seiten. Der Bereich
zeigt:

- **Schlüssel dieser Box**, den Fingerabdruck, den das Terminal bei seiner
  ersten Verbindung prüft;
- **Hinterlegte GPG-Schlüssel**, die Schlüssel, mit denen die Box installiert
  wurde;
- **Tokens**, die schon erstellten, jeweils mit dem Zeitpunkt der letzten
  Benutzung.

Gib dem neuen Token einen Namen, wähle den Schlüssel, zu dem es gehört,
wähle **Nur lesen** oder **Voller Zugriff** und bestimme, wann es abläuft.
**Token erstellen** zeigt es einmal an. Kopiere es dann. Die Box behält nur
einen Hash und kann es nicht noch einmal zeigen.

Ein Token funktioniert nur in einer Anfrage, die mit dem Schlüssel signiert
ist, für den es erstellt wurde, also nützt ein vom Bildschirm abgeschriebenes
Token ohne diesen Schlüssel nichts. Ein Token mit Nur-Lese-Zugriff kann jeden
Bereich ansehen und nichts ändern. Kein Token kann Tokens erstellen,
auflisten oder widerrufen, und keines erreicht den Einrichtungsassistenten
oder die Anmeldung. **Widerrufen** beendet eines sofort.

## Den Client installieren \{#install-the-client}

```sh
curl -fsSLO https://nightly.link/dasmatus/losos/workflows/ci/main/losos-pages-updates.zip
unzip -o losos-pages-updates.zip -d losos-pages-updates
sh losos-pages-updates/install.sh
```

Das Zip ist der neueste Build des Clients aus dem GitHub von LosOS, mit dem
Programm, seiner Prüfsumme und dem Skript. Das Skript prüft das Programm
gegen die Prüfsumme und legt `losos` in `~/.local/bin` ab, oder in
`/usr/local/bin`, wenn du es als root ausführst. Es fragt nie nach einem
Passwort. Lies es vorher, wenn du willst. Es ist kurz und sagt oben, was es
tut.

`losos` bewahrt deinen Schlüssel und deine Tokens im Schlüsselbund des
Desktops auf (GNOME Keyring, KWallet oder KeePassXC), über `secret-tool`. Auf
Debian und Ubuntu ist das das Paket `libsecret-tools`, auf Fedora und Arch
`libsecret`. In eine einfache Datei schreibt es sie nie.

## Verbinden \{#connect}

```sh
losos key import --gpg you@example.com
losos login http://192.168.1.23
losos
```

`key import --gpg` kopiert deinen geheimen Schlüssel aus GnuPG in den
Schlüsselbund. Hast du den geheimen Schlüssel in einer Datei, macht
`losos key import secret.asc` dasselbe aus der Datei.

`login` nimmt die Adresse der Box von ihrem Bildschirm. Einfaches `http://`
ist hier in Ordnung: Anfrage und Antwort sind Ende zu Ende verschlüsselt,
egal was sie transportiert. `https://<name>.local` geht auch. Das Zertifikat
der Box ist ihr eigenes, also übernimmt `losos` das, welches es beim Login
sieht, und merkt es sich. Zeigt die Box später ein anderes Zertifikat, hält
`losos` an und sagt das. Führe `login` erneut aus, um das neue anzunehmen,
sobald du weißt, warum es sich geändert hat.

`login` fragt die Box nach ihrem Schlüssel und zeigt den Fingerabdruck in
Vierergruppen. Prüfe, dass er mit **Schlüssel dieser Box** im Bereich
Zugriffstokens übereinstimmt, bevor du mit Ja antwortest. Füge dann das
Token ein, das `losos` nicht anzeigt. Ab dann merkt sich `losos` den
Schlüssel der Box und weigert sich, mit ihr zu sprechen, falls er sich je
ändert.

`losos` allein öffnet die Vollbildansicht: links die Seitenleiste, rechts der
Bereich, unten die Tasten. Die Pfeiltasten oder `j` und `k` bewegen, Enter
ändert eine Einstellung, Tab wechselt zwischen Seitenleiste und Bereich, `/`
durchsucht die Bereiche, `?` listet den Rest, und `q` beendet. Eine Änderung
bleibt ein Entwurf, bis du `a` drückst, um sie anzuwenden, und die Box fragt
vor dem Neuaufbau nach, wie es die Admin-Seiten tun.

:::note[Was auf den Admin-Seiten bleibt]
Zugriffstokens, der Markt, Diagramme und alles, was Daten löscht, bleiben
auf den Admin-Seiten.
:::

## Unterwegs \{#away-from-home}

Erreicht die Box das Internet über eine Edge, merkt sich `login` auch die
Adresse der Box auf der Edge, und `losos hosts` zeigt sie als **unterwegs**.
Führt die eigene Adresse der Box ins Leere, weil dieser Rechner in einem
anderen Netz ist, schickt `losos` den Aufruf stattdessen an diese Adresse.
Die Edge reicht ihn weiter, ohne ihn zu lesen: Er ist weiterhin mit deinem
Schlüssel signiert und an den Schlüssel der Box verschlüsselt, und die Box
antwortet genauso. Die Edge hat ein Zertifikat, dem das Web vertraut, also
gibt es nichts weiter festzuhalten.

Nur die Aufrufe des Terminals gehen so über die Edge. Die übrigen
Admin-Seiten bleiben im eigenen Netz der Box. Soll auch das Terminal dort
bleiben, schalte `losos.access.throughEdge` im Bereich „Erweitert“ aus.

## LosOS Cloud und LosOS Git von diesem Rechner aus nutzen \{#use-losos-cloud-and-losos-git-from-this-computer}

```sh
losos cloud setup
losos git setup
```

Beide fragen nach deinem LosOS-Passwort, dem, mit dem du dich bei LosOS
Cloud anmeldest, und richten den Rechner ein, auf dem du sie ausführst. Der
Bereich **Dieser Computer** der Vollbildansicht macht dasselbe mit einer
Taste.

`cloud setup` fordert bei der Box ein App-Passwort nur für diesen Rechner an
und legt es im Schlüsselbund dort ab, wo der Dateimanager danach sucht.
LosOS Cloud erscheint dann in der Seitenleiste des Dateimanagers (GNOME
Dateien und andere Dateimanager, die GTK-Lesezeichen lesen), und beim
Öffnen siehst du deine Dateien, ohne nach einem Passwort gefragt zu werden.
Der Dateimanager fragt bei der ersten Verbindung einmal, ob er das eigene
Zertifikat der Box annehmen soll.

`git setup` fordert bei der Box ein LosOS-Git-Token nur für diesen Rechner
an und legt es im Schlüsselbund ab. git holt es sich dann von `losos`,
wann immer es mit der Box spricht, sodass Klonen und Pushen kein Passwort
brauchen:

```sh
git clone https://mattbox.local/git/notshared/notes.git
```

Hat die Box ihr eigenes Zertifikat, wird git angewiesen, genau den Schlüssel
dieses Zertifikats anzunehmen, dieselbe Prüfung, die `losos` macht.

Beide tragen den Namen des Rechners, etwa „LosOS on laptop“. `--device`
wählt einen anderen Namen. Um einem Rechner den Zugang zu nehmen, lösche
seinen Eintrag unter **Sicherheit** in den Einstellungen von LosOS Cloud
oder unter **Anwendungen** in denen von LosOS Git. Führst du ein Setup
erneut aus, ersetzt es das Passwort oder Token dieses Rechners.

## In einem Skript \{#in-a-script}

`losos call` sendet eine Anfrage und gibt die Antwort aus:

```sh
losos call GET /api/state
```

Es beendet sich mit 1, wenn die Box mit einem Fehler antwortet, sodass ein
Skript das prüfen kann. `losos --help` listet jeden Befehl.
