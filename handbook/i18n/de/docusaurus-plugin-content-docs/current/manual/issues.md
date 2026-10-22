---
title: Issues in LosOS Git
sidebar_position: 4.55
slug: /manual/issues
---

# Issues in LosOS Git

LosOS Git hält die Issues der Box: Fehler, Aufgaben und Ideen, für Code und
für alles andere. Du findest sie unter **Issues** in der Seitenleiste von
LosOS Git, in einer Liste für alle Projekte der Box. Die Adresse ist
`https://<box>.local/git/issues`.

## Projekte und Kürzel \{#projects-and-keys}

Jedes Issue gehört zu einem Projekt, und jedes Projekt hat ein kurzes
Kürzel. Ein Issue heißt nach Kürzel und Nummer: `BT-12` ist Issue 12 des
Projekts mit dem Kürzel `BT`. Der Name bleibt dem Issue sein Leben lang,
du kannst ihn also ruhig in einen Commit, einen Chat oder eine Mail
schreiben.

Ein Repository bekommt sein eigenes Projekt, sobald jemand zum ersten Mal
seine **Issues** öffnet. Das Kürzel entsteht aus dem Namen des Repositorys,
und die Nummern der Issues sind die des Repositorys: `BT-12` und
`backup-tool#12` sind dasselbe Issue. Ein Projekt kann auch für sich
stehen, ohne Repository, für Arbeit, die kein Code ist. **Neues Projekt**
in der Seitenleiste legt eins an. Ein Projekt ist entweder privat, offen
nur für die Leute, die du hinzufügst, und für alle, die sein Repository
sehen, oder offen für alle mit einem Konto auf der Box, die dann auch seine
Issues ändern dürfen.

## Status, Priorität und der Rest \{#status-priority-and-the-rest}

Ein Issue läuft durch den Ablauf seines Projekts. Ein neues Projekt hat
sechs Status: Backlog, Todo, In Progress, In Review, Done und Canceled.
Jeder Status gehört zu einer von fünf Arten: Backlog, zu erledigen, in
Arbeit, erledigt und abgebrochen. Die Listen, das Board und die
Schließwörter unten richten sich nach der Art. Ein Projekt kann seine
Status also umbenennen oder eigene hinzufügen.

Neben dem Status hat ein Issue:

- eine Priorität: dringend, hoch, mittel, niedrig oder keine;
- eine zuständige Person;
- Labels, die entstehen, wenn du einen neuen Namen tippst;
- eine Schätzung in Punkten (1, 2, 3, 5, 8, 13 oder 21);
- ein Fälligkeitsdatum;
- einen Zyklus (siehe unten);
- ein übergeordnetes Issue, das es zum Unter-Issue macht. Das übergeordnete
  zeigt, wie viele seiner Unter-Issues erledigt sind.

Zwei Issues können zusammenhängen: eines blockiert das andere, eines ist
ein Duplikat des anderen, womit das Duplikat abgebrochen wird, oder sie
hängen einfach zusammen.

Jedes Issue behält seinen Verlauf: wer es angelegt hat, jede Änderung von
Status, Priorität, Person, Label oder Zyklus, jeden Kommentar und die
Commits und Pull-Requests, die es genannt haben.

## Listen, das Board und Ansichten \{#lists-the-board-and-views}

**Meine Issues** zeigt, was dir zugewiesen ist, über alle Projekte.
**Alle Issues** zeigt alles, was du sehen darfst. Ein Projekt hat seine
Issues, sein **Backlog** mit den Issues, die noch niemand begonnen hat, und
seine **Zyklen**.

Jede Liste kannst du als Liste, nach Status gruppiert, oder als Board mit
einer Spalte je Status zeigen. Auf dem Board ziehst du eine Karte in eine
andere Spalte, um ihren Status zu ändern, oder nach oben und unten, um die
Reihenfolge zu ändern. **Filter** grenzt eine Liste nach Status, Priorität,
Person, Label, Zyklus und mehr ein, und **Anzeige** wählt, wie sie
gruppiert und sortiert wird. **Ansicht speichern** behält Filter und
Darstellung unter einem Namen in der Seitenleiste; eine Ansicht behältst du
für dich oder teilst sie mit allen, die das Projekt sehen.

Markierst du Issues mit ihren Kästchen oder mit `X`, öffnet sich eine
Leiste, die Status, Priorität, Person oder Labels aller auf einmal ändert.

## Zyklen \{#cycles}

Ein Zyklus ist ein Zeitabschnitt, meist eine oder zwei Wochen, mit den
Issues, die in ihn gehören. **Zyklen** eines Projekts listet sie mit dem
Anteil erledigter Issues auf, und **Dieser Zyklus** zeigt die Issues des
Zyklus, der heute läuft.

## Der Posteingang \{#the-inbox}

