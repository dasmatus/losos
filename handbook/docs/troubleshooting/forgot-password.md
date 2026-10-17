---
title: Forgot the password
sidebar_position: 5
slug: /troubleshooting/forgot-password
---

# Forgot the password

<div className="losos-symptom">

**What you see:** LosOS Login, the box's sign-in page at
`https://<box>/auth/`, does not accept your sign-in name and password, or
LosOS cloud's sign-in page does not accept the password.

</div>

## First, make sure it is the password

- **The sign-in name.** Sign-in names are lower case. Check the name as well
  as the password.
- **Too many tries.** After a few wrong passwords in a row, LosOS Login
  refuses that name for a few minutes, even with the right password. Wait,
  then try again.
- **A box set up earlier** has no sign-in names yet. Its dialog says **Make
  your sign-in** and needs the spare admin key; see
  [A box set up earlier](../manual/sign-in-and-spare-key#a-box-set-up-earlier).

## Someone else set up the box

Ask an administrator. Under **Settings > Users** they can set a new password
for you with **New password**; see
[Adding people](../manual/sign-in-and-spare-key#adding-people). Only the
owner's password needs the spare key, as below.

## You have the spare admin key

![The dialog asking for the spare admin key.](../img/unlock-spare-key.png)

On the **Sign in to this box** dialog, choose **Use the spare admin key
instead** and paste the 64-character key from the printed sheet. The admin
pages open.

The key also lets you set a new password for the owner, the person who set
up the box, through the same route the wizard used. It changes the owner's
sign-in password and their LosOS cloud password together. There is no button
for it on the panes yet, so you run one command from a computer on the LAN.
Replace the key, the address and the password. The new password must follow
the four rules:

```bash
curl -H "Authorization: Bearer <spare key>" -H "Content-Type: application/json" \
  -d '{"password": "Correct-horse battery staple 1"}' \
  http://<address>/api/set-password
```

The answer names the LosOS cloud account it reset. Sign in on LosOS Login
with your sign-in name and the new password, and to LosOS cloud with the same
password.

## You have neither

The box keeps only a hash of your password. There is no reset link, because
the box sends no mail, and no shell to run a reset on. Before you reinstall,
check whether another account in LosOS cloud has the admin role. That account
can reset the owner's password from LosOS cloud's Users page, which gets you
back to your files, but not into the admin pages. For the admin pages the
only way back is a **reinstall**, which wipes the disk, and then a restore of
your files from the other copy you keep, such as the desktop client's folder
or the phone's uploads.

## Preventing the next time

Print the spare key when the wizard shows it, and keep it away from the box.
Put the password in a password manager. Give a second trusted person an
administrator account under **Settings > Users**.
