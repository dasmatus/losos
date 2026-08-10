# losos Nextcloud plugin + Haskell/PHP dev environment — design

Date: 2026-08-10
Status: implemented (PHP app + Haskell backend + JSON schema + NixOS wiring
all written and verified 2026-08-10)

> **Update 2026-08-10:** the original scope put the Haskell backend out of
> scope ("the user writes it"). That was reversed mid-implementation: the
> backend (`losos-ctl`) is now written in Haskell using a type-class effect
> abstraction, a formal JSON schema exists, and both the PHP and Haskell test
> suites pass. This document has been updated to describe what was actually
> built; the sections marked "design" below are unchanged in intent, the
> sections marked "implementation" describe the realised code.

## Goal

Set up a Haskell + PHP development environment (the host's fish shell config
**minus Zellij**) for developing a Nextcloud plugin, and write **both sides**
of that plugin (PHP frontend + Haskell `losos-ctl` backend, plus a formal JSON
schema). The plugin configures the machine itself: it toggles this box between
"Local only" (private Nextcloud) and "On the mesh" (contributing storage to the
Tahoe-LAFS grid) and triggers a NixOS rebuild, with rebuild progress shown as a
Nextcloud notification. Also enable a curated default set of Nextcloud apps from
nixpkgs and keep the Nextcloud App Store open so more can be installed on
demand.

## Context

- losos is a stateless NixOS appliance: tmpfs root, encrypted `/persist`,
  impermanence, **no SSH**, no login shells for the data accounts. The only
  privileged entry point is the Nextcloud web UI (admin = `notshared`).
- Existing `losos.sharingMyStorage` option (bool) is the single source of truth
  for whether this box contributes storage to the Tahoe grid. The plugin's
  toggle maps 1:1 onto it: `local` ⟺ `false`, `mesh` ⟺ `true`.
- The user's `dots` repo (`~/Dokumente/codeberg/personal/dots/flake/devshell.nix`)
  already implements the "host fish config minus Zellij" dev-shell pattern.
  This spec mirrors it for losos.
- nixpkgs (pinned) provides: GHC 9.10.3, cabal 3.16, HLS 2.13, PHP 8.3.32,
  Composer 2.10.2, Nextcloud 34. The NixOS nextcloud module has three app stores
  (`apps`, `nix-apps` = `extraApps`, `store-apps` = writable App Store path);
  setting `extraApps` disables the App Store *unless* `appstoreEnable = true`.

## Backend contract (Haskell, implemented)

One binary `losos-ctl`, invoked by PHP via `sudo -n`. JSON on stdout (one JSON
object per line). The contract is formalised in `backend/schema.json` (JSON
Schema draft 2020-12, with `$defs` for `mode`, `rebuildState`, `rebuild`,
`state`, `stateResponse`, `changeResponse`, `statusResponse`, `stateFile`, and a
`commands` object documenting the four subcommands).

| Subcommand | Effect | stdout JSON |
|---|---|---|
| `state --json` | read current config | `{"mode":"local\|mesh","sharing":bool}` |
| `change --mode <local\|mesh>` | apply config + trigger rebuild (async) | `{"job":"<id>"}` |
| `status --json` | rebuild progress | `{"state":"idle\|building\|done\|failed","progress":0-100,"message":"…"}` |
| `rebuild-done <exitcode>` (internal) | record terminal rebuild status | (none) |

`change` returns immediately with a job id; the rebuild runs detached. This is
the "schema: `change rebuild`" — the `change` command *is* the rebuild trigger.
`rebuild-done` is not part of the public contract; it is invoked by the detached
rebuild shell itself once `nixos-rebuild` finishes, to record success/failure.

### Backend design — type-class effect abstraction

`backend/src/Lib.hs` defines a `Losos m` type class of the effects a command
needs (`loadState`, `saveState`, `rewriteConfig`, `spawnRebuild`,
`rebuildLogTail`, `nextJobId`, `emit`). Every command (`cmdState`, `cmdChange`,
`cmdStatus`, `cmdRebuildDone`) is polymorphic in `m`. Two interpreters:

