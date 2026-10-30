---
title: Administration
sidebar_position: 2
mdx:
  format: md
---

# Administration

## Admin UI

Open `http://<ip>/` (the address on the box's tty1 banner) or
`https://<host>.local/` from the LAN. The admin UI refuses anything
outside the LAN, including traffic through the [master proxy](master-proxy.md).
`<host>.local` is an mDNS name. A computer that does not resolve it, typically
the host of a NAT'ed VM, uses the IP address, and every route below answers
on it the same way.

Services on the same host:

| Service          | URL                       |
| ---------------- | ------------------------- |
| Admin UI         | `<host>.local/`           |
| LosOS Cloud      | `<host>.local/cloud/`     |
| LosOS Git        | `<host>.local/git/`       |
| Handbook         | `<host>.local/handbook/`  |
| [Diagrams](diagrams.md) | `<host>.local/diagrams/` |

The box's IP address (the one on the tty1 banner) and its bare name work in
place of `<host>.local` everywhere, LosOS Cloud included. That is the way
in from a libvirt VM, which gets no mDNS name. LosOS Cloud keeps no list of
trusted names, so it answers on whichever address a request arrived on.

`losos.tls.enable` (on by default) adds HTTPS on :443 with a certificate the
box generates itself, valid for two years. Plain HTTP on :80 stays open.
Browsers do not trust the certificate until you install it. The first-run
wizard offers a one-line installer for macOS, Linux and Windows
(`/setup/trust.sh`, `/setup/trust.ps1`, both LAN-only) and the plain download
(`/setup/losos-ca.crt`). See [Install in depth](install.md).

## Look

**Settings, Look** changes how the admin pages look, for every browser that
opens the box. Nothing here is a setting in the Nix sense. It is a document
lososd keeps at `/var/lib/losos/look.json`, so a change takes effect the
moment it is saved, with no rebuild.

### Background

Pick one of the three shipped pictures, or upload one of your own (PNG, JPEG,
WebP, GIF or SVG, up to 8 MiB). The **Veil** slider lays the page colour over
the picture, from 20 % to 90 %, so text stays readable in both the light and
the dark theme. *Plain* removes the picture again.

:::note[Why the picture needs no token]
lososd serves an uploaded picture at `/api/look/background` without a token,
because a CSS `background-image` cannot send one. The route sits behind the
same LAN-only guard as the rest of the admin pages, and a wallpaper is not a
secret.
:::

### Widgets written by hand

The Overview board's gallery (**Add a widget**) has two ways to make your
own. *Build one* composes a widget from the box's readings without any code.
*Write one* takes HTML, style and script of your own, kept on the box and
listed on this pane, where you can edit and delete it.

A widget is a few files. `index.html` is where it starts; it links the rest
by name, as in `<link rel="stylesheet" href="style.css">`,
`<script src="app.js"></script>` or `<img src="dot.svg">`, and
`fetch("data.json")` reads a file of its own. A new widget starts as
`index.html`, `style.css` and `app.js`; **Add a file** adds another (`.html`,
`.css`, `.js`, `.json`, `.svg`, `.txt` or `.md`). Scripts are classic
scripts: load several with several `<script src>` tags, in order, rather
than `import` between them.

The editor completes as you type: the `losos` object and its members, the
names of the readings inside `losos.metric("`, the colour variables after
`var(--`, and the widget's own file names wherever a file is named. Press
Ctrl+Space to ask for suggestions anywhere.

A hand-written widget runs inside a sandboxed frame, an origin of its own
with no access to the admin pages, the admin token or the API. It talks to
the box through a small `losos` object the frame provides:

| Call                           | What it gives                                                        |
| ------------------------------ | -------------------------------------------------------------------- |
| `losos.metric(name)`           | A promise of one of the box's readings, the same names the gallery's built-in widgets use (`storage.bytes`, `box.settings`, …). |
| `losos.theme`, `losos.lang`    | `light`/`dark` and the language the admin page is in.                |
| `losos.palette`                | The box's colours, also set as CSS variables (`var(--accent)` works). |
| `losos.onTheme(fn)`            | Called whenever the owner switches the theme.                         |
| `losos.resize()`               | Ask the board to re-measure the tile after a change it cannot see.    |
| `losos.files`                  | The names of the widget's files.                                      |
| `losos.file(name)`             | One of the widget's files, as text.                                   |
| `losos.asset(name)`            | One of the widget's files as a `data:` URL, for an image set from script. |

The editor's **How it works** tab repeats this with an example. Its preview is the
real frame, so what it shows is what the tile will show. A widget can fetch
the internet (a weather tile, say) but not the box. The limit is 24 widgets,
each at most 12 files and 128 KiB together.

The editor also has a **Build with AI** tab, where a language model on the
box writes the widget from a description and puts its files in the editor for
you to read before saving. It runs on the box, so what you type stays there
and a build costs nothing; see [Build with AI](widget-builder.md).

## Admin token

The box makes a random 64-hex-character token on first boot and keeps it
sealed in its credential store, as `admin-token` (see
[Credentials](../reference/credentials.md)); `lososd` reads it from its own
credential directory. The token can change any setting, so it is root.

On a new, unclaimed box, the first LAN caller to submit `POST /api/setup/claim`
sets the owner password, becomes the first person who can sign in (an
administrator) and receives the token in the response. This route
requires no token, but stops accepting claims once the box has an owner. Set
up the box only on a trusted LAN, because the first caller becomes its owner.
The box hands the token back only once, and the UI cannot fetch it again.

Nothing has to start before the first password, so `GET /api/setup/claim`
answers `ready: true` on an unclaimed box and the first-run wizard moves on
at once. A request that needs a service still on its first start (LosOS Git,
say) answers `503` with `{"error": …, "ready": false, "waitingFor": …}` and
a `Retry-After`, and changes nothing.

The first-run wizard is that caller. After it, people sign in on LosOS
Login, the box's sign-in page at `https://<box>/auth/`, each with a sign-in
name and password of their own. LosOS Login is an open-source sign-in server
that runs on the box. lososd keeps the people file it reads, with each
password as an argon2id hash, and LosOS Login picks up a change to it at
once. nginx asks LosOS Login about every `/api/` request and passes the
person's name on to lososd, which opens the sections their groups allow. The
first person is always an administrator. A person changes their own password
with `POST /api/set-password`, and for the owner that changes the LosOS
Cloud password too. An administrator adds, changes and removes everyone
else on the Users pane, through `GET` and `POST /api/people` and
`POST /api/people/{access,password,remove}`. The pane is there only while
the box signs people in itself (`standalone` in `GET /api/me`). lososd
refuses to touch the owner, the caller's own account or the last
administrator. LosOS Login looks each person up again on every request, so
a removal or a changed group reaches the apps at the next click too.
Sessions live in a Redis of LosOS Login's own, on a socket only it can
open, so a restart of LosOS Login or the nightly reboot signs nobody out.

LosOS Cloud and LosOS Git sign people in the same way. nginx asks LosOS
Login and sends the person on in three headers, `X-Losos-Cloud-User`,
`-Name` and `-Email`, which it sets on every request to either app so no
client can send its own. The owner, in the `losos-owner` group, arrives as
the `notshared` account in both; everyone else under their sign-in name.
LosOS Cloud (`backend/src/cloud/auth.rs`) and LosOS Git
(`backend/src/forge/auth.rs`) read the headers themselves, and each makes
an account the first time; lososd then keeps the name and address in both
apps in step with the people file. A request to LosOS Git that brings its
own token in `Authorization` (git, the API) is not asked about. A person
LosOS Login does not know is sent to it,
from the LAN and through the edge alike; neither app shows its own sign-in
page. Through the edge, the tunnel delivers from loopback with
`X-Forwarded-Proto: https`, which nginx believes from loopback only, and
lososd adds each public name the box answers on to LosOS Login's cookie
domains. While LosOS Login starts, the apps answer 503 with a page that
retries by itself. Both apps read the headers only on their Unix sockets,
`/run/losos-cloud/http.sock` and `/run/losos-git/http.sock`, which only
root and nginx may open; LosOS Git's loopback port, `127.0.0.1:3000`, takes
tokens and never the headers. So nothing on the box can name a person to
either app. The names `notshared`, `shared` and `losos` are kept for the
box's own accounts.

Each person with the `losos-account` group (Own account on the Users pane)
keeps their own name, email address and password through `GET` and
`POST /api/account`. LosOS Login keeps their authenticator app and passkeys
in its own database; adding or removing one needs a one-time code it would
normally mail, so its notifier writes the code to a file and
`POST /api/account/code` hands it back behind the person's password. An
administrator sets two rules with `POST /api/people/rules`: two-factor
sign-in for everyone, and passwords that run out after a number of days.
lososd marks the people file with two groups: `losos-two-factor` for
everyone with a second factor, which LosOS Login asks for on every sign-in,
and `losos-locked` for anyone a rule catches. LosOS Login denies a locked
person LosOS Cloud and LosOS Git, and lososd answers them only on their
account (`locked` in `GET /api/me`). lososd refuses the two-factor rule
while the administrator setting it has no second factor, and the spare key
is never locked. `POST /api/people/two-factor/reset` removes a person's
second factors.

The token itself is still shown once, after the password is set, as the
**spare admin key**, with Copy and Print buttons. A request that carries it
skips LosOS Login and lososd checks the key itself, so the key opens the
admin pages while LosOS Login is not running. The sign-in dialog has a link
to enter it instead. A box set up before people had sign-in names
asks for the key once, together with a sign-in name and a password, to make
its first administrator.

A new password has to be at least 12 characters with a lower-case letter, an
upper-case letter, a digit and a symbol such as `-` or `!`. The wizard shows
the four rules with a tick each as you type.

:::warning[Lost password and spare key]
There is no way to rotate the token from the box, since it has no shell.
Losing the password *and* the spare key means reinstalling from the install
medium, which wipes the disk.
:::

## Settings

Settings are Nix. The settings page generates `modules/overrides.nix` and
runs `nixos-rebuild switch`. So:

- A change takes a rebuild, not a restart.
- The box can be rebuilt from this repository plus that one file.
- `apply` replaces `overrides.nix` entirely. A setting the UI does not write
  is reset on the next save.

### Advanced

**Settings, Advanced** lists every `losos.*` option the box declares, read
from the modules it runs. `nix/options-doc.nix` generates the document at
build time, and lososd serves it at `GET /api/options` with the current
`overrides.nix` joined in. Each row shows the option's description, its
default, the value the box is running with, and an editor for its type: a
switch, a number with its bounds, a choice, a text field, or one item per
line for a list.

- An option one of the other panes already edits (the name, the mesh
  settings, the hardening switches) is shown with its value and a link to that
  pane, so there is one place to change it.
- The three the installer writes (`targetDrives`, `tpm.enable`, `bios`) and
  the packages the build chooses are read-only. A line for one of them in
  `overrides.nix` would fail the next rebuild.
- Options marked **Careful** (`admin.*`, `cluster.*`, `hostName`,
  `storage.*`, `tls.*`, `upgradeFlakeUri`, …) ask once before the first
  change. A wrong value leaves a box with no shell to fix it from.
- **Use default** removes the line rather than writing the default's value,
  so a default that moves with an update is followed.
- A line in `overrides.nix` the box has no option for (a typo, a setting
  from another version) is shown in its own group and blocks Apply until it
  is removed. lososd refuses such a body too (`backend/src/options.rs`,
  `check_body`). It checks every line of an apply against the document:
  declared, not read-only, a value of the right kind, no `${`.

Everything written here goes through the same Apply and the same rebuild as
the other panes. `losos.edge.*` is left out because the appliance never reads
it.

### History and LosOS Git

Every Apply, storage change and factory reset is one commit in `/etc/nixos`,
the repository `losos-ctl install` made. The commit is named after the
settings it changed, `Change hostName, cluster.enable`, with the before and
after of each in the body. **Settings, History** lists the last forty.

With `losos.configRepo.enable` (the default) and LosOS Git on, lososd also
keeps the configuration in a private repository on LosOS Git,
`<owner>/losos-config`, owned by the admin's own account (`notshared`).
lososd creates the account on the first sync, as LosOS Git's administrator. A
reconciler in lososd runs every 30 s:

- it commits anything uncommitted and pushes the box's branch when LosOS
  Git is behind;
- when LosOS Git is ahead, because you cloned the repository, edited
  `modules/overrides.nix` and pushed, it fast-forwards to it, checks the new
  `overrides.nix` line by line the way Apply is checked, and starts a
  rebuild. A refused push is reported under History and the box stays on
  its own commit;
- when the two have diverged (a rewritten history) it does nothing and says
  so. Sort it out from a clone;
- while a rebuild is running, the push waits for the next tick.

**Sync now** on the History pane runs one tick immediately. `losos-ctl
config` prints the same document, and `losos-ctl config --sync` runs a tick.
Secrets never enter the repository. `overrides.nix` carries option values
only, and the token lososd authenticates to LosOS Git with is the admin
token LosOS Git writes on its first start,
`/var/lib/losos-git/admin-token`, outside `/etc/nixos`. There is no CI
runner behind this. The box polls.

The clone address is shown on the pane (`http://<box>/git/<owner>/
losos-config.git`). The repository is private, so a computer needs a token
of its own (`losos git setup`, [The terminal client](../manual/terminal)).
lososd talks to LosOS Git on its loopback port with the admin token, acting
as the owner with `X-Losos-As` where a call is the owner's.

### Moving from Forgejo

A box that ran Forgejo carries its records into LosOS Git by itself, on the
first start after the update and before LosOS Git answers: repositories,
accounts, collaborators, access tokens (the old values keep working for
git and the API), labels, issues, pull requests, comments, assignees and
attachments. An organisation becomes an account of the same name.

The report is `/var/lib/losos-git/forgejo/import.json`. It counts what
Forgejo had and what came over, table by table, and names what did not,
such as milestones, releases and timeline entries. A full dump of
Forgejo's database is kept beside it (`forgejo.sql`, or `forgejo.db` on a
native box), and Forgejo's own folder and the Postgres data are left where
they were. If the move fails, nothing changes and the box tries again on the
next start; LosOS Git serves the repositories meanwhile.

