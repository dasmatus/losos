---
title: Passwort vergessen
sidebar_position: 5
slug: /troubleshooting/forgot-password
---

# Passwort vergessen

<div className="losos-symptom">

**Was du siehst:** „Dieses Passwort wurde nicht akzeptiert.“ im
Entsperrdialog oder auf der Anmeldeseite von LosOS cloud.

</div>

## Erst sicherstellen, dass es am Passwort liegt \{#first-make-sure-it-is-the-password}

- **LosOS cloud läuft nicht.** Der Entsperrdialog sagt dann „LosOS cloud
  läuft gerade nicht, deshalb kann das Passwort im Moment nicht geprüft
  werden“, statt das Passwort abzulehnen. Warte nach einem Neustart ein paar
  Minuten, oder nimm den Ersatzschlüssel wie unten beschrieben.
- **Eine Box von vor dem 7. Oktober 2026** lehnte auf den Admin-Seiten
  jedes Passwort ab, auch wenn LosOS cloud es annahm, weil das Passwort nie
  bei der Prüfung ankam. Pull Request #71 hat das behoben, und das
  nächtliche Update bringt die Korrektur mit. Bis sie ankommt, funktioniert
  der Ersatzschlüssel.
- **Zu viele Versuche.** Zehn Fehlversuche sperren den Rechner für eine
  wachsende Zeit aus. Warte ein paar Minuten.

## Du hast den Ersatz-Admin-Schlüssel \{#you-have-the-spare-admin-key}

![Der Entsperrdialog, der nach dem Ersatz-Admin-Schlüssel fragt.](@site/docs/img/unlock-spare-key.png)

Wähle im Entsperrdialog **Stattdessen den Ersatz-Admin-Schlüssel verwenden**
und füge den Schlüssel mit 64 Zeichen vom gedruckten Blatt ein. Die
Admin-Seiten gehen auf.

Mit dem Schlüssel kannst du außerdem ein neues Besitzer-Passwort für LosOS
cloud setzen, über denselben Weg, den der Assistent benutzt hat. In den
Bereichen gibt es dafür noch keine Schaltfläche, also führst du einen
Befehl auf einem Rechner im LAN aus. Ersetze den Schlüssel, die Adresse und
das Passwort. Das neue Passwort muss die vier Regeln erfüllen:

```bash
curl -H "Authorization: Bearer <spare key>" -H "Content-Type: application/json" \
  -d '{"password": "Correct-horse battery staple 1"}' \
  http://<address>/api/set-password
```

Die Antwort nennt das Konto, das zurückgesetzt wurde. Melde dich mit dem
neuen Passwort bei LosOS cloud an. Ab dann akzeptieren es auch die
Admin-Seiten.

## Du hast keins von beiden \{#you-have-neither}

LosOS cloud prüft das Passwort und hat die einzige Kopie davon. Einen Link
zum Zurücksetzen gibt es nicht, weil die Box keine Mails verschickt, und
auch keine Shell, auf der man ein Zurücksetzen ausführen könnte. Bevor du
neu installierst, prüf, ob ein anderes Konto in LosOS cloud die
Admin-Rolle hat. Dieses Konto kann das Passwort des Besitzers auf der Seite
Benutzer von LosOS cloud zurücksetzen. Sonst führt der einzige Weg zurück
über eine **Neuinstallation**, die die Festplatte löscht, und danach über
das Wiederherstellen deiner Dateien aus der anderen Kopie, die du
aufbewahrst, etwa dem Ordner des Desktop-Clients oder den Uploads vom
Handy.

## Damit es nicht wieder passiert \{#preventing-the-next-time}

Druck den Ersatzschlüssel aus, wenn der Assistent ihn zeigt, und bewahr
ihn nicht bei der Box auf. Leg das Passwort in einem Passwort-Manager ab.
Gib einer zweiten Person, der du vertraust, ein Admin-Konto in LosOS cloud.
