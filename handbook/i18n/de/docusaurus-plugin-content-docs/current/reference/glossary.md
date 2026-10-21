---
title: Glossar
sidebar_position: 5
---

# Glossar

![Wo die Begriffe hingehören: Boxen, die Edge, die offizielle LosOS-Edge, eine Firmen-Edge und der Pool des Mesh.](@site/docs/img/setup-types-de.svg)

**Admin-Seiten.** Die eigene Webseite der Box unter `/`, nur im LAN
erreichbar, auf der du die Box einrichtest und dich um sie kümmerst. Sie ist
eine Single-Page-App, die die Box selbst ausliefert.

**Anwenden.** Der Druck auf die Schaltfläche, die geänderte Einstellungen in
einen Neubau der Box umsetzt.

**Pinnwand.** Das Raster der Widgets auf der Seite Übersicht, in deinem
Browser gespeichert.

**Box.** Ein Rechner, auf dem LosOS läuft. Die Admin-Seiten sagen „diese
Box“.

**Edge.** Ein Server im Internet oder im Netzwerk einer Firma, zu dem Boxen
einen Tunnel offen halten. Er gibt ihnen eine öffentliche Adresse, das Mesh
und den Marktplatz. Die **LosOS-Edge** ist die des Projekts. Eine
**Firmen-Edge** ist die von jemand anderem.

**Find my box.** Die Seite auf der LosOS-Edge, die aus einem Chrome-Browser
eine Box in deinem LAN findet.

**Keyfile-Modus.** Der Entsperrmodus der Festplatte für Rechner ohne TPM.
Der Schlüssel liegt auf der Boot-Partition.

**LosOS Cloud.** Nextcloud in den Farben der Box: Dateien, Fotos, Kalender,
Kontakte, Notizen, Aufgaben.

**LosOS Git.** Forgejo in den Farben der Box: Repositories.

**Marktplatz.** Für geteilten Speicherplatz und geteilte CPU-Leistung bezahlt
werden und selbst welche kaufen, in USDC, Bitcoin oder Monero, oder per
Karte, wo die Edge Karten annimmt. Er läuft nur auf der LosOS-Edge.

**Mesh.** Der Cluster der Boxen hinter einer Edge. Sie legen freien
Speicherplatz zusammen, den das Mesh repliziert, und freie CPU-Leistung, die
jede Box innerhalb ihrer Stunden verleiht, solange sie untätig ist.

**Overrides.** Die eine Datei mit den Einstellungen, die du gewählt hast,
`modules/overrides.nix`. Die Box ist der LosOS-Quellcode plus diese Datei.

**Neubau.** Die Box baut aus ihrer Beschreibung eine neue Version ihres
ganzen Systems und wechselt darauf. Änderungen an Einstellungen und das
Update um 03:00 sind Neubauten.

**Reserve.** Die 10 % der Festplatte, die der Installer ungenutzt gelassen
hat. Du kannst sie im Bereich Speicher beanspruchen.

**Ersatz-Admin-Schlüssel.** Der Schlüssel mit 64 Zeichen, den der Assistent
einmal anzeigt, um die Admin-Seiten zu entsperren, wenn LosOS Cloud das
Passwort nicht prüfen kann.

**TPM.** Der Sicherheitschip in der Firmware des Rechners, der im
standardmäßigen Entsperrmodus den Festplattenschlüssel enthält.

**Schleier.** Der Schieberegler im Bereich Aussehen, der die Seitenfarbe über
das Hintergrundbild legt.

**Widget.** Eine Kachel auf der Pinnwand. Eingebaut, aus Messwerten gebaut
oder von Hand geschrieben.

**Assistent.** Die drei Schritte, die die Admin-Seiten auf einer neuen Box
zeigen: dem Zertifikat vertrauen, das Passwort festlegen, anmelden.

**Zeitfenster.** Die Stunden am Tag, zwischen denen eine Box freie
CPU-Leistung verleiht.
