---
title: The wizard keeps waiting
sidebar_position: 6
---

# The wizard keeps waiting

<div className="losos-symptom">

**What you see:** step 2 of the first run says "Waiting for this box to
finish starting", and stays there.

</div>

## Why

The password lives in LosOS cloud, so it cannot be set until LosOS cloud has
installed itself, which it does on the box's first start. On a mini-PC that
takes two to five minutes; in an emulated VM without hardware acceleration it
can take fifteen. The page checks every few seconds and continues on its own.

## What to do

1. **Wait ten minutes** before anything else. Open
   `http://<address>/api/setup/claim` in another tab: `"ready": false` with
   a `waitingFor` field says what it is waiting for.
2. **After fifteen minutes**, restart the box (pull the power) and reopen the
   wizard. LosOS cloud's first start is restartable.
3. **A box that never gets there** after a restart usually has no working
   network at the moment it needs to fetch nothing (everything is on the
   box) but a full disk or a failed install. Check the Storage pane if the
   admin pages open, otherwise reinstall.

## A known bug

Take 9 of the demo (6 October 2026) found the panel staying on "starting"
after LosOS cloud was in fact ready. That is pull request #62. If you reload
the page and it goes straight on, you hit it.
