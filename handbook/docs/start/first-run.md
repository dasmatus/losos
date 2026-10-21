---
title: The first run
sidebar_position: 3
---

# The first run

The first time you open the box's address, the admin pages show a three-step
wizard. Do it soon after the install, from a computer on your own network.
Whoever finishes the wizard first owns the box.

## Step 1: Trust this box

The box serves HTTPS with a certificate it made itself. Your browser has
never seen that certificate, so it does not trust it yet. Everything after
this step should travel over the encrypted connection, including the password
you are about to set. That is why the wizard asks you to install the
certificate first.

<img src={require("../img/wizard-step1-trust.png").default} alt="Step 1 of the wizard: the one-line installers for macOS, Linux and Windows, the certificate download and the fingerprint to compare." width="520" />

The quick way is one line in a terminal, shown for the computer you are on:

```bash title="macOS and Linux"
curl -fsSL http://<address>/setup/trust.sh | sh
```

```powershell title="Windows (PowerShell)"
irm http://<address>/setup/trust.ps1 | iex
```

What that line does, and does not do:

- It adds **one certificate** to the stores your browsers read, for your
  user only: the login keychain on macOS, the user's Trusted Root store on
  Windows, the NSS stores Chrome and Firefox use on Linux. It installs no
  software and never asks for administrator rights. Deleting that one entry
  undoes it.
- It comes **from the box, on your own network**, not from the internet. It
  is short, and you can open the link in a tab and read it first.
- It prints the certificate's **fingerprint**, which you compare with the one
  on the wizard page.
- macOS asks for your account password once. The keychain asks for it, not
  the script. Windows shows its own "install this root certificate?" dialog.

:::note[The certificate on a phone]
Phones have no terminal. On a phone, download `losos-ca.crt` from the same
page and add it as a trusted certificate in the phone's settings. You can do
the same by hand on a computer.
:::

Then restart the browser and reopen the page at `https://<name>.local` or
`https://<address>`. You can also skip this step and come back to it later.
If it goes wrong, see the
[certificate warning](../troubleshooting/certificate-warning) page.

## Step 2: Choose how you sign in

Choose a **sign-in name** and set the **owner password**. The name is lower
case: it starts with a letter, then letters, digits, `.`, `_` or `-`, at most
32 characters. The password needs at least 12 characters with a lower-case
letter, an upper-case letter, a digit and a symbol such as `-` or `!`. The
wizard ticks each rule as you type. You become the box's first
administrator, and you sign in to the admin pages on LosOS Login, the box's
sign-in page at `https://<box>/auth/`, with this name and password. The same
password opens LosOS Cloud and LosOS Git.

<img src={require("../img/wizard-step2-password.png").default} alt="Step 2 with a sign-in name and a password typed in: each of the four password rules is ticked as it is met." width="520" />

The step waits until LosOS Cloud has finished its own first start, which takes
a few minutes on a fresh box. Meanwhile the page shows "waiting for" and the
reason. It continues on its own.

<img src={require("../img/wizard-step2-waiting.png").default} alt="Step 2 while LosOS Cloud is still installing itself: the page says what it is waiting for and continues on its own." width="520" />

:::warning[The spare key is shown once]
When the password is set, the page shows the **spare admin key** once: 64
characters, with **Copy** and **Print** buttons. Print it or write it down and
keep it away from the box. It opens the admin pages when LosOS Login is not
answering. The page cannot show it again. See
[Sign-in and the spare key](../manual/sign-in-and-spare-key).
:::

When the password is set, the step also shows two rows, and both take the
password you just set:

- **Your name on LosOS Login**: your new sign-in name. It opens the admin
  pages.
- **Your LosOS Cloud account**: the account you type in step 3 and in the
  desktop and phone apps.

<img src={require("../img/wizard-step2-spare-key.png").default} alt="Step 2 after the password is set: your name on LosOS Login, your LosOS Cloud account, and the spare admin key shown once with Copy and Print." width="520" />

A **passkey** for LosOS Cloud, so a phone or a laptop can sign in to your
files without typing the password, can be added later from the security
settings inside LosOS Cloud.

## Step 3: Sign in

The wizard opens LosOS Cloud inside the page, and you sign in with your
LosOS Cloud account and the password you just set. Then the wizard takes you
to the admin pages' overview. From here on, see [The admin
pages](../manual/admin-pages).

The first time, a short tour opens over the overview. Each step shows a
small moving picture of the page, with the sidebar entry where that feature
lives. Every box has six steps: LosOS Cloud's apps, adding more, storage,
lending the box at night, the nightly update and where to find the tour
again. The tour asks the box what else it has and adds a step for each:
Office, lending memory to AI models, a name to be paid at, chat and calls
(only while the box shares its storage in a mesh), Build with AI and
profile pictures. A box with none of these sees the six steps.
**Skip the tour** closes it, and that browser does not open it by
itself again. **Take the tour** under Settings, About brings it back.

![The tour's first step over the overview: a picture of the page with LosOS Cloud's six apps, "1 of 6", and Skip the tour and Next.](../img/tour.png)

![Step 3: You are signed in. That was the last step.](../img/wizard-step3-sign-in.png)
