---
title: Die Admin-Seiten
sidebar_position: 1
---

# Die Admin-Seiten

Die Admin-Seiten sind die eigene Webseite der Box unter `http://<address>/`.
Die Box liefert sie nur in dein lokales Netz aus, auch wenn eine Edge die Box
veröffentlicht. Jeder Tab bittet dich, dich mit deinem eigenen Namen und
Passwort bei LosOS Login anzumelden, der Anmeldeseite der Box unter
`https://<address>/auth/`, bevor er etwas zeigt, das du ändern kannst.

## Die Bereiche \{#the-sections}

Die Seitenleiste links hat sechs Bereiche und darunter die Dinge, die du
angeheftet hast. Eingeklappt schrumpft sie zu einer Leiste aus Symbolen. Auf
dem Handy tritt an ihre Stelle eine Leiste am unteren Bildschirmrand, in
Reichweite deines Daumens: die wichtigsten Bereiche, dann **Mehr**, das die
ganze Seitenleiste von unten hochholt. Wisch sie nach unten, um sie zu
schließen. Läuft auf der Box LosOS Chat und LosOS Mail, bekommen die beiden
zwei der Plätze in der Leiste.

![Die Übersicht: links die Seitenleiste, der Name der Box und ob sie antwortet, die App-Kacheln, die Speicherkarte und das Board.](@site/docs/img/overview-de.png)

| Bereich           | Was dort ist                                                                                                 |
| ----------------- | ------------------------------------------------------------------------------------------------------------ |
| **Übersicht**     | Ob die Box antwortet und seit wann, die Speicherkarte, die App-Kacheln und dein Board mit Widgets.            |
| **Apps**          | Die Apps auf dieser Box mit ihren Einstellungen, eine Suche in Artifact Hub und die von dort installierten Apps.      |
| **Speicher**      | Wie voll die Festplatte ist, was den Platz belegt, und die Reserve, die du freigeben kannst.                  |
| **Mesh**          | Anderen Boxen beitreten, die Stunden, in denen diese Box ihre freie Zeit verleiht, **Markt** mit dem Teilen der Festplatte und **Maschinen**. |
| **Diagramme**     | LosOS Diagrams: Diagramme, Pläne und Boards als Karten, dazu das Netzwerk dieser Box gezeichnet und simuliert. |
| **Einstellungen** | Netzwerk, Aussehen, Hardware, Sicherheit, Benutzer, Zugriffstokens, Zugangsdaten, Erweitert, Historie, Sicherung, Über und Zurücksetzen, jeder davon ein eigener Bereich. |

<img src={require("@site/docs/img/overview-phone-de.png").default} alt="Die Übersicht auf einem Handy: Übersicht, Apps, Speicher und Mesh in einer Leiste am unteren Rand, alles andere unter Mehr." width="260" /> <img src={require("@site/docs/img/overview-phone-sheet-de.png").default} alt="Dasselbe Handy nach einem Tipp auf Mehr: alle Bereiche in einem Blatt, das von unten aufsteigt." width="260" />

## Suche \{#search}

Die Lupe oben in der Seitenleiste öffnet eine Liste, die Seiten, Apps,
Dateien, Repositorys und Issues findet. Strg+Umschalt+P öffnet sie auf jeder
Admin-Seite ebenfalls (Cmd+Umschalt+P auf einem Mac), und genauso ein
Doppelpunkt, den du irgendwo außerhalb eines Textfelds tippst. Sie findet:

- die Bereiche und Einstellungsbereiche, nach Namen oder nach einem Wort wie
  „gpu“ oder „hostname“, in der Sprache auf dem Bildschirm oder auf Englisch
- die Apps in LosOS Cloud und LosOS Git
- Dateien und Ordner in LosOS Cloud, nach Namen
- Repositorys in LosOS Git
- Issues in LosOS Git, nach ihren Wörtern oder ihrem Kürzel wie `BT-12`;
  tippst du nichts, die offenen Issues, die dir zugewiesen sind
- Text in Dateien, sobald du drei Buchstaben tippst: in deinen Dateien in
  LosOS Cloud und im Hauptzweig jedes Repositorys in LosOS Git
- deine angehefteten Einträge
- Apps zum Installieren aus Artifact Hub; wählst du eine, öffnet sich der
  Bereich Apps mit ihrem Installationsdialog
- ein paar Aktionen: helles oder dunkles Design, die Sprache, die Tour,
  dieses Handbuch und das Abmelden

Mit den Pfeiltasten bewegst du dich durch die Liste, Enter öffnet den
markierten Eintrag, und Escape schließt die Liste. Dateien und Repositorys
öffnen sich in ihrer eigenen App, und Strg+Enter oder Cmd+Enter öffnet einen
Eintrag in einem neuen Tab.