- **`instance Losos IO`** — the production backend. State lives at
  `$LOSOS_STATE_DIR/state.json` (default `/var/lib/losos`). `rewriteConfig`
  does a line-based replace of the `losos.sharingMyStorage = <bool>;` line in
  `$LOSOS_CONFIG` (default `/etc/nixos/defaults.nix`), injecting one before the
  final `}` if absent. `spawnRebuild` runs `setsid -f sh -c 'nixos-rebuild switch
  --flake <flake> > <log> 2>&1; ec=$?; <ctl> rebuild-done $ec'` detached
  (`create_group`, all std streams `NoStream`) so it outlives the sudo/PHP
  process tree. `rebuildLogTail` returns the last non-empty line of the log
  (capped at 240 chars) for failure messages. `nextJobId` is
  `<YYYYMMDDHHMMSS>-<pid>`.
- **`TestM`** — a `newtype` over `StateT TestState Identity` (`deriving newtype`
  for `Functor`/`Applicative`/`Monad`/`MonadState`) with pure stubs. The test
  suite (`backend/test/Spec.hs`) runs the *same* `cmdChange`/`cmdStatus`/
  `cmdRebuildDone` against `TestM`, so the state-machine logic is exercised
  deterministically with no filesystem and no `nixos-rebuild`.

This is the payoff of the type class: the commands are written once and tested
without IO. `runTest :: TestState -> (forall m. Losos m => m a) -> TestState`
runs a command in the pure interpreter.

NixOS wiring (implemented): option `losos.backend.package` (nullOr package,
default `null`) + `security.sudo.extraRules` NOPASSWD grant for user `nextcloud`
→ root, commands pinned to exactly the `losos-ctl` binary path, active only when
the package is set. When the package is null, `BackendService` reports "backend
not installed" instead of failing. `modules/defaults.nix` sets
`losos.backend.package = self.packages.x86_64-linux.losos-ctl` so the toggle
works out of the box.

## PHP app (`nextcloud-app/`)

```
nextcloud-app/
  appinfo/info.xml          metadata + admin-settings section
  appinfo/routes.php         AJAX routes
  appinfo/app.php            bootstrap: register admin section
  lib/AppInfo/Application.php  IBootstrap: wire BackendService + SettingsController
  lib/Controller/SettingsController.php  getState/setMode/rebuildStatus (admin-only)
  lib/Service/BackendService.php          shells out to `sudo -n losos-ctl …`
  lib/Service/CommandRunner.php           injectable executor interface
  lib/Service/ProcCommandRunner.php      proc_open default executor
  lib/Service/BackendException.php       + BackendNotInstalledException
  lib/Settings/Section.php + AdminSettings.php
  templates/admin.php        toggle UI
  css/style.css
  js/settings.js             toggle + poll status + OC.Notification toast
  composer.json              phpunit / php-cs-fixer scripts
  tests/BackendServiceTest.php + tests/stubs/ocp/IConfig.php
```

- `BackendService` builds argv as an array (each element shell-escaped by
  `ProcCommandRunner` via `escapeshellarg`) and runs
  `sudo -n /run/current-system/sw/bin/losos-ctl <sub> --json`, parses JSON,
  throws `BackendException` on non-zero / malformed output, and
  `BackendNotInstalledException` when the binary is missing. The executor is
  the `CommandRunner` interface, injectable so the PHPUnit suite stubs the
  subprocess (`StubRunner`) without touching sudo.
- `SettingsController` checks admin membership (`IGroupManager->isAdmin`) on
  every endpoint; POST routes are CSRF-protected by the AppFramework.
