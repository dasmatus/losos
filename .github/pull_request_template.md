<!--
Keep every heading. Write for someone who has not read the diff: the
description alone should say what changed and how it was checked. Delete
these comments as you fill the sections in.
-->

Before: what a reader saw, or what the box did, before this change.

After: what they see, or what it does, now.

One sentence on what the change does, when the two paragraphs above do not
already make it obvious.

## Screenshots

<!--
Required for every change a person can see: the admin UI and the wizard,
the installer and the tty1 banner, the Nextcloud and Forgejo themes, the
wiki and docs pages, the Vercel demo pages. Take both at the same window
size and in the same state, so that the only difference between the two
columns is the change. Drag the images into this box, or name the folder
they are in (sessions keep theirs in the project folder under
demo/<topic>/). One row per screen that changed; add light/dark or
phone-width rows when the change touches them.

If nothing a person can see changes, replace the table with the line
"No visible change." followed by one line saying why (backend only, CI
only, a test, a refactor with the same output).
-->

| Before | After |
| ------ | ----- |
|        |       |

## How

<!-- The mechanism, briefly: which modules or files carry it and why there. -->

## Tested

<!--
What ran and what it showed, then what did not run and why. The VM tests
need KVM, so a session usually cannot run them; say so rather than leaving
the line out. A bare `nix build` is not a test gate (see CLAUDE.md).
-->

- `devenv test` (pins, lint, `cargo test`, flake eval):
- `admin-ui/app`: `npm run typecheck`, `npm run test:browser`:
- VM tests (`nix build .#checks.x86_64-linux.<name>`, KVM only):
- Install toplevel (`nix build .#nixosConfigurations.install.config.system.build.toplevel`):
- Screenshots taken in (browser, VM, or ISO under QEMU):

## Notes for the reviewer

<!-- Where to look first, what to try, what was left out on purpose, follow-ups. -->
