[English](Home) · [Slovenčina](Home-sk) · **Deutsch**

# losos

losos ist eine NixOS-Appliance für einen Mini-PC. Sie betreibt Nextcloud für
deine eigenen Dateien und kann freien Speicherplatz und CPU-Leistung an ein
Mesh anderer losos-Boxen verleihen. Es gibt kein SSH und keine Login-Shell.
Du verwaltest die Box über eine Webseite, und sie aktualisiert sich selbst.

Das Root-Dateisystem ist ein tmpfs, das bei jedem Start neu aufgebaut wird.
Nur die in `modules/impermanence.nix` aufgeführten Verzeichnisse bleiben
erhalten, und zwar auf einer verschlüsselten Partition `/persist`.

Das Handbuch für Besitzer ist das [Handbook](https://losos.dasmat.us)
(`handbook/` im Repository). Jede Box liefert es außerdem unter
`http://<address>/handbook/` aus, sodass es auch allein im LAN lesbar ist.
Die Seiten unten richten sich an Leute, die das Projekt bauen und betreiben.

## Seiten

- [Installation](Install-de). Das Installer-ISO, Secure Boot, TPM, das
  Demo-Image, das Vergrößern der Festplatte.
- [Administration](Administration-de). Die Admin-UI, Einstellungen, Updates
  und der nächtliche Neustart.
- [Mesh](Mesh-de). Speicher und Rechenleistung für andere Boxen bereitstellen.
- [Master-Proxy](Master-Proxy-de). Die Box aus dem Internet erreichen, ohne
  einen Port zu öffnen.
- [Edge-Föderation](Edge-Federation-de). Deine eigene Edge im LAN, weitergeleitet über die offiziellen.
- [Härtung](Hardening-de). Die standardmäßige Härtungsbasis und die
  optionalen Schalter.
- [TPM und Entsperren der Festplatte](TPM-de). Was der Chip leistet und
  worauf eine Box ohne ihn verzichtet.
- [Sicherheitsmodell](Security-Model-de). Wogegen geschützt wird und wogegen
  nicht.
- [Architektur](Architecture-de). Wie die Teile zusammenspielen.
- [Entwicklung](Development-de). Die Dev-Shell, Tests, Lock-Dateien.
- [CI und Releases](CI-and-Releases-de). CI-Jobs, Release-Medien, der
  Binary-Cache.

## Im Vergleich mit Alternativen

- **Ein NAS (Synology, QNAP).** Dieselbe Idee, eine Box mit Web-UI. Der
  Unterschied: Das Root-Dateisystem wird hier bei jedem Start verworfen,
  sodass die Änderungen eines Angreifers nur bis zum nächsten Neustart
  bestehen, und der kommt spätestens jede Nacht um 00:07. Außerdem kannst du
  alles, was auf der Box läuft, lesen und selbst neu bauen.
- **Ein VPS.** Die Box steht bei dir zu Hause, und ihre Festplatte ist
  verschlüsselt. Der Schlüssel ist im TPM-Chip der Box versiegelt oder liegt
  auf einem Rechner ohne Chip auf der Boot-Partition. Der optionale
  [Master-Proxy](Master-Proxy-de) gibt ihr eine öffentliche Adresse ohne
  eingehenden Port.
- **Reines NixOS.** Du könntest das alles selbst schreiben. Hier ist es
  bereits geschrieben, und Benutzertrennung, verschlüsselte Persistenz,
  Vergrößern der Festplatte, Härtung und die Bedingungen für das Mesh haben
  jeweils einen VM-Test.
- **Andere Self-Hosting-Lösungen.** Keine davon verleiht Kapazität an ein
  Mesh nur dann, wenn das Zeitfenster des Besitzers offen ist *und* die Box
  untätig ist.

## Was es nicht ist

- Kein Allzweck-Server. Eine Shell gibt es bewusst nicht.
- Kein Backup. Bewahre Kopien von `/persist` an anderer Stelle auf.
- Nicht fertig.
