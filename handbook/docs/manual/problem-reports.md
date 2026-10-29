---
title: Problem reports
sidebar_position: 6.5
slug: /manual/problem-reports
---

# Problem reports

A box has no shell and often nobody watching it, so it keeps its own list
of what went wrong. When something on that list is new to the project, it
writes a report you can read in full and send to the developers as a
GitHub issue. Nothing leaves the box until you send it.

## What the box notices

Once a minute the box reads its log for three kinds of problem:

- a service that **stopped with an error**, such as a failed start or a
  timeout;
- a program that **crashed**, ended by a signal like `SIGSEGV` or
  `SIGABRT`;
- a program the kernel **stopped because the box ran out of memory**.

The same problem again is one entry with a count, not a new one.
**Settings → Problems** lists them, newest first.

The box does not write core dumps. A core dump of a running service would
hold its secrets, so the report is written from the log instead.

## Problems the handbook explains

Some failures already have a page here, such as a disk that is
[out of room](../troubleshooting/out-of-room.md) or an
[Apply that cannot download](../troubleshooting/apply-fails.md). For
those, the entry says which page and links to it, and offers no report:
the page says what to do, and the developers already know.

## Reading a report

**Read the report** opens the whole text, exactly as GitHub would get it:

- what happened, when and how many times;
- the service's state;
- the log of that service around the time it failed, or the kernel's
  lines for a crash or a memory kill;
- the machine: release, firmware, processor, memory and kernel.

Before you see it, the box takes out what could identify you: IP and MAC
addresses, e-mail addresses, long keys and tokens, Tor and I2P names, the
box's own name and the names of the people who use it. Read it anyway. A
file name or a message in a log can still say more than you want public.

## Sending one

**Send on GitHub** asks GitHub for a code. Open
`github.com/login/device` on any phone or computer, sign in, and enter
the code the pane shows. The report then goes out as an issue under your
GitHub account, so the developers can ask you about it. The box keeps the
sign-in only until the report is sent.

You can add a note first, such as what you were doing when it happened.
It goes above the report.

If somebody already reported the same problem, your report is added to
their issue as a comment rather than opening a second one. Each report's
title ends in a short fingerprint, such as `[8c1f04e2]`, which is what
makes two reports the same.

**Stop** ends the wait for the code at any time. Nothing is sent.

**Dismiss** takes an entry off the list. It comes back if the problem
happens again.

## From the installer

The installer medium reports the same way, on its own screen:

- When an install **fails** with something this handbook does not
  explain, the screen offers a report. When the handbook does explain it,
  the screen names the page instead.
- While the install runs, the installer watches **memory and processor
  use**. The medium runs entirely from memory, so if memory stays above
  90 % for a few seconds, or the processor above 95 % for a minute, it
  says so at the end, even when the install finished, and offers a report.

The whole report is on screen 7: hold **Alt** and press **F7** to read
it, and **Alt+F1** to come back. Press **R** to send it. The screen then
shows the address and the code to enter on another device, and the issue's
address once it is filed. Any other key goes on without sending.

## Without JavaScript

The [pages without JavaScript](without-javascript.md) have the same
**Problems** pane. The page reloads itself every few seconds while it
waits for the code.
