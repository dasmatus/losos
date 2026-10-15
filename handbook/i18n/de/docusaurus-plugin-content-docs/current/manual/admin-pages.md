---
title: Die Admin-Seiten
sidebar_position: 1
---

# Die Admin-Seiten

Die Admin-Seiten sind die eigene Webseite der Box unter `http://<address>/`.
Die Box liefert sie nur in dein lokales Netz aus, auch wenn eine Edge die Box
veröffentlicht. Jeder Tab fragt nach dem Passwort des Besitzers, bevor er
etwas zeigt, das du ändern kannst.

## Die Bereiche \{#the-sections}

Die Seitenleiste links hat fünf Bereiche und darunter die Dinge, die du
angeheftet hast. Eingeklappt schrumpft sie zu einer Leiste aus Symbolen. Auf
dem Handy öffnet sie sich als Overlay.

![Die Übersicht: links die Seitenleiste mit den fünf Bereichen, der Name der Box und ob sie antwortet, die App-Kacheln, die Speicherkarte und das Board.](@site/docs/img/overview.png)

| Bereich           | Was dort ist                                                                                                 |
| ----------------- | ------------------------------------------------------------------------------------------------------------ |
| **Übersicht**     | Ob die Box antwortet und seit wann, die Speicherkarte, die App-Kacheln und dein Board mit Widgets.            |
| **Apps**          | Die Apps auf dieser Box mit ihrem Modus, eine Suche in Artifact Hub und die von dort installierten Apps.      |
| **Speicher**      | Wie voll die Festplatte ist, was den Platz belegt, und die Reserve, die du freigeben kannst.                  |
| **Mesh**          | Anderen Boxen beitreten, die Stunden, in denen diese Box ihre freie Zeit verleiht, und **Markt** mit dem Teilen der Festplatte. |
| **Einstellungen** | Netzwerk, Aussehen, Hardware, Sicherheit, Erweitert, Historie, Über und Zurücksetzen, jeder davon ein eigener Bereich. |

<img src={require("@site/docs/img/overview-phone.png").default} alt="Die Übersicht auf einem Handy: Die Bereiche klappen in eine Menütaste zusammen." width="260" /> <img src={require("@site/docs/img/overview-phone-sheet.png").default} alt="Dasselbe Handy mit dem Menü, das als Overlay über der Seite offen ist." width="260" />

## Suche \{#search}

Die Lupe oben in der Seitenleiste öffnet eine Liste, die Seiten, Apps,
Dateien und Repositorys findet. Strg+Umschalt+P öffnet sie auf jeder
Admin-Seite ebenfalls (Cmd+Umschalt+P auf einem Mac), und genauso ein
Doppelpunkt, den du irgendwo außerhalb eines Textfelds tippst. Sie findet:

- die Bereiche und Einstellungsbereiche, nach Namen oder nach einem Wort wie
  „gpu“ oder „hostname“, in der Sprache auf dem Bildschirm oder auf Englisch
- die Apps in LosOS cloud und LosOS Git
- Dateien und Ordner in LosOS cloud, nach Namen
- Repositorys in LosOS Git
- Text in Dateien, sobald du drei Buchstaben tippst: in deinen Dateien in
  LosOS cloud und im Hauptzweig jedes Repositorys in LosOS Git
- deine angehefteten Einträge
- Apps zum Installieren aus Artifact Hub; wählst du eine, öffnet sich der
  Bereich Apps mit ihrem Installationsdialog
- ein paar Aktionen: helles oder dunkles Design, die Sprache, die Tour,
  dieses Handbuch und das Abmelden

Mit den Pfeiltasten bewegst du dich durch die Liste, Enter öffnet den
markierten Eintrag, und Escape schließt die Liste. Dateien und Repositorys
öffnen sich in ihrer eigenen App, und Strg+Enter oder Cmd+Enter öffnet einen
Eintrag in einem neuen Tab.

Die Dateisuche braucht eine Anmeldung mit deinem Passwort, nicht mit dem
Ersatzschlüssel. Bis dahin sagt die Liste das anstelle der Dateien.

Damit du in deinen Dateien in LosOS cloud suchen kannst, führt die Box einen
Index ihres Texts. Sie prüft alle zwei Minuten auf neue und geänderte Dateien
und liest nur diese. Sie liest reinen Text und Code, Markdown, CSV, JSON und
HTML, OpenDocument-Dateien, Word-, Excel- und PowerPoint-Dateien in ihren
aktuellen Formaten (.docx, .xlsx, .pptx) und PDFs. Bilder, Audio, Video und
Archive überspringt sie, und von jeder Datei behält sie nur das erste
Megabyte Text. Direkt nach dem ersten Start sagt die Liste, dass die Box
deine Dateien noch liest. Der Index bleibt auf der Box, ein Löschen auf
Werkseinstellungen entfernt ihn, und `losos.search.enable` im Bereich
Erweitert schaltet ihn ab. LosOS Git braucht keinen Index: Jede Suche liest
die Repositorys direkt.

