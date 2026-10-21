---
title: LosOS Git
sidebar_position: 4.6
slug: /manual/losos-git
---

# LosOS Git

LosOS Git ist der eigene Git-Server der Box. Er hält deine Repositorys,
nimmt Pushes und Klone über HTTPS an und hat Seiten, auf denen du Code
liest, Pull-Requests öffnest und sie reviewst.

Öffne ihn unter `https://<name>.local/git/`. `<name>` ist der Name der
Box, wie er auf ihrem Bildschirm steht. Du meldest dich mit LosOS Login
an. Wer die Box eingerichtet hat, ist der Besitzer, und seine Repositorys
liegen unter `notshared`.

## Repositorys \{#repositories}

Die erste Seite zeigt deine Repositorys und die, die jemand mit dir teilt.
**Neues Repository** legt eins an, privat, solange du das nicht
ausschaltest. Ein leeres Repository zeigt die Befehle, mit denen du einen
ersten Branch hineinpushst.

Die Seiten eines Repositorys stehen in der Seitenleiste:

- **Code** zeigt die Dateien eines Branches und darunter das README. Oben
  wählst du einen anderen Branch, oder du öffnest eine Datei und liest sie
  mit Zeilennummern. Markdown-Dateien erscheinen als formatierter Text.
- **Pull-Requests** zeigt die offenen und die geschlossenen.
- **Commits** und **Branches** zeigen die Geschichte. **Vergleichen** bei
  einem Branch zeigt, was er hat und der Standard-Branch nicht.
- **Worktrees** zeigt die Checkouts, die in das Repository pushen (siehe
  unten).
- **Einstellungen** benennt das Repository um, ändert seine Beschreibung
  und macht es privat oder öffentlich. Nur der Besitzer des Repositorys und
  seine Admins sehen sie.

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

## Einstellungen \{#settings}

`losos.git.enable` schaltet LosOS Git aus. Es steht im Bereich
**Erweitert**.