**Posteingang** listet, was mit den Issues passiert ist, denen du folgst:
eines wurde dir zugewiesen, jemand hat dich mit `@` genannt, ein Kommentar,
ein neuer Status, ein Commit oder Pull-Request, der es nennt. Du folgst
einem Issue, das du angelegt hast, das dir zugewiesen ist, das du
kommentiert hast oder in dem du genannt wurdest, und jedem, bei dem du die
Glocke einschaltest. Die Zahl neben **Posteingang** zählt, was du noch
nicht gelesen hast.

## Die Tastatur \{#the-keyboard}

Alles geht auch ohne Maus. Drück `?` auf den Issue-Seiten für die Liste.
Die häufigsten:

| Taste | Macht |
| --- | --- |
| `C` | ein neues Issue |
| `J`, `K` | in der Liste nach unten und oben |
| `Enter` | das Issue öffnen |
| `X` | es markieren |
| `S`, `P`, `A`, `L` | Status, Priorität, Person oder Labels ändern |
| `0` bis `4` | die Priorität setzen |
| `B` | zwischen Liste und Board wechseln |
| `/` | in der Liste suchen |
| `Strg+K` | jedes Issue, Projekt oder jede Ansicht nach Namen finden |

## Commits und Pull-Requests \{#commits-and-pull-requests}

Der Name eines Issues in einer Commit-Nachricht oder einem Pull-Request
verbindet beide: Das Issue zeigt den Commit oder Pull-Request, und der
Posteingang sagt es allen, die dem Issue folgen. `BT-12`, `#12` und
`owner/repo#12` funktionieren alle.

Ein Schließwort vor dem Namen erledigt das Issue auch: `close`, `closes`,
`closed`, `fix`, `fixes`, `fixed`, `fixing`, `resolve`, `resolves` oder
`resolved`. Ein Commit mit `Fixes BT-12` setzt BT-12 auf Done, sobald er im
Hauptzweig des Repositorys ankommt. Ein Pull-Request, der ein Issue nennt,
setzt es auf In Review, wenn es noch niemand begonnen hatte, und wird er
gemergt, gehen die Issues, die er laut eigenem Text schließt, auf Done. Ein
Branch, der nach einem Issue heißt, etwa `bt-12-free-space`, nennt es auch.

So schließen kann nur, wer das Issue ändern darf.

## Mit den anderen Apps \{#with-the-other-apps}

Aus dem Menü **⋯** oben an einem Issue:

- **Per Mail senden** öffnet LosOS Mail mit Namen und Link des Issues;
- **Im Chat teilen** öffnet es in LosOS Chat;
- **Fälligkeit in den Kalender** gibt das Fälligkeitsdatum als
  Kalenderdatei;
- **Kontakt der zuständigen Person** öffnet die Person in Kontakte.

Mail, Chat und Kontakte gibt es, wenn die Box teilt und in einem Mesh ist.
Jedes dir zugewiesene Issue mit Fälligkeitsdatum steht auch in einem
Kalender-Abo unter `https://<box>.local/git/api/v1/tracker/calendar.ics`.
Eine Kalender-App abonniert es mit deinem Namen und einem Zugriffstoken von
LosOS Git mit dem Bereich für Issues als Passwort.

Ist Build with AI an, kann **Die KI der Box** an einem Issue es **In
Schritte zerlegen**, die du ankreuzt, um sie zu Unter-Issues zu machen,
oder **Zusammenfassen**, was gesagt wurde. Das Modell läuft auf der Box,
und nichts ändert sich, bis du wählst, was bleibt.

## Wer was darf \{#who-may-do-what}

Wer das Repository eines Projekts sehen darf, sieht auch seine Issues,
darf sie kommentieren und neue öffnen; ein neues Issue von jemandem, der
das Repository nur lesen darf, behält Titel und Beschreibung und sonst
nichts. Ein Issue ändern braucht Schreibrecht auf Repository oder Projekt.
Ablauf, Labels, Zyklen und Mitglieder eines Projekts ändern seine
Administratoren.

## Eine Box mit Forgejo oder Boards aus LosOS Cloud \{#a-box-that-had-forgejo-or-losos-cloud-boards}

Aus Forgejo übernommene Issues werden die Issues des Projekts ihres
Repositorys, mit denselben Nummern, Kommentaren und Labels.

Die Boards aus LosOS Cloud werden einmal übernommen, beim ersten Start mit
Issues: Jedes Board wird ein Projekt mit seinen Spalten als Ablauf, seine
Karten werden Issues mit Labels, Personen, Fälligkeiten und Kommentaren,
und seine Mitglieder Mitglieder des Projekts. Ein Board, das zu einem
Repository gehörte, füllt das Projekt dieses Repositorys, und eine Karte,
die auf eines seiner Issues zeigte, verschiebt dieses Issue, statt ein
zweites anzulegen. Die Kopie in LosOS Cloud bleibt, wie sie war. Was
übernommen wurde, steht in `/var/lib/losos-git/boards/import.json`.
