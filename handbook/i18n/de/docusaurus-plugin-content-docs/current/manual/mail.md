---
title: Mail
sidebar_position: 4.7
slug: /manual/mail
---

# Mail

![LosOS Mail: die Ordner, der Posteingang mit drei Nachrichten und oben Verschlüsselung, Filter und Schreiben.](@site/docs/img/mail-inbox-de.png)

![Eine Nachricht von Eva, neben dem Posteingang geöffnet, darüber die Tasten zum Antworten, Weiterleiten, Verschieben und Löschen.](@site/docs/img/mail-message-de.png)

LosOS Mail ist ein Mail-Programm für die Personen auf der Box. Wer über
LosOS Login angemeldet ist, hat ein Postfach, und **Mail** in der
Seitenleiste öffnet es unter `/mail`. Der Mailserver ist Stalwart 1.0. Er
läuft auf der geteilten Seite der Box, unter dem Konto `shared`, legt die
Postfächer im geteilten Datenverzeichnis ab und antwortet nur auf der Box
selbst: Aus dem Netz erreicht ihn nichts.

## Wann es Mail gibt \{#when-mail-is-there}

Mail gibt es nur, solange die Box ihren Speicher mit einem Mesh teilt, dem
sie beigetreten ist (siehe [Speichermodi](../types/storage-modes)). Bei
ausgeschaltetem Teilen oder außerhalb eines Mesh:

- steht **Mail** nicht in der Seitenleiste, und `/mail` öffnet sich nicht;
- ist der Mailserver gestoppt;
- bleiben die Postfächer unverändert auf der verschlüsselten Platte der Box.

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

**Zusammenfassen** in der Leiste der Nachricht fragt das eigene Sprachmodell
der Box nach der offenen Nachricht. Das Modell läuft auf der Box, Nachricht
und Antwort bleiben also dort, und es liest nur die Nachricht, nach der du
gefragt hast. Es beantwortet eine Frage nach der anderen; auf einer kleinen
Box kann eine Antwort ein, zwei Minuten dauern. Text in einer Nachricht, der
der KI etwas befiehlt, zählt zur Nachricht und nicht als deine Bitte.

Fragen zu Mail, Dateien und Notizen gehen an Gemma 3 1B, ein Modell für
Text. **Mit KI bauen** behält ein Modell für Code. Die Box lädt jedes Modell
einmal herunter, wenn sie zum ersten Mal online ist, und hält immer nur
eines im Speicher. Eine Administratorin oder ein Administrator kann unter
**Einstellungen**, **Erweitert** ein anderes Modell wählen
(`assistant.model`). Gemma is provided under and subject to the Gemma Terms
of Use found at ai.google.dev/gemma/terms.

## Verschlüsselung \{#encryption}

Mail kann Nachrichten mit OpenPGP verschlüsseln und signieren, dem
Standard, den GnuPG, Thunderbird und die meisten Mailprogramme lesen.

**Dein Schlüssel.** **Verschlüsselung** neben **Schreiben** erstellt einen
Schlüssel für deine Adresse oder importiert einen, den du schon hast. Du
wählst eine Passphrase dafür; sie ist nicht dein Anmeldepasswort. Der
Schlüssel wird in deinem Browser erstellt und entsperrt. Die Box bewahrt
den privaten Schlüssel nur gesperrt mit deiner Passphrase auf und versiegelt
ihn zusätzlich an ihr TPM oder ihren eigenen Schlüssel, wie ihre anderen
Geheimnisse (siehe [Zugangsdaten](../reference/credentials)). Ohne die Passphrase kann
niemand auf der Box deine verschlüsselte Mail lesen, auch kein
Administrator. Dasselbe Fenster zeigt den Fingerabdruck, lädt eine
Sicherung (weiterhin mit deiner Passphrase gesperrt) und den öffentlichen
Schlüssel herunter, sperrt den Schlüssel wieder und entfernt ihn. Ein
entfernter Schlüssel kann an ihn geschickte Mail erst wieder lesen, wenn du
die Sicherung importierst.

![Das Fenster Verschlüsselung mit einem Schlüssel: Fingerabdruck, Adresse und Datum, dazu die Tasten zum Sperren, Sichern und Entfernen.](@site/docs/img/mail-pgp-key.png)

**Entsperren.** Der Schlüssel bleibt in dem Tab entsperrt, in dem du ihn
entsperrt hast, bis du ihn sperrst oder den Tab schließt. Mail fragt nach
der Passphrase, wenn sie gebraucht wird: um eine verschlüsselte Nachricht
zu öffnen oder um eine zu signieren.

