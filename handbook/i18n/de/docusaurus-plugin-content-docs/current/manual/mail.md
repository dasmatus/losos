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
- Mit einem Rechtsklick auf eine Nachricht in der Liste antwortest du,
  markierst, archivierst oder verschiebst sie, ohne sie zu öffnen; siehe
  [das Rechtsklick-Menü](admin-pages#the-right-click-menu).

## Was Mail noch nicht kann \{#what-mail-does-not-do-yet}

Mail geht nur zwischen den Personen auf dieser Box. Mail an eine andere
Domain wird mit einer Meldung abgelehnt, die das sagt, und von außen kommt
nichts an. Mail aus dem Internet zu empfangen braucht eine öffentliche
Adresse und Port 25, die ein Anschluss zu Hause selten hat. Die Box bietet
auch kein IMAP oder SMTP für andere Mail-Programme: Mail in den
Admin-Seiten ist der einzige Weg hinein.