## Angeheftete Einträge \{#pins}

Hefte die Apps, Dateien und Repositorys an, die du am häufigsten öffnest,
und sie stehen unter **Angeheftet** in der Seitenleiste. Ein angehefteter
Eintrag öffnet sich in seiner eigenen App: eine Datei oder ein Ordner in
LosOS cloud, ein Repository in LosOS Git, eine App auf der Übersicht oder
eine App, die du über die Suche installiert hast, auf ihrem eigenen Port.
Die Box speichert die Liste, sodass jeder Browser, mit dem du dich
anmeldest, dieselben Einträge zeigt.

- Zeige auf der Übersicht auf eine App-Kachel und klicke auf die Pinnnadel
  in ihrer Ecke. Eine angeheftete Kachel behält die Nadel, in Farbe.
- Auf der Seite Apps steht **Anheften** neben jeder installierten App.
- Der Stift neben **Angeheftet** öffnet die ganze Liste. Dort schiebst du
  einen Eintrag nach oben oder unten, löst ihn und heftest aus drei Tabs an:
  **Apps**, **Dateien**, das durch die Ordner von LosOS cloud führt, und
  **Repositorys**.
- Zeige in der Seitenleiste auf einen Eintrag und klicke auf die
  durchgestrichene Nadel, um ihn zu lösen. Die [Suche](#search) findet
  angeheftete Einträge ebenfalls.

![Die Übersicht mit sechs Einträgen unter Angeheftet in der Seitenleiste; die angehefteten Kacheln Kalender und Notizen sind markiert.](@site/docs/img/pins-overview.png)

![Die Liste der angehefteten Einträge, mit dem Tab Dateien auf dem Ordner Documents von LosOS cloud geöffnet.](@site/docs/img/pins-dialog.png)

Die Box listet deine Dateien mit einem App-Passwort auf, das sie bei LosOS
cloud anfordert, wenn du dich mit deinem Passwort anmeldest. Bis du das
einmal getan hast, bittet dich der Tab **Dateien** darum. Das App-Passwort
erscheint in den Sicherheitseinstellungen von LosOS cloud als „LosOS admin
page“, und wenn du es dort widerrufst, endet die Auflistung bis zu deiner
nächsten Anmeldung. Die Seitenleiste fasst 24 angeheftete Einträge.

## Die Tour \{#the-tour}

Wenn du diese Seiten nach der Einrichtung zum ersten Mal öffnest, startet
über der Übersicht eine Tour aus sechs kurzen Schritten. **Tour starten**
unter Einstellungen, Über öffnet sie erneut. Ist in deinem System „Bewegung
reduzieren“ eingestellt, zeigt jeder Schritt ein Standbild statt eines
bewegten.

## Benachrichtigungen \{#notifications}

Kurze Statusmeldungen erscheinen oben rechts: eine Änderung, die gerade
angewendet wird, eine Änderung, die übernommen wurde, ein hinzugefügtes
Widget. Eine Frage, die deine Antwort braucht, etwa das Zurücksetzen der Box
oder das Freigeben der Reserve, öffnet stattdessen einen Dialog. Die sichere
Wahl steht an erster Stelle.

![Ein Einstellungsbereich mit einer noch nicht angewendeten Änderung: unten die Leiste mit Verwerfen und Anwenden.](@site/docs/img/settings-apply-bar.png)

## Design und Sprache \{#theme-and-language}

Die Leiste oben schaltet zwischen Englisch, Slowakisch und Deutsch um und
zwischen „Wie im Browser“, hell und dunkel. Dieser Browser merkt sich beides.

![Die Übersicht auf Deutsch und im dunklen Design.](@site/docs/img/overview-de-dark.png)

## Abmelden \{#sign-out}

**Abmelden** vergisst den Admin-Schlüssel in diesem Tab. Das Schließen des
Tabs bewirkt dasselbe. Siehe [Anmeldung und der
Ersatzschlüssel](sign-in-and-spare-key).

## Was die Admin-Seiten nicht können \{#what-the-admin-pages-cannot-do}

Es gibt keinen Dateibrowser, kein Terminal und keine Log-Anzeige. Deine
Dateien liegen in LosOS cloud; der Tab **Dateien** in der Liste der
angehefteten Einträge zeigt Ordner nur, damit du aus ihnen anheften kannst.
Die Box hat keine Shell, also haben auch die Seiten keine. Über sich selbst
berichtet die Box auf der Übersicht, im Bereich Speicher und im Bereich
Über. Reicht das nicht, sagt [Wenn etwas nicht
funktioniert](../troubleshooting), was es sonst noch gibt.
