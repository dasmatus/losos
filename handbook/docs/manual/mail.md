---
title: Mail
sidebar_position: 4.7
slug: /manual/mail
---

# Mail

LosOS Mail is a mail client for the people on the box. Each person signed in
through LosOS Login has a mailbox, and **Mail** in the sidebar opens it at
`/mail`. The mail server is Stalwart. It runs on the shared side of the
box, under the `shared` account, and keeps the mailboxes in the shared data
directory.

## When Mail is there

Mail exists only while the box shares its storage with a mesh it has
joined (see [storage modes](../types/storage-modes)). With sharing off, or
outside a mesh:

- there is no **Mail** in the sidebar, and `/mail` does not open;
- the mail server is stopped;
- the mailboxes stay on the disk, in the shared directory, which the box
  cannot read while sharing is off.

Once the box shares with its mesh again, Mail comes back with the same mail
in it.

## Your address

Your address is your LosOS Login name at the box's mail domain, for
example `matus@mattbox.home.arpa`. The box makes your mailbox the first time
you open Mail. There is no separate mail password: Mail is open while you
are signed in. The spare key opens the admin pages but has no mailbox, so
Mail asks you to sign in with your own account.

## Reading and writing

- **Folders**: Inbox, Drafts, Sent, Archive, Junk and Bin, with the unread
  count beside each.
- **Search** looks through the open folder: senders, subjects and text.
- **Write** opens a new message. The **To** field suggests the other people
  on the box. **Reply**, **Reply all** and **Forward** quote the message.
- **Flag**, **Mark as unread**, **Archive** and **Move to** work on the open
  message. **Delete** moves it to the bin, and deleting it from the bin
  removes it for good.
- Formatted mail opens as plain text. **Show formatting** draws the
  sender's layout in a sandboxed frame with no scripts, no forms and no
  pictures from outside the box, so the sender cannot tell that you opened
  it. A message with no plain text opens in that frame straight away.
- Attachments download to your computer. They never open inside the page.

## Encryption

Mail can encrypt and sign messages with OpenPGP, the standard GnuPG,
Thunderbird and most mail programs read.

**Your key.** **Encryption**, beside **Write**, makes a key for your address
or imports one you already have. You choose a passphrase for it; it is not
your sign-in password. The key is made and unlocked in your browser. The
box keeps only the private key locked with your passphrase, and seals that
again to its TPM or its own key, as it does its other secrets (see
[credentials](../reference/credentials)), so nobody on the box, an administrator included, can read your encrypted
mail without the passphrase. The same window shows the fingerprint, downloads
a backup (still locked with your passphrase) and the public key, locks the
key again, and removes it. A removed key cannot read mail sent to it until
you import the backup.

**Unlocking.** The key stays unlocked in the tab where you unlocked it, until
you lock it or close the tab. Mail asks for the passphrase when it needs it:
to open an encrypted message or to sign one.

**Writing.** Under the message, **Encrypt** and **Sign** switch each one on
or off. As you type an address, Mail looks for that person's key and shows
where it came from:

- someone on this box: the key they made here;
- your contacts: a key saved on the contact;
- earlier mail: a key the person sent in an Autocrypt header, learned when
  you opened their mail;
- their mail domain: the domain's Web Key Directory (WKD);
- keys.openpgp.org, when their domain has none. An administrator can point
  this at another keyserver, or turn it off, with `losos.mail.keyserver`.

When every recipient has a key, **Encrypt** turns on by itself. If one of
them has none, Mail says who, and the message is not sent until you turn
encryption off or leave them out. Encrypted mail is also encrypted to your
own key, so you can read your copy in Sent. The subject is not encrypted.
Mail is sent as PGP/MIME; **Inline, for older mail programs** puts the
encrypted text in the body instead, which protects the text but not
attachments.

**Reading.** An encrypted message shows a lock in the list. Opening it
decrypts it in your browser, and a line under the headers says how it came:
encrypted, signed and by whom, signed by a key the box does not know, or a
signature that does not match. A message signed by someone other than its
sender says so. Decrypted mail is shown as text, never as the sender's
formatting, and its attachments download like any other.

**Your public key for others.** The box publishes everyone's public key in
its own Web Key Directory at `/.well-known/openpgpkey/`, so mail programs
elsewhere find it from your address. Every message you send also carries it
in an Autocrypt header.

## What Mail does not do yet

Mail goes between the people on this box only. Mail to another domain is
refused with a message that says so, and nothing arrives from outside.
Receiving mail from the internet needs a public address and port 25, which
a home connection rarely has. The box also offers no IMAP or SMTP for other
mail apps: Mail in the admin pages is the only way in.
