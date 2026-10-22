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
  jeweils mit der Zahl der ungelesenen Nachrichten.
- **Suchen** durchsucht den offenen Ordner: Absender, Betreff und Text.
- **Schreiben** öffnet eine neue Nachricht. Das Feld **An** schlägt die
  anderen Personen auf der Box vor. **Antworten**, **Allen antworten** und
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
- Anhänge werden auf deinen Rechner heruntergeladen. Sie öffnen sich nie in
  der Seite.

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

**Lesen.** Eine verschlüsselte Nachricht hat in der Liste ein Schloss. Beim
Öffnen wird sie in deinem Browser entschlüsselt, und eine Zeile unter den
Kopfzeilen sagt, wie sie kam: verschlüsselt, signiert und von wem, signiert
mit einem Schlüssel, den die Box nicht kennt, oder mit einer Signatur, die
nicht passt. Eine Nachricht, die jemand anderes als ihr Absender signiert
hat, sagt das. Entschlüsselte Mail wird als Text gezeigt, nie in der
Formatierung des Absenders, und ihre Anhänge werden wie alle anderen
heruntergeladen.

**Dein öffentlicher Schlüssel für andere.** Die Box veröffentlicht den
öffentlichen Schlüssel jeder Person in ihrem eigenen Web Key Directory
unter `/.well-known/openpgpkey/`, damit Mailprogramme anderswo ihn über
deine Adresse finden. Jede Nachricht, die du schickst, trägt ihn außerdem
in einem Autocrypt-Header.

## Was Mail noch nicht kann \{#what-mail-does-not-do-yet}

Mail geht nur zwischen den Personen auf dieser Box. Mail an eine andere
Domain wird mit einer Meldung abgelehnt, die das sagt, und von außen kommt
nichts an. Mail aus dem Internet zu empfangen braucht eine öffentliche
Adresse und Port 25, die ein Anschluss zu Hause selten hat. Die Box bietet
auch kein IMAP oder SMTP für andere Mail-Programme: Mail in den
Admin-Seiten ist der einzige Weg hinein.
