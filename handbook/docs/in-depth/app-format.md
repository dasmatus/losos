---
title: App format
sidebar_position: 4.8
mdx:
  format: md
---

# App format

![The Apps pane.](../img/apps.png)

An app for a LosOS box is one Compose file and the files beside it. The box
runs it as `notshared` on rootless Podman, with Podman Compose, and needs no
cluster for it. The same format serves two sources:

- apps an owner, or a program working for them, sends to the box,
- the box's own apps, declared in Nix with `losos.apps.builtin`.

A chart from the **Apps** pane's search, installed as `notshared`, ends up
in the same place: the box renders it with `helm template` and turns its
pods into a Compose project of this shape.

## The file

```yaml
x-losos:
  port: 8080            # the port the app's page listens on
services:
  web:
    image: docker.io/library/nginx:1.27-alpine
    command: ["nginx", "-g", "daemon off;", "-c", "/site/nginx.conf"]
    volumes:
      - ./site:/site:ro       # a folder the app ships
      - data:/var/lib/board   # the app's data
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

`x-losos` is the box's own block:

| Key | Meaning |
|---|---|
| `port` | The port the app's page listens on. The **Open** link and the LAN forward go there. Without it the box picks a port named in `ports` or `expose`, preferring a service called `web` or `ui`. |
| `images` | Image archives in the Nix store, loaded before the app starts. For apps built with Nix; an app from elsewhere pulls its images from a registry. |

## What the box does to it

Every service runs:

- on the box's own network (`network_mode: host`), so `ports` and `expose`
  only tell the box where the app listens;
- as uid 1000 inside the container and as `notshared` outside it
  (`userns_mode: keep-id`), so what the app writes belongs to `notshared`;
- with no capabilities and `no-new-privileges`;
- restarted unless it was stopped, unless the file says otherwise.

Services find each other by name: each service name points at `127.0.0.1`.
Because they share one network, they must listen on different ports, all
from 1024 up, and none of the ports the box keeps for itself (its web
server, its admin API on 8082, LosOS Git's 3000, PostgreSQL's 5432, Redis's
6379, the cluster's ports, the forward range 30000 to 30099).

Volumes:

| In the file | On the box |
|---|---|
| `name:/path`, with `name` under `volumes:` | `/home/notshared/data/apps/<app>/<name>`, kept when the app is removed |
| `./path:/path` | a file or folder the app ships, from its project folder |
| `/path` alone | an anonymous volume in the image store, gone with the container |
| `type: tmpfs` | memory |

`secrets:` and `configs:` with a `file:` the app ships are mounted read-only
at `/run/secrets/<name>` and `/<name>`. `env_file` must name a file the app
ships.

The box refuses, with a sentence naming the service and the key:
`privileged`, `cap_add`, `devices`, `pid` or `ipc`, any `network_mode` but
`host`, `build` (apps are not built on the box), `extends`,
`volumes_from`, `sysctls`, a path outside the app (`/etc`, `../`), a named
volume with a driver or one marked `external`, a port below 1024 or one the
box keeps, and a port another app uses.

## Installing one

`POST /api/apps/compose` takes the file and the files beside it:

```json
{
  "release": "board",
  "title": "Board",
  "compose": "x-losos:\n  port: 8080\nservices: ...",
  "files": { "site/nginx.conf": "...", "site/index.html": "..." }
}
```

`release` names the app and its data folder: a lower-case letter, then up to
39 lower-case letters, digits or dashes. The files are text, at most 500 of
them and 4 MiB together. The answer is the app's record with
`phase: "installing"`; `GET /api/apps` shows it `running` or `failed`, with
the reason. Sending the same name again replaces the app and keeps its data
and its port.

The app gets a port from `losos.apps.ports`, reachable from the local
network only. **Remove** on the **Apps** pane, or `POST /api/apps/remove`,
stops it and closes the port. The data folder stays.

## The box's own apps

A module ships one of the box's apps like this:

```nix
losos.apps.builtin.board = {
  compose = {
    "x-losos" = {
      port = 8091;
      images = [ boardImage ]; # a pkgs.dockerTools image
    };
    services.web = {
      image = "localhost/board:1";
      volumes = [ "data:/var/lib/board" ];
    };
    volumes.data = { };
  };
  files = ./board-files; # or null
  path = "/board/";
};
```

At every boot `losos-builtin-board.service` shapes it like an installed app
and starts it. With `path`, the front door serves it there, with the prefix
stripped, to the LAN only unless `lanOnly = false`; `locationConfig` adds
nginx lines to that location. Its name
cannot be taken by an installed app, and the **Apps** pane cannot remove it.

:::caution[Who is calling]
An app on the host network listens on a port every process on the box can
reach, the other apps included. It must not take a header nginx sets, such
as who is signed in, as proof. A service that needs that runs on a Unix
socket only nginx can open, outside this format.
:::

## Where it lives

| What | Where |
|---|---|
| The app's record and the files as sent | `/var/lib/losos/apps/<app>/` (root only) |
| The apps repo: each app's Compose file and files, one commit per install, update and removal | `/var/lib/losos-podman/notshared/apps/<app>/` (a Git repository of `notshared`'s own) |
| The project Podman Compose runs | `/var/lib/losos-podman/notshared/projects/<app>/compose.json` |
| The image store | `/var/lib/losos-podman/notshared/storage` |
| The app's data | `/home/notshared/data/apps/<app>/` |
| The unit that runs it | `losos-podman-app@<app>.service` (`up` on start, `down` on stop) |
| The job log of the last install | `/var/lib/losos/apps/<app>/job.log` |

The rules are `backend/src/compose.rs`; the units are `modules/apps.nix`;
`nix/tests/podman-apps.nix` boots a box with no cluster and runs one app of
each kind on it.

The apps repo is apart from the box's configuration in `/etc/nixos`, and
`notshared` owns it, so the history of what the owner ran stays on their own
side of the box. A chart is kept there as the Compose file the box made of
it. The box's own apps are not in it: they are part of the configuration.
An app's files can carry its passwords, so nobody but `notshared` (and
root) can read the repo.
