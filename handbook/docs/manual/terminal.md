---
title: The terminal client
sidebar_position: 10
slug: /manual/terminal
---

# The terminal client

`losos` is a terminal program that manages a box from another computer.
It shows the same panes as the admin pages, in the same words and colours,
and needs no SSH, because the box has none. Every request it sends is
signed by your own GPG key and encrypted to the box. The box answers the
same way, so only the box can read the request and only you can read the
answer.

It runs on 64-bit x86 Linux.

## Before you install the box: seed your key

The box only accepts requests signed by a GPG key it was given at install
time. Nothing on a running box can add one later. If it could, anyone who
got into the admin pages could add their own key and then make a token for
it.

Export the public half of your key:

```sh
gpg --armor --export you@example.com > owner.asc
```

Then set it as `losos.access.gpgKeys` in the configuration you build the
install stick from, and the installer copies it onto the box. When you run
the installer by hand, `losos-ctl install --gpg-key owner.asc` does the
same. The key needs an encryption subkey, because the box encrypts its
answers to it. `gpg --full-generate-key` makes one by default.

A box installed without a key can still use everything else. Its Access
tokens pane says that no key was seeded and makes no tokens.

## Make a token

Open **Settings → Access tokens** in the admin pages. The pane shows:

- **This box's key**, the fingerprint the terminal checks on its first
  connection;
- **Seeded GPG keys**, the keys the box was installed with;
- **Tokens**, the ones already made, with when each was last used.

Give the new token a name, pick the key it belongs to, choose **Read only**
or **Full access**, and pick when it expires. **Create token** shows it
once. Copy it then. The box keeps only a hash and cannot show it again.

A token works only in a request signed by the key it was made for, so a
token copied off a screen is no use without that key. A read-only token
can look at every pane and change nothing. No token can make, list or
revoke tokens, and none can reach the setup wizard or the sign-in.
**Revoke** ends one at once.

## Install the client

```sh
curl -fsSL https://proxy.losos.dasmat.us/updates/main/x86_64/install.sh | sh
```

The script downloads the program and its checksum, checks one against the
other, and puts `losos` in `~/.local/bin`, or in `/usr/local/bin` when you
run it as root. It never asks for a password. Read it first if you like.
It is short and says what it does at the top.

`losos` keeps your key and tokens in the desktop's keychain (GNOME
Keyring, KWallet or KeePassXC) through `secret-tool`. On Debian and Ubuntu
that is the `libsecret-tools` package, on Fedora and Arch `libsecret`. It
never writes them to a plain file.

## Connect

```sh
losos key import --gpg you@example.com
losos login https://mattbox.local
losos
```

`key import` copies your secret key into the keychain. `login` asks the box
for its key and shows the fingerprint in groups of four. Check that it
matches the one on the Access tokens pane before you answer yes. Then
paste the token, which `losos` does not echo. From then on `losos` remembers the box's key,
and refuses to talk to it if the key ever changes.

`losos` on its own opens the full-screen view: the sidebar on the left, the
pane on the right, the keys at the bottom. Arrows or `j` and `k` move,
Enter changes a setting, Tab moves between the sidebar and the pane, `/`
searches the panes, `?` lists the rest and `q` quits. A change stays a
draft until you press `a` to apply it, and the box asks before it rebuilds,
as the admin pages do.

Access tokens, the market, the Lab and anything that wipes data stay in
the admin pages.

## In a script

`losos call` sends one request and prints the answer:

```sh
losos call GET /api/state
```

It exits 1 when the box answers with an error, so a script can check it.
`losos --help` lists every command.
