---
title: KI-Review in LosOS Git
sidebar_position: 4.6
slug: /manual/ai-review
---

# KI-Review in LosOS Git

LosOS Reviewer liest die Pull-Requests eines Repositorys in LosOS Git und
hinterlässt ein Review, so wie es ein Review-Bot auf einer gehosteten Forge
tut. Er läuft auf der Box, mit demselben Sprachmodell wie Mit KI bauen.
Der Code verlässt die Box also nie, und das Review kostet nichts.

Er ist in jedem Repository aus, bis jemand ihn einschaltet.

## Einschalten \{#turning-it-on}

Öffne das Repository in LosOS Git, dann **Einstellungen**, und schalte
**KI-Review** ein. Nur die Administratoren des Repositorys sehen den
Schalter.

Der Schalter lässt sich nicht einschalten, solange Mit KI bauen auf der
Box aus ist (`losos.builder.enable`), denn diese Einstellung installiert
das Modell. Eine neue Box lädt das Modell einmal herunter, wenn sie zum
ersten Mal online ist. Bis der Download fertig ist, warten Reviews in der
Warteschlange, und der Pull-Request sagt das.

![Die Einstellungen eines Repositorys mit eingeschaltetem KI-Review: LosOS Reviewer kommentiert jeden Pull-Request und genehmigt oder mergt nie.](@site/docs/img/git-settings-de.png)

## Was er tut \{#what-it-does}

Wird ein Pull-Request geöffnet oder wieder geöffnet, prüft LosOS Reviewer
ihn ganz. Kommen neue Commits dazu, prüft er, was diese Commits seit
seinem letzten Review geändert haben. Pro Lauf veröffentlicht er ein
Review, als Konto **LosOS Reviewer** (`losos-reviewer`), als Bot
gekennzeichnet:

- einen Überblick, was der Pull-Request tut;
- eine Tabelle der geänderten Dateien, jede mit einem Satz dazu, was sich
  geändert hat;
- Kommentare zu einzelnen Zeilen, wo er ein Problem gefunden hat. Behebt
  eine geänderte Zeile es, trägt der Kommentar diese Zeile als Vorschlag.

Er kommentiert nur. Er genehmigt nie, fordert keine Änderungen an, pusht
nicht und führt nicht zusammen.

Du antwortest auf seine Kommentare und löst sie wie die von allen anderen.
Soll er noch einmal schauen, wähle **LosOS Reviewer** unter **Reviewer**
im Pull-Request, auch wenn er schon einmal geprüft hat. Das darf jeder,
der in dem Pull-Request um Reviews bitten darf.

Während er arbeitet, zeigen die Prüfungen des Pull-Requests eine Zeile
**LosOS Reviewer** mit seinem Fortschritt, zum Beispiel „Reviewing: 2 of 5
files done“, und am Ende die Zahl der Kommentare.

![Geänderte Dateien eines Pull-Requests: LosOS Reviewer, als Bot markiert, kommentiert eine Shell-Zeile, die einen Pfad ohne Anführungszeichen löscht, und schlägt die Zeile mit Anführungszeichen vor.](@site/docs/img/git-ai-review-de.png)

## Was er finden kann \{#what-it-can-find}

Jede geänderte Datei wird in Stücken von bis zu 80 Zeilen gelesen. Für
jedes Stück passieren zwei Dinge:

- Die Box prüft die neuen Zeilen auf eine kurze Liste von Fehlern, bei
  denen sie sicher sein kann: SQL, das aus Werten zusammengesetzt wird,
  Shell-Variablen ohne Anführungszeichen in Befehlen, die Dateien ändern,
  Schlüssel und Tokens im Code, eine geteilte Standardliste in Python, ein
  Bereich einen Schritt hinter dem Ende in Rust, `==` in JavaScript und
  TypeScript, ein React-Effekt ohne Abhängigkeitsliste und ein Dateiname,
  von dem die Verzeichnisse nicht mehr abgeschnitten werden.
- Das Modell wird gefragt, ob die neuen Zeilen einen Fehler haben. Ein
  Kommentar des Modells bleibt nur, wenn er eine Zeile nennt, die der
  Pull-Request hinzufügt, und den Code dieser Zeile zitiert. Alles andere,
  was es sagt, wird verworfen.

Wie viel das Modell beiträgt, hängt von seiner Größe ab. Das
Standardmodell, Qwen2.5-Coder 1.5B, passt auf eine Box mit 4 GiB
Arbeitsspeicher. Es beschreibt Änderungen gut, findet aber selten selbst
einen Fehler, also kommen auf so einer Box die meisten Kommentare aus den
Prüfungen der Box. Qwen2.5-Coder 7B findet auch eigene Fehler und braucht
etwa 8 GiB. Um es zu nutzen, setze `losos.builder.model` im Bereich
Erweitert auf `qwen2.5-coder:7b`. Mit KI bauen nutzt es dann auch.

Ein Review beweist nicht, dass der Code stimmt. Lies den Code selbst,
bevor du zusammenführst.

## Was er auslässt \{#what-it-leaves-out}

- Dateien, die Werkzeuge schreiben und niemand Zeile für Zeile liest:
  Lock-Dateien, minifizierter und generierter Code, Bilder und andere
  Dateien, die kein Text sind.
- Gelöschte Dateien und Zeilen, die nur entfernt werden.
- Alles nach den ersten 24 Stücken eines großen Pull-Requests. Die Tabelle
  sagt, welche Dateien ausgelassen wurden.

## Die Box bleibt nutzbar \{#the-box-stays-usable}

Reviews laufen einzeln, eine Frage nach der anderen, in der Reihenfolge,
in der sie angefragt wurden. Ein neuer Push ersetzt ein unfertiges Review
desselben Pull-Requests, und das Ausschalten verwirft die Reviews, die
noch warten. Auf zwei Kernen dauert ein Stück etwa 10 bis 20 Sekunden,
also ist ein Pull-Request mit ein paar Dateien in wenigen Minuten geprüft.
Ein Bau in Mit KI bauen und ein Review teilen sich das Modell, und jedes
wartet auf die laufende Frage des anderen.

LosOS Git erreicht nur Adressen auf der Box selbst. Der Reviewer nutzt die
geteilten Rechner des Mesh nicht, weil er sonst deinen Code auf die Boxen
anderer Leute schicken würde.
