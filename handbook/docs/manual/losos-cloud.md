---
title: LosOS Cloud
sidebar_position: 4.4
slug: /manual/losos-cloud
---

# LosOS Cloud

LosOS Cloud is the box's own file service: your files in a browser, a
network folder for the file manager, and a server the Nextcloud desktop and
phone apps sync with. It runs on the box beside the Nextcloud service at
`/nextcloud/`, which it is taking over app by app; for now the two keep
separate files.

Open it at `https://<name>.local/cloud/`. `<name>` is the box's name, as on
the box's screen. You sign in with LosOS Login, and everyone who can sign in
gets their own files. The person who set up the box gets the owner's.

## In the browser

- **Files** lists your folders. Drop files or whole folders on the page to
  upload them, or use **Upload**. Tick files to download, rename, move,
  copy or delete them, or to see their earlier versions.
- **Recent** shows what changed lately, **Favorites** what you starred.
- **Photos** shows every picture, newest month first. Click one to see it
  large and use the arrow keys to go through them.
- **Trash** keeps deleted files for 30 days. Restore one, or delete it for
  good.
- **Devices** lists the computers and phones that sync, and signs one out.
- The search field at the top finds files by name.
- Right-click a file, a folder or the space around them for what you can
  do there; see [the right-click menu](admin-pages#the-right-click-menu).

## Desktop and phone apps

The Nextcloud desktop client and the Nextcloud apps for Android and iOS
work with LosOS Cloud. Give them the box's address without a path:

```text
https://<name>.local
```

The app opens LosOS Login in a browser. Sign in, press **Allow** on the
page that follows, and the app gets a password of its own. It then shows up
under **Devices**, where you can sign it out without touching your LosOS
Login password.

## In the file manager

The same files are a WebDAV share at this address:

```text
https://<name>.local/remote.php/webdav/
```

On the box's network, GNOME Files and Dolphin list it under the network as
**LosOS Cloud on `<name>`**. Windows and macOS add it by address, as in
[Files on your computer](files-on-your-computer), with this address in
place of LosOS Cloud's. Install the box's certificate first, as in
[first run, step 1](../start/first-run#step-1-trust-this-box).

On the box's network the share takes your LosOS Login name and password.
Through the internet it takes only an app's own password, so use the
desktop or phone app there.

## Settings

`losos.cloud.enable` turns LosOS Cloud off, and `losos.cloud.trashDays`
sets how long the trash keeps files (1 to 365 days). Both are on the
**Advanced** pane.
