---
title: Files on your computer
sidebar_position: 4.5
slug: /manual/files-on-your-computer
---

# Files on your computer

Your LosOS Cloud files are also a network folder. Any computer on the box's
network can open it in its file manager, drag files in and out, and open
them in place, without installing anything. The folder is LosOS Cloud's
WebDAV share, at this address:

```text
https://<name>.local/nextcloud/remote.php/webdav/
```

`<name>` is the box's name, as on the box's screen. The share holds the same
files as Files in LosOS Cloud.

## Before you start

Install the box's certificate on the computer, as in
[first run, step 1](../start/first-run#step-1-trust-this-box). Without it,
the file manager says it cannot verify the box and asks whether to go on,
and Windows refuses to connect at all.

## Which name and password

On the box's network, type the sign-in name and password you use on LosOS
Login. The person who set up the box gets the owner's files. Anyone else
gets their own, and the first time they open the share LosOS Cloud makes
their account.

Through the internet, LosOS Login does not answer, so the share asks for an
app password instead. Make one in LosOS Cloud under **Settings** >
**Security** > **Create new app password**. LosOS Cloud then shows the name
and the password to type. An app password works on the box's network too,
and you can revoke it on its own.

## Linux

GNOME Files and KDE Dolphin find the box by themselves.

- **GNOME Files:** open **Other Locations**. Under **Networks**, click
  **LosOS Cloud on `<name>`**, then type your name and password.
- **Dolphin:** open **Network** in the places panel, then **LosOS Cloud on
  `<name>`**.

To type the address instead, use `davs://<name>.local/nextcloud/remote.php/webdav/`
in GNOME Files' **Connect to Server** field, or
`webdavs://<name>.local/nextcloud/remote.php/webdav/` in Dolphin's location
bar.

Choose to remember the password and the folder opens without asking next
time. GNOME Files can also add it to the sidebar as a bookmark.

## Windows

File Explorer does not list the share under **Network**. Add it by address:

1. Open **This PC**, then **Map network drive** (under the **...** menu on
   Windows 11).
2. Pick a drive letter. In **Folder**, type
   `https://<name>.local/nextcloud/remote.php/webdav/`.
3. Tick **Connect using different credentials** and **Reconnect at
   sign-in**, then **Finish**.
4. Type your name and password, and tick **Remember my credentials**.

The share then appears under **This PC** with its own drive letter.

:::note[Large files on Windows]
Windows' own WebDAV client refuses files larger than about 50 MB unless
its limit is raised. For large files or whole folders, use the desktop
app below.
:::

## macOS

The Finder does not list the share under **Network** either. Add it by
address:

1. In the Finder, choose **Go** > **Connect to Server** (Command-K).
2. Type `https://<name>.local/nextcloud/remote.php/webdav/` and click
   **Connect**.
3. Choose **Registered User**, type your name and password, and tick
   **Remember this password in my keychain**.

Click **+** before **Connect** to keep the address in the list of favourite
servers.

## The desktop and phone apps

The share is for opening files where they are. To keep a copy of a folder
in sync, or to upload photos from a phone, use the LosOS Cloud apps (the
Nextcloud desktop client and the Nextcloud phone apps). Give them the
address `https://<name>.local/nextcloud`. They open LosOS Login in a
browser to sign you in, then keep an app password of their own.

On a Linux computer with the `losos` terminal client, `losos cloud setup`
sets up the file manager for you; see [The terminal client](terminal).