- `js/settings.js`: toggle → POST `setMode` → poll `rebuildStatus` every 2s →
  update a single toast via `OC.Notification.showHtml` (`Rebuilding… 42%`),
  final `Rebuild complete` / `Rebuild failed`.

## devShell (`flake/devshell.nix` + `flake.nix` output)

`mkShell` whose `shellHook` `exec`s `fish -i -N -C "source <fishInit>"` only
when interactive (the `case $- in *i*` guard keeps `nix develop -c <cmd>`
working under bash). `fishInit` sets `fish_greeting`, `fastfetch`, `cat`/`ls`/
`cd` aliases, zoxide + starship init — **no Zellij, no claude/codex aliases**.

`nativeBuildInputs`:
- Haskell: `ghc`, `cabal-install`, `haskell-language-server`
- PHP: `php83` + extensions (intl, mbstring, gd, zip, pdo_pgsql, gmp, bcmath,
  redis, opcache), `php83Packages.composer`, `php83Packages.phpunit`,
  `php83Packages.php-cs-fixer`
- Nextcloud: `nextcloud34` (provides `occ`)
- Shell UX: `fish`, `bat`, `eza`, `zoxide`, `fastfetch`, `starship`

No local Nextcloud is spun up (per decision: toolchains + occ + composer only).

## Enabling apps (`modules/services.nix`)

```nix
services.nextcloud = {
  extraAppsEnable = true;
  extraApps = { … curated self-contained set …; losos = self.packages.x86_64-linux.losos-app; };
  appstoreEnable = true;  # keep App Store open for heavy/external-server apps
};
```

Curated set (self-contained, broadly useful): deck, tasks, notes, bookmarks,
calendar, contacts, maps, polls, forms, tables, collectives, news, mail, music,
memories, groupfolders, files_automatedtagging, files_linkeditor,
files_retention, previewgenerator, checksum, notify_push, dav_push,
twofactor_webauthn, twofactor_admin, guests, impersonate, unroundedcorners.

Left to the App Store (external server / heavy / needs IdP or API key):
onlyoffice, richdocuments, spreed, recognize, integration_openai,
integration_deepl, integration_paperless, user_saml, user_oidc, sociallogin,
registration, end_to_end_encryption, whiteboard, uppush, nextpod, phonetrack,
gpoddersync, qownnotesapi, quota_warning, hmr_enabler, extend, repod, oidc.

## Plugin as a Nix derivation

`flake.nix` gains `packages.x86_64-linux.losos-app`: a trivial derivation that
drops `nextcloud-app/` into a store path with the right layout, so
`extraApps.losos` symlinks it in read-only. No `fetchNextcloudApp` (local source).

## Verification (all passing 2026-08-10)

- **PHP**: `composer test` → 8 tests / 11 assertions OK (BackendService:
  getState/setMode mesh/invalid/rebuildStatus/non-zero-exit/non-JSON/missing-
  backend/isInstalled). `composer lint` (php-cs-fixer `@PSR12`) → 0 violations.
- **Haskell**: `nix build .#losos-ctl` (callCabal2nix, `doCheck`) → builds, test
  suite (tasty, `TestM` interpreter) passes. End-to-end against a scratch
  `LOSOS_STATE_DIR` + bogus flake: `change --mode mesh` returns a job id,
  rewrites the config line, writes `state.json` with `state: building`, the
  detached rebuild fails fast (non-root, bogus flake) and `rebuild-done` flips
  status to `failed` with the real error captured from the log tail; `state
  --json` returns `{"mode":"mesh","sharing":true}`.
- **NixOS**: the `install` system's toplevel evaluates with the new `extraApps`
  (29 curated apps + `losos`) + `appstoreEnable` + `extraAppsEnable` + the
  sudoers rule + the backend package wired in.
- **devShell**: `nix develop .#` opens the fish dev shell with the full Haskell
  stdlib (`ghcWithPackages`), PHP 8.3 + extensions, composer, nextcloud34 (`occ`),
  and the host fish config minus Zellij.