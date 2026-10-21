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

Das läuft auf deinem Computer, im Terminal-Client, weil die Admin-Seiten
nicht in deinen Computer sehen. Der Begleiter meldet sich mit LosOS Login
an, der Anmeldung der Box selbst, in deinem Browser.

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

## Den Begleiter einbauen

```sh
losos agents install --host https://mattbox.local
```

Nimm die Adresse deiner Box. Ist der Terminal-Client schon mit der Box
verbunden (siehe [Der Terminal-Client](terminal)), kannst du `--host`
weglassen.

Es fragt nacheinander nach jedem gefundenen Werkzeug. In einem Werkzeug,
zu dem du nein sagst, ändert sich nichts. Nenn die Werkzeuge, um nur diese
zu nehmen, und lass mit `--yes` die Fragen weg:

```sh
losos agents install claude-code zed --yes
```

War dieser Computer noch nie mit der Box verbunden, zeigt der Client den
Fingerabdruck des Zertifikats der Box. Vergleiche ihn mit dem, den dein
Browser für die Box zeigt, und sag ja, wenn sie übereinstimmen. Von da an
spricht der Begleiter nur mit einer Box mit diesem Zertifikat.

Dann meldet sich der Begleiter an. Der Client zeigt einen Link und einen
Code:

```text
Der Begleiter meldet sich bei https://mattbox.local mit LosOS Login an. Öffne diesen Link im Browser, melde dich an und erlaube den LosOS-Begleiter:
  https://mattbox.local/auth/consent/openid/device-authorization?user_code=WDJBMJHT
Die Seite zeigt den Code WDJBMJHT. Warte auf die Freigabe…
```

Öffne den Link, melde dich bei LosOS Login an wie immer, mit deinem
zweiten Schritt, wenn du einen hast, prüfe, dass die Seite denselben Code
zeigt, und wähle **Accept**. Das Terminal macht danach von selbst weiter.

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

Der Begleiter sieht dein Passwort nie. Du gibst es in deinem Browser bei
LosOS Login ein, und LosOS Login gibt dem Begleiter zwei Tokens: eines
öffnet die Box für eine Stunde, das andere holt danach ein neues Paar.
Wird der Begleiter 30 Tage lang nicht benutzt, muss er sich neu anmelden.

Die Box lässt diese Tokens Dateien, Repositories und den Zustand der Box
lesen, sonst nichts, auch wenn du Administrator bist. Der Begleiter sendet
nur Lesezugriffe, und die Box lehnt alles andere ab. Wer bei seinem Konto
noch einen zweiten Schritt oder ein neues Passwort einrichten muss, kann
den Begleiter erst danach anmelden.

Die Tokens liegen im Schlüsselbund deiner Arbeitsumgebung, dort, wo der
Terminal-Client auch alles andere aufbewahrt. In die Einstellungen eines
KI-Werkzeugs werden sie nie geschrieben: Jedes Werkzeug startet den
Begleiter, und der Begleiter liest den Schlüsselbund.

## Wieder entfernen

```sh
losos agents remove
```

Es fragt nach jedem Werkzeug, in dem der Begleiter ist, entfernt ihn und
setzt jede Einstellungsdatei zurück, wie sie war. Ist der Begleiter in
keinem Werkzeug mehr, meldet der Client ihn ab: Er gibt beide Tokens an
LosOS Login zurück und vergisst sie. Die Box nimmt sie spätestens nach
einer Minute nicht mehr an.
