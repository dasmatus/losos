---
title: Protokolle
sidebar_position: 6.5
slug: /manual/logs
---

# Protokolle

**Einstellungen › Protokolle** zeigt, was die Dienste der Box ins
Systemjournal geschrieben haben: der Steuerdienst, LosOS Login, der
Webserver, LosOS Cloud, LosOS Git, die Apps, Backups, Updates und der Rest.
Wenn etwas fehlschlägt, sagt die Box hier, warum, und du liest es im
Browser. Niemand muss sich an der Box selbst anmelden.

Nur ein Administrator oder wer den Ersatzschlüssel hat, kann den Bereich
öffnen. Die Zeilen verraten viel über alle, die die Box nutzen.

## Die Zeilen lesen

Der Bereich öffnet mit den Zeilen des Steuerdienstes seit dem letzten Start
der Box, die neuesten unten. Jede Zeile zeigt die Uhrzeit, den Schweregrad,
den Dienst, der sie geschrieben hat, und was er sagte. Fährst du mit der
Maus über den Dienst, siehst du seinen vollen Unit-Namen und die
Prozessnummer.

Der Filter über den Zeilen wählt:

- **Dienst.** Einer der Dienste der Box, **Alle LosOS-Dienste** oder **Das
  ganze Journal**. In der Liste stehen nur die Dienste, die diese Box
  ausführt.
- **Schweregrad.** **Fehler**, **Warnungen und Fehler** und so weiter bis
  **Alles**, mit den Debug-Zeilen.
- **Seit.** Dieser Start, der Start davor, die letzte Stunde, der letzte Tag
  oder die letzte Woche.
- **Enthält.** Nur Zeilen mit diesem Text, ohne auf Groß- und
  Kleinschreibung zu achten.

**Ältere Zeilen anzeigen** oben holt die Zeilen vor der ersten auf dem
Bildschirm. **Mitlesen** fügt neue Zeilen hinzu, sobald die Dienste sie
schreiben, bis du **Mitlesen beenden** drückst. **Herunterladen** speichert
bis zu 5000 Zeilen des aktuellen Filters als Textdatei, etwa für einen
Fehlerbericht.

Der Filter steht in der Adresse, also öffnet ein Link wie
`https://<adresse>/settings/logs?unit=lososd&priority=3` die Fehler des
Steuerdienstes.

## Was ausgeblendet wird

Bevor eine Zeile das Journal verlässt, blendet die Box alles aus, was wie
ein Geheimnis aussieht, und zeigt an seiner Stelle `[hidden]`: Passwörter
und Tokens in der Form `password=…` oder `"token": "…"`, die Zugangsdaten in
einem `Authorization`- oder `Cookie`-Header, ein Passwort in einer Adresse,
private Schlüssel, bekannte Tokenformate und lange Folgen aus Hex oder
Base64, wie Schlüssel meist aussehen. Der Ersatzschlüssel wird überall
ausgeblendet, wo er auftaucht. Das gilt auch für die Suche und den
Download, also findet keines von beiden ein Geheimnis über seine Zeichen.

Im Zweifel blendet sie mehr aus: auch eine lange Prüfsumme.

## Wenn die Verwaltungsseiten nicht laden

Die Protokolle kommen von einem kleinen eigenen Leser, nicht vom
Steuerdienst, also funktioniert der Bereich auch, wenn der Steuerdienst
steht oder neu startet. Genau dann brauchst du ihn oft.

Wenn die anderen Seiten warten oder einen Fehler zeigen, öffne
`https://<adresse>/basic/settings/logs`. Diese Seite braucht weder
JavaScript noch den Steuerdienst. Läuft LosOS Login, meldest du dich wie
gewohnt an; die Seite nimmt auch den Ersatzschlüssel direkt. Sie sagt, wenn
der Steuerdienst nicht antwortet, und verlinkt seine Zeilen.

## Was fehlt

Das Installationsprogramm schreibt auf den Bildschirm des Rechners, auf dem
es läuft, nicht ins Journal, also sind seine Meldungen nicht auf der
installierten Box. Wie die Installation ausging, zeigt es auf diesem
Bildschirm.
