---
title: Sign-in and the spare key
sidebar_position: 2
slug: /manual/sign-in-and-spare-key
---

# Sign-in and the spare key

Each person signs in to the admin pages on LosOS Login, the box's sign-in
page, with their own sign-in name and password. The box also has one spare
admin key, shown once during setup.

## Your sign-in name and password

The wizard's password step asks for a sign-in name and a password. The
person who finishes it becomes the box's first administrator. The same
password also becomes the owner's password for LosOS cloud, so one password
opens both.

A sign-in name is lower case. It starts with a letter, then letters, digits,
`.`, `_` or `-`, at most 32 characters. A new password needs at least 12
characters, a lower-case letter, an upper-case letter, a digit and a symbol.

The box keeps each password only as a hash. There is no reset link, because
the box sends no mail. If you lose yours, see
[Forgot the password](../troubleshooting/forgot-password).

## LosOS Login

LosOS Login is the box's sign-in page at `https://<box>/auth/`, in the LosOS
colours. When you open the admin pages signed out, a dialog titled **Sign in
to this box** has a **Sign in** button. It takes you to LosOS Login and
brings you back once you are in. While LosOS Login is still starting, right
after a boot, the admin pages say "LosOS Login is starting" with a spinner
and a few facts about LosOS, then go on by themselves.

![The Sign in to this box dialog, with the Sign in button and the link to use the spare admin key instead.](../img/unlock-dialog.png)

LosOS Login works over HTTPS only. A plain `http://` address for it is
sent on to `https://`. Until the box's certificate is installed, the browser
warns first; see [Certificate warning](../troubleshooting/certificate-warning).

A sign-in lasts 12 hours, and ends sooner after 2 hours with nothing done.
Tick **Remember me** on LosOS Login to stay signed in on that computer for a
month. **Sign out** in the admin pages ends it at once.

:::note[Wrong attempts]
After a few wrong passwords in a row, LosOS Login refuses that name for
a few minutes. Wait, then try again.
:::

## LosOS cloud and LosOS Git

On the box's own network, LosOS cloud and LosOS Git sign you in with LosOS
Login too. Open either one signed out and you land on LosOS Login; sign in
there once and both apps let you in, with no second password.

The person who set up the box arrives in both apps as the owner's account,
the one that holds the files and repositories from before. Anyone else gets
an account of their own in each app the first time they open it, named
after their sign-in name.

The apps keep their own sign-in page where LosOS Login cannot help:

- through the internet (the box's public address), where LosOS Login does not
  answer;
- while LosOS Login is still starting after a boot;
- on LosOS cloud, at `/nextcloud/login?direct=1`, if you ask for it.

There, the owner signs in with their password as before. Desktop and phone
apps, WebDAV and `git` keep using app passwords and tokens.

**Log out** in LosOS cloud also signs you out of LosOS Login. LosOS Git's
**Sign out** does not: while you are signed in to LosOS Login, LosOS Git
signs you straight back in. Sign out of LosOS Login (in the admin pages or
in LosOS cloud) to leave both. An app tab that is already open stays signed
in until you sign out of that app too.

## Who sees what

The admin pages are split into sections: storage, backup, apps, market
(with virtual machines), network (custom domains), look, the Lab and access
tokens. An administrator can use all of them, and only an administrator can
apply settings or use Mesh, Hardware, Security, Advanced, History, About and
Reset, erase or restore the box, or see the recovery code. A person who is
not an administrator gets the sections they were given, or reads every
section without changing anything. Anyone signed in sees the overview. The
admin pages hide the panes a person cannot use.

The first person on a box is always an administrator. The admin pages have
no page for adding more people yet.

## The spare admin key

The box made a 64-character key on its first start. The wizard showed it to
you once, after you set the password, with **Copy** and **Print** buttons.
The key opens every pane, as an administrator, without LosOS Login. So it
still works on a day LosOS Login does not.

On the **Sign in to this box** dialog, choose **Use the spare admin key
instead** and paste the key. The tab stays unlocked until you close it.

![The dialog asking for the 64-character spare admin key.](../img/unlock-spare-key.png)

![A wrong spare key: the dialog says the key was not accepted and stays open.](../img/unlock-refused.png)

After ten wrong keys from one computer, the box locks that computer out. The
lockout starts at one second and doubles, up to five minutes.

:::warning[Keep the spare key away from the box]
Keep the printed sheet away from the box. Anyone with the key can change
every setting, which amounts to root. The box cannot show the key again or
rotate it, because it has no shell. If you lose the password *and* the spare
key, you have to reinstall from the stick, which wipes the disk.
:::

## A box set up earlier

A box set up before people had their own sign-in names has nobody to sign in
as. Its dialog says **Make your sign-in** and asks for a sign-in name, a
password and the spare admin key. The key proves the box is yours, and you
become its first administrator. From then on, sign in with that name and
password.

## Passkeys

Once the certificate is installed and you connect over HTTPS, LosOS cloud
offers a passkey. A phone or laptop then signs in to LosOS cloud with its
own unlock instead of the password. LosOS Login does not take passkeys and
always asks for your name and password. A passkey is not a second spare key.

## The claim window

A fresh box has no owner. The first browser on the LAN to finish the wizard
becomes the owner and its first administrator, and the window closes. So run
the wizard soon after the install, on a network you trust. Nothing outside
the LAN can claim a box, and only a reinstall lets anyone claim it again.
