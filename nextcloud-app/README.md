# losos — Nextcloud app

Admin settings app for the **losos** appliance. It flips this box between
**Local only** (private Nextcloud; `losos.sharingMyStorage = false`) and
**On the mesh** (contributing storage to the Tahoe-LAFS grid;
`losos.sharingMyStorage = true`), then triggers an automatic NixOS rebuild.
Rebuild progress is shown as a Nextcloud notification.

The PHP side only talks to the **Haskell `losos-ctl` backend** (invoked through
a locked-down sudoers rule). The backend owns the actual config rewrite +
`nixos-rebuild`. See `docs/superpowers/specs/` in the repo root for the design.

## Backend contract

`losos-ctl` is called as `sudo -n /run/current-system/sw/bin/losos-ctl …`:

| Subcommand | stdout JSON |
|---|---|
| `state --json` | `{"mode":"local\|mesh","sharing":bool}` |
| `change --mode <local\|mesh>` | `{"job":"<id>"}` (async rebuild starts) |
| `status --json` | `{"state":"idle\|building\|done\|failed","progress":0-100,"message":"…"}` |

## Layout

```
appinfo/   info.xml, routes.php, app.php
lib/       AppInfo/Application.php, Controller/SettingsController.php,
           Service/BackendService.php, Settings/{Section,AdminSettings}.php
templates/ admin.php
css/ js/ img/   UI
tests/     PHPUnit suite for BackendService (stubs OCP\IConfig)
```

## Develop

From the repo root:

```sh
nix develop .#          # fish + Haskell + PHP toolchains (host config, no Zellij)
cd nextcloud-app
composer install        # phpunit + php-cs-fixer
composer test           # run the BackendService unit suite
composer lint           # php-cs-fixer dry-run
```

Point `backend_path` (app config `losos`) at a real `losos-ctl` to exercise the
subprocess path; the unit tests stub it out.