### Box-wide app settings

The Apps pane carries the settings that hold for everyone on LosOS Cloud
and LosOS Git, the ones each app otherwise keeps on its own administration
pages. Each field is an option, `losos.cloud.trashDays` or one under
`losos.git.site.*`, so it goes through Apply, the gate, History and the
configuration repository like any other setting. The Advanced pane shows
their values and links to the app's fold on the Apps pane (`/apps#cloud`,
`/apps#git`) instead of drawing a second editor (`ON_PANE` in
`admin-ui/app/src/lib/option-value.ts`).

| Option | What it sets in the app |
| --- | --- |
| `losos.cloud.trashDays` | how many days a deleted file stays in the trash (1 to 365, default 30) |
| `losos.git.site.requireSignIn` | `losos-git serve --require-sign-in` |
| `losos.git.site.defaultPrivate` | `--public-by-default` when `"public"` |
| `losos.git.site.pushCreate` | `--push-create` |

`modules/cloud.nix` hands `trashDays` to the service as `--trash-days`, and
the service's hourly sweep deletes for good whatever has sat in someone's
trash longer than that.

LosOS Git's three are flags on its command line (`modules/git.nix`), so
there is no page inside it to change them from.

:::note[What stays in the apps]
Everything an account sets for itself stays in the app. In LosOS Git, the
owner's account is the administrator. LosOS Cloud has no administrator
inside it at all. LosOS Login decides who may use it, and everything
box-wide is a setting on the Apps pane.
:::