**Schreiben.** Unter der Nachricht schaltest du **Verschlüsseln** und
**Signieren** ein oder aus. Während du eine Adresse tippst, sucht Mail den
Schlüssel dieser Person und zeigt, woher er kommt:

- jemand auf dieser Box: der Schlüssel, den die Person hier erstellt hat;
- deine Kontakte: ein beim Kontakt gespeicherter Schlüssel;
- frühere Mail: ein Schlüssel, den die Person in einem Autocrypt-Header
  geschickt hat, gelernt, als du ihre Mail geöffnet hast;
- ihre Maildomain: das Web Key Directory (WKD) der Domain;
- keys.openpgp.org, wenn ihre Domain keines hat. Ein Administrator kann mit
  `losos.mail.keyserver` einen anderen Schlüsselserver eintragen oder ihn
  abschalten.

Hat jeder Empfänger einen Schlüssel, schaltet sich **Verschlüsseln** von
selbst ein. Fehlt einer, sagt Mail, bei wem, und die Nachricht geht erst
raus, wenn du die Verschlüsselung ausschaltest oder die Person weglässt.
Verschlüsselte Mail wird auch an deinen eigenen Schlüssel verschlüsselt,
damit du deine Kopie unter Gesendet lesen kannst. Der Betreff wird nicht
verschlüsselt. Mail wird als PGP/MIME verschickt; **Inline, für ältere
Mailprogramme** setzt den verschlüsselten Text stattdessen in den Inhalt,
was den Text schützt, aber nicht die Anhänge.

![Eine neue Nachricht an Eva: Evas Schlüssel wurde auf dieser Box gefunden, Verschlüsseln und Signieren sind an.](@site/docs/img/mail-pgp-compose.png)

**Lesen.** Eine verschlüsselte Nachricht hat in der Liste ein Schloss. Beim
Öffnen wird sie in deinem Browser entschlüsselt, und eine Zeile unter den
Kopfzeilen sagt, wie sie kam: verschlüsselt, signiert und von wem, signiert
mit einem Schlüssel, den die Box nicht kennt, oder mit einer Signatur, die
nicht passt. Eine Nachricht, die jemand anderes als ihr Absender signiert
hat, sagt das. Entschlüsselte Mail wird als Text gezeigt, nie in der
Formatierung des Absenders, und ihre Anhänge werden wie alle anderen
heruntergeladen.

![Eva liest die Nachricht: ein Schloss in der Liste und der Hinweis, dass Matus sie verschlüsselt und signiert hat, mit Matus' Fingerabdruck.](@site/docs/img/mail-pgp-opened.png)

**Dein öffentlicher Schlüssel für andere.** Die Box veröffentlicht den
öffentlichen Schlüssel jeder Person in ihrem eigenen Web Key Directory
unter `/.well-known/openpgpkey/`, damit Mailprogramme anderswo ihn über
deine Adresse finden. Jede Nachricht, die du schickst, trägt ihn außerdem
in einem Autocrypt-Header.

## Umzug auf Stalwart 1.0 \{#moving-to-stalwart-10}

Eine Box, die schon vor dieser Version Mail hatte, kopiert sie beim ersten
Start nach dem Update selbst auf den neuen Server: Ordner, Nachrichten,
Absenderadressen und Filter jeder Person. Mail funktioniert während des
Kopierens, und Post, die in der Zeit ankommt, bleibt. Hält die Box mitten
drin an, macht das Kopieren beim nächsten Start weiter, ohne etwas doppelt
anzulegen. Die alten Postfächer bleiben neben den neuen im Ordner `db-0.15`,
und nichts darin wird gelöscht.

## Sicherungen \{#backups}

[Sicherungen](backup-and-erase) enthalten auch Mail. Der Mailserver
pausiert für die paar Sekunden, die die Box zum Kopieren der Postfächer
braucht, und startet dann wieder. In diesen Sekunden bittet Mail dich, es
gleich noch einmal zu versuchen.

## Was Mail noch nicht kann \{#what-mail-does-not-do-yet}

Mail geht nur zwischen den Personen auf dieser Box. Mail an eine andere
Domain wird mit einer Meldung abgelehnt, die das sagt, und von außen kommt
nichts an. Mail aus dem Internet zu empfangen braucht eine öffentliche
Adresse und Port 25, die ein Anschluss zu Hause selten hat. Die Box bietet
auch kein IMAP oder SMTP für andere Mail-Programme: Mail in den
Admin-Seiten ist der einzige Weg hinein.