:::note[Die Dateisuche braucht das App-Passwort der Box]
Die Dateisuche benutzt ein App-Passwort, das die Box von LosOS Cloud
bekommt, wenn das Passwort des Besitzers gesetzt wird, und das tut der
Assistent. Fehlt es, sagt die Liste das anstelle der Dateien; siehe
[Wie die Box deine Dateien auflistet](#pins) weiter unten.
:::

Damit du in deinen Dateien in LosOS Cloud suchen kannst, führt die Box einen
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

## Das Rechtsklick-Menü \{#the-right-click-menu}

Klick mit der rechten Maustaste auf den Verwaltungsseiten, in LosOS Cloud
oder in Mail, und ein Menü zeigt, was du mit dem tun kannst, was unter dem
Zeiger liegt. Auf einem Touchscreen hältst du den Finger gedrückt. Mit der
Tastatur öffnet Shift+F10 oder die Menütaste es für das, was den Fokus hat,
und die Pfeiltasten bewegen sich darin.

- **Leere Fläche**: Zurück, Vorwärts, Neu laden, die Suche, das helle oder
  dunkle Design und dieses Handbuch.
- **Markierter Text**: Kopieren und eine Suche nach den Wörtern.
- **Ein Link**: in neuem Tab öffnen oder die Adresse kopieren.
- **Eine App-Kachel oder ein angehefteter Eintrag**: in neuem Tab öffnen,
  anheften oder lösen.
- **Eine Datei oder ein Ordner in LosOS Cloud**: öffnen, herunterladen,
  umbenennen, verschieben, kopieren, frühere Versionen, Stern, teilen, Link
  kopieren, löschen. Sind mehrere Dateien markiert, wirkt das Menü auf alle.
  Die Fläche um die Liste bietet einen neuen Ordner und das Hochladen, eine
  Datei im Papierkorb das Wiederherstellen oder endgültige Löschen.
- **Eine Nachricht in Mail**: öffnen, antworten, allen antworten,
  weiterleiten, als gelesen oder ungelesen markieren, markieren,
  archivieren, in einen anderen Ordner verschieben, löschen.
- **Eine Person in Kontakte**: Mail, Chat, Anruf oder Video, bearbeiten,
  als `.vcf` exportieren, löschen. Wer unter **Auf dieser Box** steht,
  bietet statt der letzten drei **Zu Kontakten hinzufügen**.

![Das Menü auf einer leeren Fläche der Übersicht, mit geöffnetem Erscheinungsbild: hell, dunkel und wie im Browser.](@site/docs/img/context-menu-page-de.png)

![Das Menü einer Datei in LosOS Cloud: Öffnen, Herunterladen, Umbenennen mit F2, Verschieben, Kopieren, Versionen, Zu Favoriten hinzufügen, Teilen, Link kopieren, Issue anlegen und Löschen mit Del.](@site/docs/img/context-menu-file-de.png)

Die Tasten neben einem Eintrag funktionieren auch ohne Menü. In LosOS Cloud
benennt F2 die markierte Datei um und Entf verschiebt sie in den Papierkorb
(auf dem Mac Cmd+Rücktaste). Textfelder behalten das Menü des Browsers, für
Rechtschreibung und Einfügen. Das Menü zeigt nur, was diese Box kann: Ein
Eintrag für eine Funktion, die die Box nicht hat, fehlt, statt grau zu sein.

## Die Leiste über einer Auswahl \{#the-bar-over-a-selection}

Markier Wörter in einem Markdown-Dokument oder einer Präsentation in LosOS
Office oder in einer Notiz, und sobald du die Maustaste loslässt, erscheint
darüber eine kleine Leiste. Wähl Karten oder Pfeile in Diagrams aus, und
sie erscheint über ihnen. Sie bietet an, was du am wahrscheinlichsten als
Nächstes tust, damit du es nicht in der Werkzeugleiste suchen musst.

- **Text, den du bearbeiten kannst** (LosOS Office und Notizen unter
  **Bearbeiten**): **KI fragen**, fett, kursiv, durchgestrichen,
  hervorgehoben und Code, ein Link auf eine Webseite, ein Link auf eine
  andere Notiz, eine Erwähnung einer Person oder Datei und Kopieren. Ein
  gedrückter Knopf zeigt, dass die Auszeichnung schon gesetzt ist; ein
  zweiter Druck nimmt sie weg. Ein Link fragt seine Adresse direkt in der
  Leiste ab, und ein Link auf eine Notiz oder eine Erwähnung schlägt beim
  Tippen Namen vor.
- **Eine Notiz, die du liest**: **KI fragen**, Kopieren und den Link auf
  den Abschnitt kopieren, in dem die Wörter stehen.
- **Karten und Pfeile in Diagrams**: **KI fragen**, wenn die Karten Text
  haben, eine Farbe, den Text einer Karte bearbeiten, mehrere ausrichten,
  ein Pfeil zwischen zwei Karten, gruppieren, duplizieren und löschen.
  Jeder Knopf tut dasselbe wie der im Inspektor, also nimmt ein
  Rückgängig ihn zurück.

Laufen Mail oder Chat auf der Box, schickt die Leiste über Text die Wörter
auch per Mail oder im Chat. **KI fragen** erscheint nur, solange die KI der
Box eingeschaltet ist; bei Text, den du bearbeiten kannst, setzt **Auswahl
ersetzen** die Antwort an die Stelle der Wörter.

Mit der Tastatur bringt dich Alt+F10 in die Leiste, die Pfeiltasten
bewegen sich darin, und Enter oder die Leertaste drückt einen Knopf. Escape
führt zurück in den Text oder das Diagramm und blendet die Leiste aus, bis
du etwas anderes auswählst. Tippst du über die Auswahl, verschwindet sie
auch.

## Angeheftete Einträge \{#pins}

Hefte die Apps, Dateien, Repositorys und Issues an, die du am häufigsten öffnest,
und sie stehen unter **Angeheftet** in der Seitenleiste. Ein angehefteter
Eintrag öffnet sich in seiner eigenen App: eine Datei oder ein Ordner in
LosOS Cloud, ein Repository in LosOS Git, ein Issue oder ein Projekt in den
[Issues](./issues.md) von LosOS Git, eine App auf der Übersicht oder
eine App, die du über die Suche installiert hast, auf ihrem eigenen Port.
Die Box speichert die Liste, sodass jeder Browser, mit dem du dich
anmeldest, dieselben Einträge zeigt.

- Zeige auf der Übersicht auf eine App-Kachel und klicke auf die Pinnnadel
  in ihrer Ecke. Eine angeheftete Kachel behält die Nadel, in Farbe.
- Auf der Seite Apps steht **Anheften** neben jeder installierten App.
- Der Stift neben **Angeheftet** öffnet die ganze Liste. Dort schiebst du
  einen Eintrag nach oben oder unten, löst ihn und heftest aus vier Tabs an:
  **Apps**, **Dateien**, das durch die Ordner von LosOS Cloud führt,
  **Repositorys** und **Issues**, das ein Issue nach Wörtern oder Kürzel
  findet.
- An einem Issue oder einem Projekt in LosOS Git heftet die Nadel oben es
  an. Ein Issue steht mit seinem Titel in der Seitenleiste.
- Zeige in der Seitenleiste auf einen Eintrag und klicke auf die
  durchgestrichene Nadel, um ihn zu lösen. Die [Suche](#search) findet
  angeheftete Einträge ebenfalls.
- Klicke mit der rechten Maustaste auf eine Datei, einen Ordner, ein
  Repository oder ein Issue in der Suche, auf ein Issue in einer Liste oder
  auf einem Board von LosOS Git, auf eine App-Kachel in der Übersicht oder auf
  einen Eintrag in der Seitenleiste und wähle **An die Seitenleiste
  anheften** oder **Von der Seitenleiste lösen**. Auf einem Touchscreen
  hältst du stattdessen den Finger darauf.

![Die Übersicht mit sechs Einträgen unter Angeheftet in der Seitenleiste; die angehefteten Kacheln Kalender und Notizen sind markiert.](@site/docs/img/pins-overview-de.png)

![Die Liste der angehefteten Einträge, mit dem Tab Dateien auf dem Ordner Documents von LosOS Cloud geöffnet.](@site/docs/img/pins-dialog-de.png)

:::note[Wie die Box deine Dateien auflistet]
Die Box listet deine Dateien mit einem App-Passwort auf, das sie bei LosOS
Cloud anfordert, wenn das Passwort des Besitzers gesetzt wird, und das tut
der Assistent. Das App-Passwort erscheint in den Sicherheitseinstellungen
von LosOS Cloud als „LosOS admin page“. Widerrufst du es dort, enden die
Auflistung und die Dateisuche, bis das Passwort des Besitzers wieder gesetzt
wird, über den Weg mit dem Ersatzschlüssel unter
[Passwort vergessen](../troubleshooting/forgot-password#you-have-the-spare-admin-key). Die Seitenleiste fasst 24 angeheftete Einträge.
:::

## Die Tour \{#the-tour}

Wenn du diese Seiten nach der Einrichtung zum ersten Mal öffnest, startet
über der Übersicht eine kurze Tour: sechs Schritte auf jeder Box und einer
mehr für jede Funktion, die diese Box hat ([Der erste Start](../start/first-run)
zählt sie auf). **Tour starten**
unter Einstellungen, Über öffnet sie erneut. Ist in deinem System „Bewegung
reduzieren“ eingestellt, zeigt jeder Schritt ein Standbild statt eines
bewegten.

## Der Hinweis zum Mesh \{#the-mesh-banner}

Manche Funktionen laufen nur auf einer Box, die ihren Speicher teilt und
einem Mesh beigetreten ist. Solange nicht beides gilt, sagt ein Hinweis über
jeder Seite, dass manche Funktionen vielleicht nicht verfügbar sind, mit
einem Link zu **Mesh**, um beizutreten, oder zu **Markt**, um die Festplatte
zu teilen. **Schließen** blendet ihn in diesem Browser aus. Er verschwindet
von selbst, sobald die Box beigetreten ist, und kommt zurück, wenn sie das
Mesh verlässt.

## Benachrichtigungen \{#notifications}

Kurze Statusmeldungen erscheinen oben rechts: eine Änderung, die gerade
angewendet wird, eine Änderung, die übernommen wurde, ein hinzugefügtes
Widget. Eine Frage, die deine Antwort braucht, etwa das Zurücksetzen der Box
oder das Freigeben der Reserve, öffnet stattdessen einen Dialog. Die sichere
Wahl steht an erster Stelle.

![Ein Einstellungsbereich mit einer noch nicht angewendeten Änderung: unten die Leiste mit Verwerfen und Anwenden.](@site/docs/img/settings-apply-bar-de.png)

## Ein Spiel, während du wartest \{#a-game-while-you-wait}

Manches dauert Minuten: eine Änderung anwenden, LosOS Cloud beim ersten
Start einrichten, LosOS Login starten, eine App installieren. Solange eines
davon läuft, öffnet **Spiel was, während du wartest** neben dem Fortschritt
ein kleines Spiel darunter, zufällig eines von fünf: Stromaufwärts, Aal,
Verschmelzen, Steine und Paare. Der Fortschritt bleibt über dem Spiel zu
sehen. Nichts beginnt, bevor du eine Taste drückst oder tippst, es gibt
keinen Ton, und das Spiel pausiert, wenn du in einen anderen Tab wechselst.
Ist das Warten vorbei, hält das Spiel an und sagt es, und **Spiel
schließen** räumt es weg. Mit „Bewegung reduzieren“ in deinem System kommen
nur Verschmelzen und Paare, weil sich darin zwischen den Zügen nichts
bewegt. Deinen besten Wert pro Spiel merkt sich dieser Browser.

## Design und Sprache \{#theme-and-language}

Dein Menü oben rechts, unter deinem Namen, hat **Erscheinungsbild** („Wie
im Browser“, hell oder dunkel) und **Sprache** (die des Browsers, Englisch,
Slowakisch oder Deutsch). Es ist dasselbe Menü auf jeder Seite der Box: den
Admin-Seiten, LosOS Cloud, Office, Mail, Chat und LosOS Git. Eine Seite,
auf der du nicht angemeldet bist, hat beides hinter einem Knopf in ihrer
Leiste. Dieser Browser merkt sich beides.

![Die Übersicht auf Deutsch und im dunklen Design.](@site/docs/img/overview-de-dark.png)

## Abmelden \{#sign-out}

**Abmelden** beendet deine Anmeldung auch bei LosOS Login, sodass der
nächste Besuch wieder nach Name und Passwort fragt. Hast du die Seiten mit
dem Ersatzschlüssel geöffnet, vergisst es stattdessen den Schlüssel in
diesem Tab, und das Schließen des Tabs bewirkt dasselbe. Siehe [Anmeldung
und der Ersatzschlüssel](sign-in-and-spare-key).

## Was die Admin-Seiten nicht können \{#what-the-admin-pages-cannot-do}

Es gibt keinen Dateibrowser, kein Terminal und keine Log-Anzeige. Deine
Dateien liegen in LosOS Cloud; der Tab **Dateien** in der Liste der
angehefteten Einträge zeigt Ordner nur, damit du aus ihnen anheften kannst.
Die Box hat keine Shell, also haben auch die Seiten keine. Über sich selbst
berichtet die Box auf der Übersicht, im Bereich Speicher und im Bereich
Über. Reicht das nicht, sagt [Wenn etwas nicht
funktioniert](../troubleshooting), was es sonst noch gibt.