## Backup, restore and erase

**Settings, Backup** sends the box's data to an S3-compatible bucket the
owner rents (Amazon S3, Glacier included, Backblaze B2, Wasabi, Cloudflare
R2, MinIO).
[restic](https://restic.net) does the copying and encrypts on the box before
anything leaves it, so the bucket holds ciphertext. The repository password
is the box's recovery code (`/var/secrets/losos-recovery-code`), which the
pane shows on request; without it nobody can open a backup.

A backup holds:

- `/var/lib/losos-cloud` and `/var/lib/losos-git`, with the SQLite
  databases of LosOS Cloud and LosOS Git copied by SQLite's own backup
  first, so a write during the copy cannot tear them;
- the shared folder. When sharing is on it is an fscrypt policy, and Linux
  gives no ciphertext for a locked fscrypt file, so the script unlocks the
  policy for the copy and locks it again afterwards. restic's encryption
  covers it in the bucket;
- `modules/overrides.nix`, the home page's look, the proxy token and the
  people file LosOS Login reads.

On an `amazonaws.com` address the owner can pick a storage class: Glacier
Instant Retrieval, Glacier Flexible Retrieval or Glacier Deep Archive.
restic puts only the data packs in that class and keeps its own metadata in
S3 Standard, so checking a code and tidying stay instant. A restore from
Flexible Retrieval or Deep Archive turns on restic's `s3-restore` feature,
which asks AWS to thaw the packs and waits up to 48 hours; tidying there
never repacks (`--max-repack-size 0`), since a repack would need a thaw too.
A lifecycle rule that moves the bucket to Glacier would take restic's
metadata with it, so you choose the class on the pane instead.

The pane also offers each storage order the box bought on the
[market](/in-depth/market.md) once its volume is bound. Backups then go
through the edge that sold it, to restic's REST protocol on
`/market/storage/<order>/`, with the same encryption as a bucket. The edge
opens the volume only to a key the box makes from its recovery code and
the order id (`POST /api/backup/target/bought`), so neither the edge nor
the boxes that hold the volume's replicas can read a backup, and a new box
with the same code can restore from it. The box keeps no secret for this
target: the backup job makes the key again from the code each time.

The bucket keeps the last seven. lososd runs each backup and restore as a
transient unit, `losos-backup-<job>`, started by systemd outside lososd's
`ProtectHome=true` sandbox. The bucket's keys are sealed in the credential
store and reach the unit through `LoadCredentialEncrypted=`, never on a
command line or in a plain file.

**Restore** takes the recovery code of the box that made the backup. The
newest snapshot comes back. The apps stop, the box syncs the files back and
loads the databases (with `pg_restore --clean` on a box that still runs
PostgreSQL), the apps start, and the box checks the restored `overrides.nix`
like any apply and rebuilds. The code
that opened the backup becomes this box's recovery code. On an erased or
reinstalled box, run the wizard, set up the same bucket, then restore.

**Settings, Reset, Erase** deletes the data as well as the settings. lososd
drives it in phases kept in `state.json`, so it carries on with no tab open:

1. an optional backup. If it fails, the erase stops and nothing changes;
2. a countdown of `losos.reset.graceMinutes` (15 by default). Cancel stops
   it, and nothing on the box or outside it has changed yet;
3. leaving the edge: the box's custom domains are removed, a wallet bound to
   its names is unbound, its active market listings are closed, and it is
   taken off the registry (`POST /deregister`).
   Each step is tried even when the one before failed, and the report names
   the ones the edge did not confirm;
4. the default settings are committed and rebuilt, then lososd leaves a
   marker on `/persist` and reboots;
5. `losos-factory-wipe.service` runs early in the next boot, before
   `sysinit.target` and before any service that owns the data, and deletes
   the app data, the databases, both Kubernetes instances' state, the
   people file and LosOS Login's state, the secrets, the journal and the
   fscrypt metadata. It empties both data
   homes. The marker goes last, so an interrupted wipe runs again.

:::danger[Only the first two steps can be cancelled]
Cancelling is possible in steps 1 and 2 only. `/nix`, `/etc/nixos` and the
disk's keyfile stay, so LosOS is still installed and the box opens the
setup wizard. `/var/lib/losos-erase/report.json` keeps what the erase gave
up outside the box, as counts, and Settings, Reset shows it.
`nix/tests/erase.nix` runs the whole cycle against a MinIO bucket in one VM.
:::

## Users

Two data users, both without passwords or shells:

| User        | Owns                |
| ----------- | ------------------- |
| `notshared` | The owner's apps; their LosOS Cloud files carry this name |
| `shared`    | Mesh storage        |

Each has its own primary group and a mode-700 home, so neither can read the
other's files. `nix/tests/impermanence.nix` checks this.

## Upgrades and reboots

- **03:00.** lososd first moves `/etc/nixos` to the newest commit of
  `losos.configRepo.upstream.branch` (`main`) at
  `losos.configRepo.upstream.url` (the GitHub repository), keeping the box's
  `modules/overrides.nix` and `modules/install-target.nix`. Then
  `system.autoUpgrade` rebuilds from `losos.upgradeFlakeUri`, which is
  `git+file:///etc/nixos#install`. Keep the `#install` fragment; without it
  the rebuild fails. When another file in `/etc/nixos` changed since the last
  update, the box keeps it and does not update; the History pane says so.
  `losos-ctl config --update` runs the first step by hand.
- **00:07.** `midnight-reboot.timer` reboots unconditionally
  (`Persistent=true`, so a box that was off catches up). Because the root is
  rebuilt on boot, the reboot is how the box repairs itself.
