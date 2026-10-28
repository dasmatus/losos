---
title: LosOS Git
sidebar_position: 4.6
slug: /manual/losos-git
---

# LosOS Git

![Ein Repository in LosOS Git, mit Wiki in seinem Menü.](@site/docs/img/losos-git-repository-de.png)

LosOS Git ist der eigene Git-Server der Box. Er hält deine Repositorys,
nimmt Pushes und Klone über HTTPS an und hat Seiten, auf denen du Code
liest, Pull-Requests öffnest und sie reviewst.

Öffne ihn unter `https://<name>.local/git/`. `<name>` ist der Name der
Box, wie er auf ihrem Bildschirm steht. Du meldest dich mit LosOS Login
an. Wer die Box eingerichtet hat, ist der Besitzer, und seine Repositorys
liegen unter `notshared`.

## Repositorys \{#repositories}

Die erste Seite zeigt deine Repositorys, die, die jemand mit dir teilt,
und die, die alle auf der Box lesen können. **Neues Repository** legt eins
an und fragt, wer es sieht. Ein leeres Repository zeigt die Befehle, mit
denen du einen ersten Branch hineinpushst.

Die Seiten eines Repositorys stehen in der Seitenleiste:

- **Code** zeigt die Dateien eines Branches und darunter das README. Oben
  wählst du einen anderen Branch, oder du öffnest eine Datei und liest sie
  mit Zeilennummern. Markdown-Dateien erscheinen als formatierter Text.
- **Pull-Requests** zeigt die offenen und die geschlossenen.
- **Commits** und **Branches** zeigen die Geschichte. **Vergleichen** bei
  einem Branch zeigt, was er hat und der Standard-Branch nicht.
- **Worktrees** zeigt die Checkouts, die in das Repository pushen (siehe
  unten).
- **Einstellungen** benennt das Repository um und ändert seine
  Beschreibung und wer es sieht. Nur der Besitzer des Repositorys und seine
  Admins sehen sie.

### Wer ein Repository sieht \{#who-can-see-a-repository}

Jedes Repository hat eine von drei Einstellungen, die ein Abzeichen neben
seinem Namen zeigt:

- **Privat**: du und die Leute, die du hinzufügst. Das ist die Vorgabe.
- **Diese Box**: alle mit einem Konto auf der Box, sobald sie angemeldet
  sind.
- **Öffentlich**: alle, die die Box erreichen, auch ohne Anmeldung. Sie
  können den Code, die Geschichte und die Pull-Requests lesen und es
  klonen. Über einen Edge sind das alle im Internet.

Bei jeder Einstellung können nur du und die Leute, die du hinzufügst,
pushen. Wer `/git/` ohne Anmeldung öffnet, sieht die öffentlichen
Repositorys und einen Knopf **Anmelden**.

## Klonen und pushen \{#clone-and-push}

Die Adresse eines Repositorys steht oben auf seinen Seiten, mit einem
Knopf, der sie kopiert:

```sh
git clone https://<name>.local/git/notshared/notes.git
```

