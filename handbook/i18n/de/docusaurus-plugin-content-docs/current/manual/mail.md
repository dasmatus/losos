---
title: Mail
sidebar_position: 4.7
slug: /manual/mail
---

# Mail

LosOS Mail ist ein Mail-Programm für die Personen auf der Box. Wer über
LosOS Login angemeldet ist, hat ein Postfach, und **Mail** in der
Seitenleiste öffnet es unter `/mail`. Der Mailserver ist Stalwart. Er läuft
auf der geteilten Seite der Box, unter dem Konto `shared`, und legt die
Postfächer im geteilten Datenverzeichnis ab.

## Wann es Mail gibt \{#when-mail-is-there}

Mail gibt es nur, solange die Box ihren Speicher mit einem Mesh teilt, dem
sie beigetreten ist (siehe [Speichermodi](../types/storage-modes)). Bei
ausgeschaltetem Teilen oder außerhalb eines Mesh:

- steht **Mail** nicht in der Seitenleiste, und `/mail` öffnet sich nicht;
- ist der Mailserver gestoppt;
- bleiben die Postfächer auf der Platte, im geteilten Verzeichnis, das die
  Box nicht lesen kann, solange das Teilen aus ist.

Teilt die Box wieder mit ihrem Mesh, ist Mail mit derselben Post zurück.

## Deine Adresse \{#your-address}

Deine Adresse ist dein Name in LosOS Login in der Mail-Domain der Box, zum
Beispiel `matus@mattbox.home.arpa`. Die Box legt dein Postfach an, wenn du
Mail zum ersten Mal öffnest. Ein eigenes Mail-Passwort gibt es nicht: Mail
ist offen, solange du angemeldet bist. Der Ersatzschlüssel öffnet die
Admin-Seiten, hat aber kein Postfach, deshalb bittet Mail dich, dich mit
deinem eigenen Konto anzumelden.

## Lesen und schreiben \{#reading-and-writing}

- **Ordner**: Posteingang, Entwürfe, Gesendet, Archiv, Spam und Papierkorb,
  daneben deine eigenen Ordner, jeweils mit der Zahl der ungelesenen
  Nachrichten. Auf einem breiten Bildschirm sind sie ein Baum links von der
  Liste, auf einem schmaleren eine Zeile darüber.
- **Suchen** durchsucht den offenen Ordner: Absender, Betreff und Text.
- **Schreiben** öffnet eine neue Nachricht. Die Felder **An** und **Cc**
  schlagen deine [Kontakte](contacts) und die anderen Personen auf der Box
  vor. **Antworten**, **Allen antworten** und
  **Weiterleiten** zitieren die Nachricht.
- **Markieren**, **Als ungelesen markieren**, **Archivieren** und
  **Verschieben nach** wirken auf die offene Nachricht. **Löschen**
  verschiebt sie in den Papierkorb, und Löschen im Papierkorb entfernt sie
  endgültig.
- Formatierte Mail öffnet sich als reiner Text. **Formatierung anzeigen**
  zeigt das Layout des Absenders in einem abgeschotteten Rahmen ohne
  Skripte, ohne Formulare und ohne Bilder von außerhalb der Box, sodass der
  Absender nicht erfährt, dass du die Nachricht geöffnet hast. Eine
  Nachricht ohne reinen Text öffnet sich gleich in diesem Rahmen.
