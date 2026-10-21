---
title: Was LosOS ist
sidebar_position: 1
---

# Was LosOS ist

LosOS macht aus einem kleinen Rechner, meist einem weiterverwendeten
Mini-PC, eine Appliance. Du schließt ihn an dein Netzwerk und an den Strom
an, installierst ihn einmal von einem USB-Stick und benutzt ihn danach nur
noch über einen Webbrowser.

## Was darauf läuft \{#what-runs-on-it}

| Was du siehst          | Was es ist                                                                                                 |
| ---------------------- | ---------------------------------------------------------------------------------------------------------- |
| **LosOS Cloud**        | Deine Dateien, Fotos, Kalender, Kontakte, Notizen und Aufgaben. Basiert auf Nextcloud. Unter `/nextcloud`. |
| **LosOS Git**          | Deine Code-Repositories, standardmäßig an. Basiert auf Forgejo. Unter `/forgejo/`.                         |
| **Die Admin-Seiten**   | Hier richtest du die Box ein und kümmerst dich um sie. Unter `/`, nur aus deinem lokalen Netzwerk erreichbar. |
| **Das Mesh**           | Optional. Freier Speicherplatz und CPU-Leistung, an andere LosOS-Boxen verliehen und von ihnen geliehen.    |
| **Fernzugriff**        | Optional. Eine öffentliche Adresse für die Box über einen Edge-Server, ohne dass zu Hause ein Port offen ist. |

## Was sie von einem NAS unterscheidet \{#what-makes-it-different-from-a-nas}

![Im Inneren einer Box: nginx ist die einzige Eingangstür; dahinter der Steuerdienst, der eigene Cluster der Box mit LosOS Cloud und LosOS Git und der Mesh-Agent; darunter eine Systemfestplatte im RAM und ein verschlüsseltes Volume.](@site/docs/img/box-architecture-de.svg)

- **Sie vergisst alles, was sie nicht braucht.** Die Box baut ihre
  Systemfestplatte bei jedem Start von Grund auf neu. Nur deine Daten, die
  Einstellungen, die du gewählt hast, und eine kurze Liste von
  Systemverzeichnissen bleiben erhalten. Sie liegen auf einem verschlüsselten
  Volume. Was ein Angreifer, ein Fehler oder ein Versehen irgendwo anders
  geändert hat, ist am nächsten Morgen weg.
- **Sie startet sich jede Nacht neu**, um 00:07, und aktualisiert sich um
  03:00. Mit diesem Neustart repariert sie sich selbst. Deshalb hat sie auch
  keine Shell. Nichts auf der Box braucht eine.
- **Sie verschlüsselt ihre Festplatte und bewahrt den Schlüssel in ihrem
  eigenen Chip auf.** Auf einem Rechner mit TPM kann niemand die Festplatte
  lesen, der sie ausbaut und an einen anderen Rechner anschließt.
- **Eine eigene Anmeldung.** Jede Person meldet sich an den Admin-Seiten mit
  ihrem eigenen Namen und Passwort an, und das Passwort des Besitzers öffnet
  auch LosOS Cloud. Mit einem ausgedruckten Ersatzschlüssel kommst du an
  einem Tag hinein, an dem LosOS Login, die Anmeldeseite der Box, nicht
  läuft.
- **Sie kann verleihen, was sie nicht braucht.** Eine Box kann ihren freien
  Speicherplatz und ihre CPU-Leistung mit anderen Boxen teilen, nur zu den
  Zeiten, die du wählst, und nur, solange sie untätig ist. Der
  [Marktplatz](../types/official-edge#what-the-edge-does-for-a-box), der
  Besitzer dafür bezahlt, ist gebaut. Er öffnet, sobald es das Unternehmen
  dahinter gibt.

## Was sie nicht ist \{#what-it-is-not}

- Kein Allzweck-Server. Ohne Shell kann niemand die Box in einen Zustand
  bringen, den niemand aufgeschrieben hat.
- Kein Backup. Die Box ist eine Kopie. Bewahre von allem, was du nicht
  verlieren darfst, eine weitere Kopie auf. Die Desktop- und Handy-Clients
  von LosOS Cloud sind ein Weg dazu.
- Nicht fertig. Der Marktplatz, ein gemeinsamer Speicherpool über mehrere
  Boxen und ein signierter Installer kommen noch.
