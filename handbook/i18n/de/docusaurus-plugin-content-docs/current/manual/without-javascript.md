---
title: Ohne JavaScript
sidebar_position: 1.5
slug: /manual/without-javascript
---

# Ohne JavaScript

Die Admin-Seiten funktionieren auch in einem Browser mit abgeschaltetem
JavaScript. Öffne die Box unter ihrer gewohnten Adresse, und der Browser
wird auf dieselbe Seite unter `/basic/` geschickt: aus
`https://<adresse>/settings/network` wird
`https://<adresse>/basic/settings/network`. Die Box zeichnet diese Seiten
selbst und schickt sie als fertiges HTML, mit derselben Seitenleiste,
derselben oberen Leiste, denselben Farben und Schriften wie die gewohnten
Seiten.

Das hilft, wenn ein Browser Skripte blockiert, etwa mit NoScript oder durch
eine Firmenrichtlinie, oder wenn die gewohnten Seiten nicht laden.

![Die Übersicht mit abgeschaltetem JavaScript: die Seitenleiste, der Name der Box und die Kacheln, so wie die gewohnte Seite sie zeichnet.](@site/docs/img/without-javascript-de.png)

## Anmelden \{#signing-in}

Melde dich mit deinem Namen und Passwort an, wie bei LosOS Login. Fragt dein
Konto nach einem Code aus einer Authenticator-App, fragt die nächste Seite
danach. Ein Passkey braucht JavaScript, ein Konto, das sich damit anmeldet,
kann sich hier also nicht anmelden. Nimm stattdessen den
Ersatz-Admin-Schlüssel. Der Browser behält ihn, bis du ihn schließt.

Diese Seiten gibt es nur über https, wie LosOS Login. Eine `http://`-Adresse
wird auf `https://` umgeleitet.

## Was funktioniert \{#what-works}

- **Übersicht.** Der Name der Box, ob sie antwortet, die Kacheln und wie
  voll die Festplatte ist.
- **Speicher.** Wie viel Platz belegt und frei ist, und das Nutzen der
  Reserve, das vorher nachfragt.
- **Netzwerk.** Der Name der Box, die Erreichbarkeit von außerhalb deines
  Zuhauses und deine eigenen Domains: eine hinzufügen, ihre zwei Einträge
  sehen und sie entfernen.
- **Hardware** und **Sicherheit.** Jeder Schalter auf diesen Bereichen.
- **Suche.** Findet eine Seite der Box über ihren Namen.
- **Design, Sprache und die Seitenleiste.** Hell, dunkel oder wie der
  Browser, die Sprache und die auf Symbole eingeklappte Seitenleiste. Der
  Browser merkt sich das.

Eine Änderung zeigt sich in der Anwenden-Leiste am Fuß des Bereichs, die die
Änderungen zählt. **Anwenden** baut die Box neu wie auf den gewohnten
Seiten, und die Seite lädt sich alle paar Sekunden selbst neu, bis der
Neubau fertig ist, und sagt dann, ob er geklappt hat. **Verwerfen** setzt
den Bereich zurück.

Seiten, die es hier noch nicht gibt, etwa Apps, Mesh oder LosOS Cloud,
sagen, dass sie JavaScript brauchen, und ändern nichts.

## Was nicht funktioniert \{#what-does-not-work}

- Die Widget-Tafel auf der Übersicht. Ihre Widgets leben im Browser und
  brauchen ein Skript.
- Alles, was sich vor deinen Augen aktualisiert, außer dem Neubau. Lade die
  Seite neu, um zu sehen, was sich geändert hat.

## Warum die Seite gleich aussieht \{#why-the-page-looks-the-same}

Die Seiten ohne JavaScript nutzen das Stylesheet und die Texte der gewohnten
Seiten. Menüs, das Einklappen der Seitenleiste und der Dialog, der vor dem
Nutzen der Reserve nachfragt, öffnen und schließen sich allein mit CSS.
Hinweise gleiten herein und gehen von selbst, ebenfalls mit CSS, und bleiben
stehen, wenn dein System weniger Bewegung wünscht.

## Dieses Handbuch \{#this-handbook}

Auch das Handbuch funktioniert ohne JavaScript. Ein Browser mit
ausgeschalteten Skripten, der eine seiner Seiten öffnet, wird auf dieselbe
Seite unter `basic/` geschickt: aus `http://<box>/handbook/manual/apps` wird
`http://<box>/handbook/basic/manual/apps`, und aus der Kopie im Internet,
`https://losos.dasmat.us/manual/apps`, wird
`https://losos.dasmat.us/basic/manual/apps`. Die Seiten, die Bilder und die
drei Sprachen sind dieselben, ganz ohne Skript.

Drei Dinge funktionieren dort anders:

- **Die Suche** braucht ein Skript, daher öffnet das Suchfeld den
  **Inhalt**: jede Seite und jeden Abschnitt darin, auf einer Seite.
  Durchsuche diese Seite mit der Suche deines Browsers, Strg+F oder ⌘F.
- **Auf dem Handy** öffnet die Menütaste dieselbe Inhaltsseite, die oben
  auch die Sprachen hat.
- **Hell oder dunkel** richtet sich nach deinem System. Einen Schalter dafür
  gibt es nicht.
