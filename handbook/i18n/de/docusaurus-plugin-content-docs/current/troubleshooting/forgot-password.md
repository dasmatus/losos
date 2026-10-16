---
title: Passwort vergessen
sidebar_position: 5
slug: /troubleshooting/forgot-password
---

# Passwort vergessen

<div className="losos-symptom">

**Was du siehst:** LosOS Login, die Anmeldeseite der Box unter
`https://<box>/auth/`, nimmt deinen Anmeldenamen und dein Passwort nicht an,
oder die Anmeldeseite von LosOS cloud nimmt das Passwort nicht an.

</div>

## Erst sicherstellen, dass es am Passwort liegt \{#first-make-sure-it-is-the-password}

- **Der Anmeldename.** Anmeldenamen sind kleingeschrieben. Prüf den Namen
  genauso wie das Passwort.
- **Zu viele Versuche.** Nach einigen falschen Passwörtern hintereinander
  weist LosOS Login diesen Namen ein paar Minuten lang ab, auch mit dem
  richtigen Passwort. Warte und versuch es dann noch einmal.
- **Eine früher eingerichtete Box** hat noch keine Anmeldenamen. Ihr Dialog
  sagt **Lege deine Anmeldung an** und braucht den Ersatz-Admin-Schlüssel;
  siehe
  [Eine früher eingerichtete Box](../manual/sign-in-and-spare-key#a-box-set-up-earlier).

## Jemand anderes hat die Box eingerichtet \{#someone-else-set-up-the-box}

Frag diese Person. Die Admin-Seiten haben noch keine Seite zum Verwalten von
Personen, also lässt sich nur das Passwort des Besitzers neu setzen, mit dem
Ersatzschlüssel wie unten beschrieben.

## Du hast den Ersatz-Admin-Schlüssel \{#you-have-the-spare-admin-key}

![Der Dialog, der nach dem Ersatz-Admin-Schlüssel fragt.](@site/docs/img/unlock-spare-key-de.png)

Wähle im Dialog **An dieser Box anmelden** die Option **Stattdessen den
Ersatz-Admin-Schlüssel verwenden** und füge den Schlüssel mit 64 Zeichen vom
gedruckten Blatt ein. Die Admin-Seiten gehen auf.

Mit dem Schlüssel kannst du außerdem ein neues Passwort für den Besitzer
setzen, also für die Person, die die Box eingerichtet hat, über denselben
Weg, den der Assistent benutzt hat. Er ändert das Anmeldepasswort des
Besitzers und sein Passwort für LosOS cloud zusammen. In den Bereichen gibt
es dafür noch keine Schaltfläche, also führst du einen Befehl auf einem
Rechner im LAN aus. Ersetze den Schlüssel, die Adresse und das Passwort. Das
neue Passwort muss die vier Regeln erfüllen:

```bash
curl -H "Authorization: Bearer <spare key>" -H "Content-Type: application/json" \
  -d '{"password": "Correct-horse battery staple 1"}' \
  http://<address>/api/set-password
```

Die Antwort nennt das Konto in LosOS cloud, das zurückgesetzt wurde. Melde
dich bei LosOS Login mit deinem Anmeldenamen und dem neuen Passwort an und
bei LosOS cloud mit demselben Passwort.

## Du hast keins von beiden \{#you-have-neither}

Die Box behält dein Passwort nur als Hash. Einen Link zum Zurücksetzen gibt
es nicht, weil die Box keine Mails verschickt, und auch keine Shell, auf der
man ein Zurücksetzen ausführen könnte. Bevor du neu installierst, prüf, ob
ein anderes Konto in LosOS cloud die Admin-Rolle hat. Dieses Konto kann das
Passwort des Besitzers auf der Seite Benutzer von LosOS cloud zurücksetzen.
Damit kommst du wieder an deine Dateien, aber nicht in die Admin-Seiten. Für
die Admin-Seiten führt der einzige Weg zurück über eine
**Neuinstallation**, die die Festplatte löscht, und danach über das
Wiederherstellen deiner Dateien aus der anderen Kopie, die du aufbewahrst,
etwa dem Ordner des Desktop-Clients oder den Uploads vom Handy.

## Damit es nicht wieder passiert \{#preventing-the-next-time}

Druck den Ersatzschlüssel aus, wenn der Assistent ihn zeigt, und bewahr
ihn nicht bei der Box auf. Leg das Passwort in einem Passwort-Manager ab.
Gib einer zweiten Person, der du vertraust, ein Admin-Konto in LosOS cloud.
