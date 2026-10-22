---
title: App-Format
sidebar_position: 4.8
mdx:
  format: md
---

# App-Format

Eine App für eine LosOS-Box ist eine Compose-Datei und die Dateien daneben.
Die Box führt sie als `notshared` auf Podman ohne root aus, mit Podman
Compose, und braucht dafür keinen Cluster. Dasselbe Format dient zwei
Quellen:

- Apps, die ein Besitzer oder ein Programm in seinem Auftrag an die Box
  schickt,
- die eigenen Apps der Box, in Nix mit `losos.apps.builtin` deklariert.

Ein Chart aus der Suche im Bereich **Apps**, als `notshared` installiert,
landet am selben Ort: Die Box rendert es mit `helm template` und macht aus
seinen Pods ein Compose-Projekt dieser Form.

## Die Datei

```yaml
x-losos:
  port: 8080            # der Port, auf dem die Seite der App lauscht
services:
  web:
    image: docker.io/library/nginx:1.27-alpine
    command: ["nginx", "-g", "daemon off;", "-c", "/site/nginx.conf"]
    volumes:
      - ./site:/site:ro       # ein Ordner, den die App mitbringt
      - data:/var/lib/board   # die Daten der App
  db:
    image: docker.io/library/postgres:17
    environment:
      PGPORT: "5433"
    volumes:
      - db:/var/lib/postgresql/data
volumes:
  data: {}
  db: {}
```

`x-losos` ist der eigene Block der Box:

| Schlüssel | Bedeutung |
|---|---|
| `port` | Der Port, auf dem die Seite der App lauscht. Der Link **Öffnen** und die Weiterleitung im LAN gehen dorthin. Ohne ihn wählt die Box einen Port aus `ports` oder `expose` und bevorzugt einen Dienst namens `web` oder `ui`. |
| `images` | Image-Archive im Nix-Store, geladen bevor die App startet. Für Apps, die mit Nix gebaut sind; eine App von anderswo holt ihre Images aus einer Registry. |

## Was die Box damit macht

Jeder Dienst läuft:

- im eigenen Netz der Box (`network_mode: host`), daher sagen `ports` und
  `expose` der Box nur, wo die App lauscht;
- als uid 1000 im Container und als `notshared` außerhalb
  (`userns_mode: keep-id`), sodass alles, was die App schreibt, `notshared`
  gehört;
- ohne Capabilities und mit `no-new-privileges`;
- neu gestartet, solange ihn niemand angehalten hat, wenn die Datei nichts
  anderes sagt.

Dienste finden einander über ihren Namen: Jeder Dienstname zeigt auf
`127.0.0.1`. Weil sie ein Netz teilen, müssen sie auf verschiedenen Ports
lauschen, alle ab 1024, und auf keinem der Ports, die die Box für sich
behält (ihr Webserver, ihre Admin-API auf 8082, LosOS Git auf 3000,
PostgreSQL auf 5432, Redis auf 6379, die Ports des Clusters, der
Weiterleitungsbereich 30000 bis 30099).

Volumes:

| In der Datei | Auf der Box |
|---|---|
| `name:/path`, mit `name` unter `volumes:` | `/home/notshared/data/apps/<app>/<name>`, bleibt, wenn die App entfernt wird |
| `./path:/path` | eine Datei oder ein Ordner, den die App mitbringt, aus ihrem Projektordner |
| `/path` allein | ein anonymes Volume im Image-Speicher, verschwindet mit dem Container |
| `type: tmpfs` | Arbeitsspeicher |

`secrets:` und `configs:` mit einer `file:`, die die App mitbringt, werden
schreibgeschützt unter `/run/secrets/<name>` und `/<name>` eingebunden.
`env_file` muss eine Datei nennen, die die App mitbringt.

Die Box lehnt ab, mit einem Satz, der den Dienst und den Schlüssel nennt:
`privileged`, `cap_add`, `devices`, `pid` oder `ipc`, jeden `network_mode`
außer `host`, `build` (Apps werden nicht auf der Box gebaut), `extends`,
`volumes_from`, `sysctls`, einen Pfad außerhalb der App (`/etc`, `../`),
ein benanntes Volume mit Treiber oder mit `external`, einen Port unter 1024
oder einen, den die Box behält, und einen Port, den eine andere App nutzt.

