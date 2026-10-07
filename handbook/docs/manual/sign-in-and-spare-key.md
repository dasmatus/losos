---
title: Sign-in and the spare key
sidebar_position: 2
---

# Sign-in and the spare key

A box has **one password** and **one spare key**.

## The password

The password set in the wizard is the owner's password for LosOS cloud. It
also unlocks the admin pages: when you open them, a dialog titled **Unlock
this box** asks for it, and the box checks it with LosOS cloud over its own
loopback connection. The box keeps no second copy, so changing the password
inside LosOS cloud (Settings → Personal → Security) changes it for the admin
pages too, at once.

A new password needs at least 12 characters, a lower-case letter, an
upper-case letter, a digit and a symbol. A password set before that rule
existed still signs in.

The tab stays unlocked until it is closed. Ten wrong attempts from one
computer lock that computer out for a growing while (one second, doubling, up
to five minutes).

## The spare admin key

The admin pages are really unlocked by a 64-character key that the box
minted on its first start. The password is a way of obtaining it. That key
was shown **once**, in the wizard after the password was set, with **Copy**
and **Print** buttons. It is the spare, for the one case the password cannot
cover: LosOS cloud not running, so nothing can check the password. The
unlock dialog then says so and offers **Use the spare admin key instead**.

Keep the printed sheet away from the box. Anyone with the key can change
every setting, which is as much as root. There is no way to see it again or
to rotate it from the box, because the box has no shell. Losing the password
*and* the spare key means reinstalling from the stick, which wipes the disk.

## Passkeys

On an HTTPS connection (after the certificate is installed), the wizard and
LosOS cloud offer a passkey, so a phone or laptop signs in with its own
unlock instead of the password. A passkey is a convenience, not a second
spare: it is checked by LosOS cloud like the password.

## The claim window

A fresh box has no owner. The first browser on the LAN to finish the wizard
becomes the owner and the window closes. That is why the first run should be
done soon after the install, on a network you trust. Nothing outside the LAN
can claim a box, and a box cannot be re-claimed without a reinstall.