git fragt nach einem Namen und einem Token. `losos git setup` aus dem
[Terminal-Client](terminal#use-losos-cloud-and-losos-git-from-this-computer)
macht ein Token für einen Rechner und gibt es git, sodass Klonen und
Pushen danach kein Passwort mehr brauchen.

## Issues \{#issues}

Fehler, Aufgaben und Ideen zu einem Repository und zu Arbeit, die kein
Code ist, stehen im Issue-Tracker unter **Issues** in der Seitenleiste.
Die **Issues** eines Repositorys öffnen dort sein Projekt. Siehe
[Issues in LosOS Git](issues).

## Pull-Requests \{#pull-requests}

Pushe einen Branch und drück dann **Pull-Request öffnen** auf der Seite
Code oder **Vergleichen** auf der Seite Branches. Wähl den Branch, in den
gemergt werden soll, gib einen Titel und eine Beschreibung ein und drück
**Pull-Request öffnen**.

Ein Pull-Request hat drei Reiter:

- **Unterhaltung** enthält die Beschreibung, die Kommentare und die Reviews
  der Reihe nach und den Kasten zum Mergen.
- **Commits** zeigt die Commits des Branches.
- **Geänderte Dateien** zeigt die Änderungen, Datei für Datei.

### Review \{#review}

Zeig unter **Geänderte Dateien** auf eine Zeile und drück **+**, um sie zu
kommentieren. **Sofort kommentieren** schickt den Kommentar gleich ab.
**Zum Review hinzufügen** hält ihn in deinem Review, bis du es abschickst.
Wenn du fertig bist, wähl **Kommentar**, **Freigeben** oder **Änderungen
anfordern**, schreib eine Zusammenfassung, wenn du magst, und drück
**Review abschicken**. Wer den Pull-Request geöffnet hat, kann den eigenen
nur kommentieren.

Ein Kommentar zu einer Zeile beginnt einen Thread. Antworte darunter und
drück **Erledigt**, wenn die Sache geklärt ist; ein erledigter Thread
klappt sich zusammen, und **Wieder öffnen** holt ihn zurück. Ein Thread,
dessen Zeile sich seitdem geändert hat, ist als veraltet markiert.

**Reviewer** rechts bittet Leute um ein Review und zeigt, wo jeder steht.

### Checks \{#checks}

Wenn ein Werkzeug ein Ergebnis zum neuesten Commit eines Branches meldet,
etwa ein Build oder ein Testlauf, erscheint es auf dem Reiter Unterhaltung
über dem Kasten zum Mergen.

### Mergen \{#merge}

Der Kasten zum Mergen bietet drei Wege:

- **Merge-Commit** behält jeden Commit und fügt einen hinzu, der die
  Branches verbindet.
- **Squash** packt alle Änderungen des Branches in einen Commit.
- **Fast-Forward** zieht den Basis-Branch bis zum Branch vor, wenn die
  Basis nichts hat, was dem Branch fehlt.

**`<branch>` danach löschen** entfernt den Branch nach dem Mergen. Einen
Merge mit Konflikten lehnt die Box ab; merge den Basis-Branch in deinen,
pushe und versuch es noch einmal. **Ohne Mergen schließen** legt einen
Pull-Request beiseite, und **Wieder öffnen** holt ihn zurück.

## Worktrees \{#worktrees}

Ein Worktree ist ein Checkout eines Repositorys auf einem Rechner, angelegt
mit `git worktree add`. Gib jedem einen Namen und sag, auf welchem Rechner
er liegt, dann trägt jeder Push von dort beides mit:

```sh
git config extensions.worktreeConfig true
git worktree add ../fix-backup -b fix-backup
cd ../fix-backup
git config --worktree push.pushOption "worktree=fix-backup"
git config --worktree --add push.pushOption "computer=laptop"
git push -u origin fix-backup
```

**Worktrees** zeigt ihn dann mit seinem Branch, seinem letzten Push und dem
Pull-Request zu diesem Branch, falls es einen gibt. Ein Pull-Request, dessen
Branch aus einem benannten Worktree gepusht wird, zeigt dessen Namen in
seinem Kopf. **Vergessen** nimmt einen Worktree von der Liste; sein nächster
Push setzt ihn wieder darauf.

## Wiki \{#wiki}

Jedes Repository kann ein Wiki haben: Seiten in Markdown in
`<Repository>.wiki.git`, gelesen und geschrieben von denselben Leuten wie
das Repository. **Wiki** im Menü eines Repositorys öffnet seine
veröffentlichte Seite. In [LosOS Wiki](wiki) werden die Seiten geschrieben,
und dort stehen die Wikis aller Repositorys, die du lesen darfst.

## Leute von anderen Boxen \{#people-of-other-boxes}

Wer ein Konto auf einer anderen LosOS-Box in deinem Netz hat, kann ohne
Konto auf deinem Gerät bei deinen öffentlichen Repositorys mitmachen. Die
Person meldet sich mit ihrer Chat-Adresse an, der Adresse, die LosOS Chat
unter ihrem Namen zeigt, etwa `@eva:kitchenbox.local`.

1. Auf der Repository-Seite deines Geräts drückt sie **Mit Chat-Adresse**,
   tippt die Adresse und drückt **Weiter auf deiner Box**.
2. Ihre eigene Box öffnet sich und fragt, ob sie deiner sagen soll, wer sie
   ist. Sie drückt **`<name>.local` Bescheid geben**.
3. Sie ist zurück auf deinem Gerät, angemeldet als `@eva:kitchenbox.local`.

![Evas eigene Box fragt, bevor sie mattbox.local sagt, wer sie ist.](@site/docs/img/git-other-box-vouch.png)

Ihre Box sagt deiner nur die Chat-Adresse und den Namen. Dein Gerät prüft
die Antwort beim Chat-Server ihrer Box, bevor es ihr glaubt. Wer ein Konto
auf deinem Gerät hat, meldet sich wie bisher mit LosOS Login an.

So angemeldet kann sie kommentieren, reviewen und Pull-Requests öffnen. In
deine Branches pushen kann sie nicht. Sie pusht in Branches, deren Name mit
ihrem Anmeldenamen hier beginnt, zum Beispiel `eva@kitchenbox.local/hours`,
und bittet in einem Pull-Request um die Änderung. **Git-Passwort kopieren**
auf der Repository-Seite kopiert das Passwort, nach dem git fragt; der
Benutzername ist egal.

```sh
git push https://<name>.local/git/notshared/site.git HEAD:eva@kitchenbox.local/hours
```

Zusammenführen dürfen nur du und deine Admins. **Abmelden** beendet ihren
Besuch auf diesem Computer.

![Ein Pull-Request aus Evas eigenem Branch, mit einer Antwort des Repository-Besitzers.](@site/docs/img/git-other-box-pull.png)

## Ein Chatraum für ein Repository \{#a-chat-room-for-a-repository}

Mit LosOS Chat auf dem Gerät haben die **Einstellungen** eines Repositorys
den Schalter **In LosOS Chat über dieses Repository reden**. Schalte ihn
ein, und das Gerät macht in deinem Chat einen Raum für das Repository, mit
LosOS Git darin. LosOS Git schreibt dort jeden Push, Pull-Request,
Kommentar, jede Zusammenführung und jedes Prüfergebnis hinein.

![Die Einstellungen eines Repositorys, In LosOS Chat über dieses Repository reden eingeschaltet, mit einem Link zum Raum.](@site/docs/img/git-repository-room.png)

Im Kopf des Repositorys steht dann für alle, die angemeldet sind, ein
Knopf **Chat**. Einem öffentlichen Repository kann jede Person beitreten,
auch aus dem Chat ihrer eigenen Box. Einem privaten tritt man bei, wenn du
die Person im Chat einlädst.

Schaltest du ihn aus, hören die Beiträge auf. Der Raum und sein Inhalt
bleiben in deinem Chat.

## Einstellungen \{#settings}

`losos.git.enable` schaltet LosOS Git aus. `losos.git.site.requireSignIn`
verlangt von allen eine Anmeldung, bevor LosOS Git etwas zeigt, auch
öffentliche Repositorys. Beide stehen im Bereich **Erweitert**.
