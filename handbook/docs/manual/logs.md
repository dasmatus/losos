---
title: Logs
sidebar_position: 6.5
slug: /manual/logs
---

# Logs

**Settings › Logs** shows what the box's services wrote to the system
journal: the control service, LosOS Login, the web server, LosOS Cloud,
LosOS Git, the apps, backups, updates and the rest. When something fails,
this is where the box says why, and you read it in the browser. Nobody has
to sign in to the box itself.

Only an administrator, or whoever holds the spare admin key, can open the
pane. The lines say a lot about everyone who uses the box.

## Reading the lines

The pane opens on the control service's lines since the box last started,
newest at the bottom. Each line shows the time, how severe it is, the
service that wrote it and what it said. Hover over the service to see its
full unit name and process number.

The filter above the lines picks:

- **Service.** One of the box's services, **All LosOS services**, or **The
  whole journal**. Only the services this box runs are on the list.
- **Severity.** **Errors**, **Warnings and errors**, and so on down to
  **Everything**, which includes debug lines.
- **Since.** This start, the start before, the last hour, day or week.
- **Containing.** Only lines with this text, in any case.

**Show older lines** at the top brings in the lines before the first one on
screen. **Follow** keeps adding new lines as the services write them, until
you press **Stop following**. **Download** saves up to 5000 lines of the
current filter as a text file, for a bug report.

The address carries the filter, so a link such as
`https://<address>/settings/logs?unit=lososd&priority=3` opens on the
control service's errors.

## What is hidden

Before a line leaves the journal, the box hides anything that looks like a
secret and shows `[hidden]` in its place: passwords and tokens written as
`password=…` or `"token": "…"`, the credentials in an `Authorization` or
`Cookie` header, a password in an address, private keys, known token
formats, and long runs of hex or base64, which is what keys usually look
like. The spare admin key is hidden wherever it appears. The same applies
to the search and to the download, so neither can find a secret by its
letters.

It errs on the side of hiding: a long checksum is hidden too.

## When the admin pages do not load

The logs come from a small reader of their own, not from the control
service, so the pane keeps working when the control service has stopped
or is restarting. That is often the moment you need it.

If the other pages wait or show an error, open
`https://<address>/basic/settings/logs`. That page needs no JavaScript and
no control service. If LosOS Login is up you can sign in as usual; the page
also takes the spare admin key directly. It says when the control service
does not answer, and links to its lines.

## What is not there

The installer writes to the screen of the computer it runs on, not to the
journal, so its messages are not on the installed box. The installer shows
how it ended on that screen.
