---
title: KI-Werkzeuge auf deinem Computer
sidebar_position: 10.5
slug: /manual/ai-agents
---

# KI-Werkzeuge auf deinem Computer

Der Terminal-Client findet die KI-Werkzeuge auf deinem Computer und kann
den **LosOS-Begleiter** in sie einbauen. Mit dem Begleiter kann ein
Werkzeug deine Dateien in LosOS Cloud und deine Repositories in LosOS Git
durchsehen, sodass du es nach deinen eigenen Notizen und deinem Code fragen
kannst. Er kann nur schauen. Auf der Box ändert, verschiebt oder löscht er
nichts.

Das läuft auf deinem Computer, im Terminal-Client. Die Admin-Seiten sehen
nicht in deinen Computer, deshalb erstellen sie nur das Token, das der
Begleiter benutzt.

## Welche Werkzeuge

| Werkzeug | Wie der Begleiter hineinkommt |
|---|---|
| Claude Code | als Plugin, über `claude plugin` |
| GitHub Copilot CLI | als Plugin, über `copilot plugin` |
| Codex | als Plugin, über `codex plugin` |
| Gemini CLI | als Erweiterung, über `gemini extensions` |
| Qwen Code | als Erweiterung, über `qwen extensions` |
| opencode | ein Eintrag in `~/.config/opencode/opencode.json` |
| Claude Desktop | ein Eintrag in `~/.config/Claude/claude_desktop_config.json` |
| Cursor | ein Eintrag in `~/.cursor/mcp.json` |
| VS Code und VSCodium | ein Eintrag in `mcp.json` in den Benutzereinstellungen des Editors |
| Cline | ein Eintrag in seinen MCP-Einstellungen in VS Code |
| Windsurf | ein Eintrag in `~/.codeium/windsurf/mcp_config.json` |
| Zed | ein Eintrag unter `context_servers` in Zeds `settings.json` |
| LM Studio | ein Eintrag in `~/.lmstudio/mcp.json` |

Ein Werkzeug wird an seinem Programm im `PATH` oder an seinem
Einstellungsordner erkannt. `losos agents` listet, was gefunden wurde:

```sh
losos agents
```

## Bevor du anfängst

Der Terminal-Client muss schon mit der Box verbunden sein (siehe
[Der Terminal-Client](terminal)). Erstelle dann noch ein Token,
nur für den Begleiter: Wähle in **Einstellungen → Zugriffstokens** unter
**Zugriff** die Option **KI-Agenten**. Ein solches Token kann Dateien und
Repositories lesen, sonst nichts, und es funktioniert nur für den
Begleiter.

## Den Begleiter einbauen

```sh
losos agents install
```

Es fragt nacheinander nach jedem gefundenen Werkzeug. In einem Werkzeug,
zu dem du nein sagst, ändert sich nichts. Danach fragt es einmal nach dem
Token für KI-Agenten.

Nenn die Werkzeuge, um nur diese zu nehmen, und lass mit `--yes` die
Fragen weg:

```sh
losos agents install claude-code zed --yes
```

Bevor der Client eine Einstellungsdatei ändert, kopiert er sie nach
`~/.local/share/losos/companion/backups/`. Er ändert nur seinen eigenen
Eintrag und lässt deine Kommentare und deine anderen Server, wie sie waren.
Starte ein Werkzeug, das offen war, neu, damit es den Begleiter lädt.

Gemini CLI fragt, ob du dem Ordner vertraust, aus dem es installiert. Dein
Ja zu Gemini CLI beantwortet auch diese Frage: Der Client setzt nur den
eigenen Ordner des Begleiters auf die Liste der vertrauenswürdigen Ordner
von Gemini und streicht ihn wieder, wenn du den Begleiter entfernst. Gemini
CLI startet MCP-Server, auch den Begleiter, nur in Ordnern, denen du
vertraust.

## Was der Begleiter kann

Sobald er drin ist, hat das Werkzeug diese Werkzeuge vom Begleiter:

- einen Ordner in LosOS Cloud auflisten, Dateien nach Namen finden und in
  ihrem Text suchen;
- deine Repositories in LosOS Git auflisten und im Code ihrer
  Standard-Branches suchen;
- sagen, wie es der Box geht und mit welchem Konto der Begleiter liest.

Der Begleiter liest, was du lesen kannst. Jemand mit eigenem Konto auf der
Box bekommt seine eigenen Dateien, nicht deine.

## Warum das sicher ist

Der Begleiter hat einen eigenen GPG-Schlüssel, auf deinem Computer erzeugt.
Dein eigener Schlüssel unterschreibt einmal, beim Einrichten, dass dieser
Schlüssel diese eine Box mit diesem einen Token lesen darf. Die Box nimmt
den Schlüssel des Begleiters nur zusammen mit diesem Token für KI-Agenten
an, und ein Token für KI-Agenten nur zum Lesen von Dateien, Repositories
und dem Zustand der Box. Ein KI-Werkzeug hat also nie deinen eigenen
Schlüssel und erreicht viel weniger als du.

Schlüssel und Token liegen im Schlüsselbund deines Desktops, dort, wo der
Terminal-Client auch alles andere aufbewahrt.

## Wieder entfernen

```sh
losos agents remove
```

Es fragt nach jedem Werkzeug, in dem der Begleiter ist, nimmt ihn heraus
und stellt jede Einstellungsdatei wieder her, wie sie war. Sobald der
Begleiter in keinem Werkzeug mehr ist, vergisst der Client seinen Schlüssel
und sein Token. Widerrufe das Token für KI-Agenten auch unter
**Einstellungen → Zugriffstokens**.