## Eine installieren

`POST /api/apps/compose` nimmt die Datei und die Dateien daneben:

```json
{
  "release": "board",
  "title": "Board",
  "compose": "x-losos:\n  port: 8080\nservices: ...",
  "files": { "site/nginx.conf": "...", "site/index.html": "..." }
}
```

`release` benennt die App und ihren Datenordner: ein Kleinbuchstabe, dann
bis zu 39 Kleinbuchstaben, Ziffern oder Bindestriche. Die Dateien sind
Text, höchstens 500 und zusammen 4 MiB. Die Antwort ist der Eintrag der App
mit `phase: "installing"`; `GET /api/apps` zeigt sie danach als `running`
oder `failed`, mit dem Grund. Derselbe Name noch einmal ersetzt die App und
behält ihre Daten und ihren Port.

Die App bekommt einen Port aus `losos.apps.ports`, nur aus dem lokalen Netz
erreichbar. **Entfernen** im Bereich **Apps** oder `POST /api/apps/remove`
hält sie an und schließt den Port. Der Datenordner bleibt.

## Die eigenen Apps der Box

Ein Modul bringt eine der Apps der Box so mit:

```nix
losos.apps.builtin.board = {
  compose = {
    "x-losos" = {
      port = 8091;
      images = [ boardImage ]; # ein Image aus pkgs.dockerTools
    };
    services.web = {
      image = "localhost/board:1";
      volumes = [ "data:/var/lib/board" ];
    };
    volumes.data = { };
  };
  files = ./board-files; # oder null
  path = "/board/";
};
```

Bei jedem Start formt `losos-builtin-board.service` sie wie eine
installierte App und startet sie. Mit `path` liefert die Eingangstür sie
dort aus, ohne das Präfix und nur ins lokale Netz, außer mit
`lanOnly = false`; `locationConfig` fügt dieser Location nginx-Zeilen hinzu. Ihren Namen kann keine installierte App bekommen, und
der Bereich **Apps** kann sie nicht entfernen.

:::caution[Wer ruft an]
Eine App im Netz des Hosts lauscht auf einem Port, den jeder Prozess auf
der Box erreicht, auch die anderen Apps. Sie darf einen Header, den nginx
setzt, etwa wer angemeldet ist, nicht als Beweis nehmen. Ein Dienst, der
das braucht, läuft auf einem Unix-Socket, den nur nginx öffnen kann,
außerhalb dieses Formats.
:::

## Wo es liegt

| Was | Wo |
|---|---|
| Der Eintrag der App und die Dateien, wie sie kamen | `/var/lib/losos/apps/<app>/` (nur root) |
| Das App-Repository: Compose-Datei und Dateien jeder App, ein Commit pro Installation, Aktualisierung und Entfernung | `/var/lib/losos-podman/notshared/apps/<app>/` (ein eigenes Git-Repository von `notshared`) |
| Das Projekt, das Podman Compose ausführt | `/var/lib/losos-podman/notshared/projects/<app>/compose.json` |
| Der Image-Speicher | `/var/lib/losos-podman/notshared/storage` |
| Die Daten der App | `/home/notshared/data/apps/<app>/` |
| Die Unit, die sie ausführt | `losos-podman-app@<app>.service` (`up` beim Start, `down` beim Anhalten) |
| Das Protokoll der letzten Installation | `/var/lib/losos/apps/<app>/job.log` |

Die Regeln stehen in `backend/src/compose.rs`, die Units in
`modules/apps.nix`, und `nix/tests/podman-apps.nix` startet eine Box ohne
Cluster und führt darauf eine App jeder Art aus.

Das App-Repository ist von der Konfiguration der Box in `/etc/nixos`
getrennt und gehört `notshared`, damit die Geschichte dessen, was der
Besitzer ausgeführt hat, auf seiner Seite der Box bleibt. Ein Chart liegt
dort als die Compose-Datei, die die Box daraus gemacht hat. Die eigenen Apps
der Box sind nicht darin: Sie gehören zur Konfiguration. Die Dateien einer
App können ihre Passwörter enthalten, deshalb kann niemand außer
`notshared` (und root) das Repository lesen.
