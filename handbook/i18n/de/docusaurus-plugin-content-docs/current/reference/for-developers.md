---
title: Für Entwickler
sidebar_position: 6
---

# Für Entwickler

Der größte Teil dieses Handbuchs ist für Besitzer. LosOS bauen, testen und
ändern ist an zwei Stellen beschrieben:

- das Kapitel [Im Detail](/in-depth/index.md): Installation im Detail,
  Administration, Mesh, Marktplatz, Virtuelle Maschinen, Master-Proxy,
  Edge-Föderation, Diagramme, Härtung, Architektur;
- [`.claude/CLAUDE.md`](https://github.com/dasmatus/losos/blob/main/.claude/CLAUDE.md) im
  Repository: die Build-Befehle, die dateiübergreifende Architektur und die
  Liste der Fehler, die man leicht noch einmal macht.

## Vor dem Commit \{#before-you-commit}

Das Repository bringt in `nix/hooks/` Git-Hooks mit, die vor einem Commit
oder Push die Prüfungen der CI für die Dateien ausführen, die du änderst.
`devenv shell` und `nix develop` schalten sie ein. Ohne beides führst du in
deinem Klon einmal Folgendes aus:

```sh
bash nix/hooks/install
```

Der Commit-Hook formatiert und prüft das Rust und Nix, das du angefasst
hast, prüft die Typen der Weboberfläche und wertet jede Ausgabe des Flakes
aus. Der Push-Hook ergänzt die Rust-Tests, die Browsertests der
Weboberfläche und die VM-Tests der geänderten Dateien. Die Protokolle jedes
Laufs liegen in `.git/losos-hooks/logs/`. Die Entwicklernotizen in
`docs/development.md` nennen jede Prüfung.

## Dieses Handbuch ändern \{#changing-this-handbook}

Die Quelle ist `handbook/` im Repository, eine
[Docusaurus](https://docusaurus.io/)-Site. Die Seiten sind Markdown unter
`handbook/docs/`. Die Seitenleiste folgt dem Verzeichnisbaum, und jeder
Ordner bekommt seinen Namen aus seiner `_category_.json`.

```sh
cd handbook
bun install --frozen-lockfile --ignore-scripts
bun run start             # live preview on http://localhost:3000
bun run build             # fails on a broken link, on purpose
```

Jeder Push auf `main` veröffentlicht die Site unter
[losos.dasmat.us](https://losos.dasmat.us)
(`.github/workflows/handbook.yml`), und jede Box baut ihre eigene Kopie
(`losos-handbook` in `nix/packages.nix`, unter `/handbook/` ausgeliefert
von `modules/containers.nix`). Docusaurus läuft auf [Bun](https://bun.sh),
nicht auf Node. Nach einer Änderung an `bun.lock` aktualisiere den `hash`
des Handbuchs in `nix/packages.nix`:

```sh
python3 .github/scripts/fix-hashes.py
```

### Bilder \{#pictures}

Die Bilder liegen in `handbook/docs/img/`. Es gibt drei Arten:

- **Admin-Seiten und Assistent.** `admin-ui/app/tests/handbook-screens.mjs`
  nimmt sie aus der gebauten SPA gegen ein nachgebildetes lososd auf, sodass
  sie die aktuelle Oberfläche und das aktuelle Logo ohne Box zeigen. Es
  schreibt PNGs in doppelter Auflösung, die auf 1600 px Breite verkleinert
  und auf 256 Farben reduziert ins Handbuch kommen. Die meisten gibt es auch
  auf Slowakisch und Deutsch, als `<name>-sk.png` und `<name>-de.png`, und
  die übersetzten Seiten zeigen diese.
- **Bildschirme eines echten Rechners.** Der Installer, das tty1-Banner, die
  Passphrase-Abfrage und die Ablehnungen durch Secure Boot stammen aus
  VM-Läufen (`nix/tests/secure-boot.nix` und Installationen in eine VM). Kein
  Skript nimmt sie neu auf. Die Bilder des Rechtsklick-Menüs stammen ebenfalls
  von einer Box in einer VM: `nix/tests/context-menu.nix` schreibt sie in allen
  drei Sprachen nach `shots/after/handbook/` in seiner Ausgabe.
- **Diagramme.** Das sind von Hand geschriebene SVGs auf einer weißen Karte,
  in der Palette der Admin-UI (`admin-ui/app/src/styles/tokens.css`), sodass
  sie im hellen und im dunklen Design gleich aussehen. Bearbeite sie als
  Text.

```sh
cd admin-ui/app
bun run build
bun tests/handbook-screens.mjs /tmp/handbook-screens           # every shot
bun tests/handbook-screens.mjs /tmp/handbook-screens mesh      # names containing "mesh"
```

:::note[Wenn Playwright kein Chromium hat]
Setz `LOSOS_CHROMIUM` auf ein Chromium-Binary, wenn das von Playwright fehlt.
:::

Schreib Alt-Texte, die sagen, was das Bild zeigt, nicht was die Seite sagt.

## Übersetzen \{#translating}

Slowakisch und Deutsch sind eingebunden. Die Texte der Oberfläche liegen in
`handbook/i18n/<locale>/code.json`. Um eine Seite zu übersetzen, kopier sie
nach `handbook/i18n/<locale>/docusaurus-plugin-content-docs/current/<same path>`
und übersetze die Kopie. Eine Seite ohne Übersetzung erscheint auf Englisch
innerhalb der übersetzten Oberfläche, du kannst also eine Seite nach der
anderen übersetzen.

:::tip[Halte die drei Sprachen im Gleichschritt]
Jede Seite ist vollständig übersetzt. Ändere alle drei Sprachen zusammen.
:::

```sh
bun run write-translations --locale sk      # refresh the string files
bun run start --locale sk                   # preview one locale
```

## Schreibregeln, denen dieses Handbuch folgt \{#writing-rules-this-handbook-follows}

- Beginne mit dem, was der Leser sieht; die Ursache kommt an zweiter, die
  Lösung an dritter Stelle.
- Jede Seite zur Fehlerbehebung beginnt mit den Schritten, für die das LAN
  genügt.
- Die IP-Adresse kommt vor dem `.local`-Namen, immer.
- Nenne den Pull Request, wenn sich ein Verhalten kürzlich geändert hat,
  damit jemand mit einer älteren Box weiß, warum sein Bildschirm anders
  aussieht.
- Keine Geviertstriche.