- Anhänge werden auf deinen Rechner heruntergeladen und öffnen sich nie in
  der Seite. Was du sonst mit ihnen tun kannst, steht unter
  [Dateien und LosOS Cloud](#files-and-losos-cloud).
- Ist der Absender auf dieser Box und hat die Box Chat, beginnen **Chat**
  und **Anrufen** in der Leiste der Nachricht ein Gespräch mit ihm.
- Mit einem Rechtsklick auf eine Nachricht in der Liste antwortest du,
  markierst, archivierst oder verschiebst sie, ohne sie zu öffnen; siehe
  [das Rechtsklick-Menü](admin-pages#the-right-click-menu).

## Eigene Ordner \{#your-own-folders}

Lege eigene Ordner neben denen von Mail und darin an, so tief du willst.

- **Neuer Ordner** (das Ordnersymbol oben am Baum) fragt nach einem Namen
  und danach, wohin der Ordner kommt. Ein Name darf keinen Schrägstrich
  enthalten, und zwei Ordner an derselben Stelle dürfen nicht gleich
  heißen.
- Mit einem Rechtsklick auf einen Ordner öffnest du ihn, legst einen Ordner
  darin an, benennst ihn um, verschiebst ihn in einen anderen Ordner oder
  auf die oberste Ebene oder löschst ihn. Die Ordner von Mail lassen sich
  nur öffnen und nehmen Ordner in sich auf.
- Zieh eine Nachricht auf einen Ordner, um sie dorthin zu verschieben. Zieh
  einen deiner Ordner auf einen anderen, um ihn hineinzulegen, oder auf den
  leeren Platz unter dem Baum, um ihn auf die oberste Ebene zu holen.
- Beim Löschen eines Ordners kommen seine Nachrichten zuerst in den
  Papierkorb, sodass durch einen Fehlklick nichts verloren geht. Ein Ordner
  mit Ordnern darin wird erst gelöscht, wenn diese verschoben oder gelöscht
  sind.

## Filter \{#filters}

Filter sortieren neue Mail, sobald sie ankommt. Der Mailserver hält und
führt sie aus, deshalb wirken sie auch, wenn Mail geschlossen und niemand
angemeldet ist.

- **Filter** neben **Schreiben** zeigt sie in der Reihenfolge, in der sie
  laufen. Über eine Nachricht entscheidet der erste Filter, der passt.
- Ein Filter prüft, von wem die Nachricht ist, an wen sie geht (An oder
  Cc) oder ihren Betreff auf einen Text darin. Groß- und Kleinschreibung
  spielt keine Rolle. Er kann verlangen, dass alle Bedingungen passen oder
  eine davon.
- Dann verschiebt er die Nachricht in einen Ordner (auch in den
  Papierkorb, so löscht ein Filter), markiert sie als gelesen, markiert sie
  oder tut mehreres davon zusammen.
- Der Schalter neben einem Filter schaltet ihn aus, ohne ihn zu verlieren,
  und die Pfeile ändern die Reihenfolge.
- Mit einem Rechtsklick auf eine Nachricht und **Ähnliche Mail filtern…**
  beginnst du einen Filter für ihren Absender.
- Filter wirken auf Mail, die nach dem Speichern ankommt; Mail, die schon
  in einem Ordner liegt, bleibt, wo sie ist. Wird ein Ordner gelöscht,
  schaltet sich ein Filter aus, der nur dorthin verschoben hat.

## Dateien und LosOS Cloud \{#files-and-losos-cloud}

- **In LosOS Cloud speichern** neben einem Anhang legt ihn in den Ordner
  **Mail** deiner LosOS Cloud. **Öffnen** öffnet ihn dann in der App, die
  ihn bearbeitet: Markdown in Notizen, andere Textdateien im gemeinsamen
  Editor.
- Beim Schreiben fügen **Von diesem Computer anhängen** und **Aus LosOS
  Cloud anhängen** Dateien hinzu, zusammen bis zu 20 MB. **Weiterleiten**
  behält die Anhänge der Nachricht.
- In LosOS Cloud öffnet ein Rechtsklick auf eine Datei und **Per Mail
  senden** eine neue Nachricht mit der Datei im Anhang.

## Die KI der Box fragen \{#ask-the-boxs-ai}

Auf einer Box mit einem Sprachmodell für **Mit KI bauen** fragt
**Zusammenfassen** in der Leiste der Nachricht die KI nach der offenen
Nachricht. Das Modell läuft auf der Box, Nachricht und Antwort bleiben also
dort, und es liest nur die Nachricht, nach der du gefragt hast. Es
beantwortet eine Frage nach der anderen; auf einer kleinen Box kann eine
Antwort ein paar Minuten dauern. Text in einer Nachricht, der der KI etwas
befiehlt, zählt zur Nachricht und nicht als deine Bitte.

## Was Mail noch nicht kann \{#what-mail-does-not-do-yet}

Mail geht nur zwischen den Personen auf dieser Box. Mail an eine andere
Domain wird mit einer Meldung abgelehnt, die das sagt, und von außen kommt
nichts an. Mail aus dem Internet zu empfangen braucht eine öffentliche
Adresse und Port 25, die ein Anschluss zu Hause selten hat. Die Box bietet
auch kein IMAP oder SMTP für andere Mail-Programme: Mail in den
Admin-Seiten ist der einzige Weg hinein.
