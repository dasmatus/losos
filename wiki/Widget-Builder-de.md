[English](Widget-Builder) · [Slovenčina](Widget-Builder-sk) · **Deutsch**

# Widget-Baukasten

Der Besitzer beschreibt ein Widget in eigenen Worten, und Claude schreibt es.
Der Widget-Editor (**Widget hinzufügen**, dann **Eins schreiben**) hat neben
**Schreiben** einen Tab **Mit Claude bauen**. Das Ergebnis landet im
Quelltextfeld des Editors, wo der Besitzer es lesen, in der Vorschau
ausprobieren und ändern kann. Nichts wird gespeichert, bevor der Besitzer auf
Speichern drückt, und ein Widget von Claude läuft im selben abgeschotteten
Rahmen wie ein selbst geschriebenes (siehe
[Administration](Administration-de#selbst-geschriebene-widgets)).

Der Baukasten ist eine Funktion des Edge. Er ist standardmäßig aus, auf drei
Ebenen.

|                          | Standard | Schalter                                 |
| ------------------------ | -------- | ---------------------------------------- |
| Der Edge bietet ihn an   | aus      | `losos.edge.builder.enable`              |
| Eine Box darf ihn nutzen | aus      | `losos.edge.tenants.<id>.market`         |
| Ein Bau kann beginnen    | nein     | das Guthaben der Box deckt einen         |

Eine Box fragt nur einen offiziellen Edge, wie beim Marktplatz, weil die
Anfrage das Proxy-Token der Box trägt.

## Wie ein Bau abläuft

Der Edge betreibt einen [Claude Managed Agent](https://platform.claude.com/docs/en/managed-agents/overview):
einen Agenten mit festem Systemprompt und dem Widget-Vertrag, den der
Betreiber einmal anlegt, und pro Bau einen Cloud-Container, den Anthropic
hostet. Für jeden Bau öffnet der Edge eine Sitzung mit der Beschreibung des
Besitzers, der Sprache der Seite und, wenn der Besitzer eine Änderung
wünscht, dem aktuellen Quelltext. Der Agent schreibt `widget.html` und eine
kurze `summary.txt`; wenn die Sitzung ruht, lädt der Edge beide herunter,
löscht Sitzung und Dateien und gibt den Quelltext an die Box.

Der Agent hat in seinem eigenen Container Werkzeuge für Dateien und Shell und
keinen Webzugriff. Sein Container erreicht die Box nicht. Ein Bau, der länger
als 15 Minuten läuft, wird angehalten.

## Was es kostet

Der Besitzer zahlt den Listenpreis von Anthropic pro Million Eingabe- und
Ausgabe-Tokens des Modells (Claude Opus 5.5, 4 $ Eingabe und 20 $ Ausgabe)
plus einen Aufschlag, den der Betreiber festlegt (`markupBps`, Standard 2000,
also 20 %). Cache-Lese- und -Schreibzugriffe werden zu ihren eigenen
Listenpreisen mit demselben Aufschlag berechnet. Containerzeit wird nicht
weitergegeben. Der Edge rechnet Dollar mit `usdRate` in die Währung des
Marktplatzes um.

Bezahlt wird im Voraus. Eine Box lädt ihr Guthaben mit einem der
Guthabenpakete des Edge auf (`packs`, Standard 5, 10 und 20 in der Währung
des Marktplatzes), über Stripe Checkout auf dem eigenen Stripe-Konto des
Edge, demselben, das der Marktplatz nutzt. Das Guthaben ändert sich, wenn der
signierte Webhook `checkout.session.completed` von Stripe beim Edge ankommt.

Jeder Bau läuft mit einem festen Budget der Sitzung: so viel, wie das
Guthaben zum Listenpreis deckt, höchstens `maxBuildCents` (Standard 3 $).
Anthropic hält den Agenten an, wenn das Budget erreicht ist, und der Besitzer
sieht, dass das Widget unfertig sein kann. Berechnet wird, was die Sitzung
verbraucht hat, auf einen Cent aufgerundet, auch wenn der Agent ohne
brauchbares Widget endet. Ein Bau, dessen Sitzung der Edge nicht mehr lesen
kann, wird nicht berechnet.

## Einrichtung für Betreiber

1. Legen Sie in der Claude Console einen API-Schlüssel in einem eigenen
   Workspace mit Ausgabenlimit an. Dieses Limit hat das letzte Wort darüber,
   was ein Fehler kosten kann.
2. Versiegeln Sie den Schlüssel mit `systemd-creds` und lesen Sie ihn von
   stdin, damit der Klartext nie die Platte berührt:

   ```sh
   systemd-creds encrypt --name=claude-api-key - /var/secrets/losos-claude-api-key.cred
   ```

   Der Pfad ist `losos.edge.builder.claudeKeySealed`. Der Name muss genau
   `claude-api-key` lauten.
3. Legen Sie Agent und Umgebung einmal an:

   ```sh
   systemd-run --pipe --wait \
     -p LoadCredentialEncrypted=claude-api-key:/var/secrets/losos-claude-api-key.cred \
     sh -c 'losos-registrar builder-setup --key-file "$CREDENTIALS_DIRECTORY/claude-api-key"'
   ```

   Es gibt die zwei Zeilen für die Konfiguration des Edge aus:
   `losos.edge.builder.agentId` und `losos.edge.builder.environmentId`. Nach
   einem Upgrade führen Sie es erneut mit `--agent-id` und
   `--environment-id` aus, um den Agenten an Ort und Stelle zu aktualisieren.
4. Schalten Sie den Marktplatz ein (`losos.edge.market.enable`, siehe
   [Marktplatz](Market-de#einrichtung-für-betreiber)), dessen Stripe-Gate die
   Aufladungen annimmt, und setzen Sie dann
   `losos.edge.builder.enable = true`. Die Registrar-Unit startet ohne den
   versiegelten Schlüssel nicht, versiegeln Sie ihn also zuerst.
5. Setzen Sie `losos.edge.tenants.<id>.market = true` für jede Box, die ihn
   nutzen darf.

Beginnen Sie im **Testmodus** von Stripe. Die Tests des Edge laufen gegen
Stellvertreter für Stripe und Anthropic, die prüfen, was der Edge sendet und
wie er reagiert, aber nicht sagen können, ob einer der Dienste es annimmt.

## Optionen

| Option                               | Standard                                 |
| ------------------------------------ | ---------------------------------------- |
| `losos.edge.builder.enable`          | `false`                                  |
| `losos.edge.builder.claudeKeySealed` | `/var/secrets/losos-claude-api-key.cred` |
| `losos.edge.builder.agentId`         | keine, erforderlich                      |
| `losos.edge.builder.environmentId`   | keine, erforderlich                      |
| `losos.edge.builder.markupBps`       | `2000` (20 %)                            |
| `losos.edge.builder.usdRate`         | `"1.0"`                                  |
| `losos.edge.builder.packs`           | `[ 500 1000 2000 ]`                      |
| `losos.edge.builder.maxBuildCents`   | `300`                                    |

## Sicherheitseigenschaften

- Den Anthropic-Schlüssel hält nur die Registrar-Unit des Edge, als
  systemd-Credential, das bei jeder Anfrage gelesen wird. Boxen und Browser
  sehen ihn nie.
- Der Stripe-Schlüssel bleibt im Gate des Marktplatzes. Das Gate nimmt einen
  Guthaben-Checkout nur für einen Betrag aus den Paketen des Betreibers an, in
  der Währung des Edge, ohne Zielkonto und ohne Gebühr.
- Das Guthaben ändert sich nur durch einen signierten Webhook, der in ID,
  Sitzung, Betrag und Währung zur erfassten Bestellung passt, und jede
  Bestellung wird einmal gutgeschrieben.
- Eine Box führt einen Bau zur Zeit aus, der Edge höchstens acht.
- Das Widget ist Text. Die Box legt es in den Editor, und nichts führt es
  außerhalb des abgeschotteten Widget-Rahmens aus.

## API

Die Box erreicht den Baukasten über lososd, das ihr Proxy-Token hinzufügt:

| Route auf der Box                   | Route auf dem Edge      |
| ----------------------------------- | ----------------------- |
| `GET /api/builder`                  | `POST /builder/account` |
| `POST /api/builder/credits`         | `POST /builder/credits` |
| `POST /api/builder/builds`          | `POST /builder/builds`  |
| `GET /api/builder/builds/{id}`      | `POST /builder/build`   |

Die Formen stehen in `backend/schema.json`.
