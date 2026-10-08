---
title: The wizard keeps waiting
sidebar_position: 7
slug: /troubleshooting/wizard-waits
---

# The wizard keeps waiting

<div className="losos-symptom">

**What you see:** step 2 of the first run says "Waiting for this box to
finish starting", and stays there.

</div>

<img src={require("../img/wizard-step2-waiting.png").default} alt="Step 2 of the wizard waiting for LosOS cloud, with what it is waiting for." width="520" />

## Why

The password lives in LosOS cloud, so the wizard cannot set it until LosOS
cloud has installed itself. LosOS cloud does that on the box's first start.
On a mini-PC that
takes two to five minutes; in an emulated VM without hardware acceleration it
can take fifteen. The page checks every few seconds and continues on its own.

## What to do

1. **Wait ten minutes** before anything else. Open
   `http://<address>/api/setup/claim` in another tab. If it says
   `"ready": false`, its `waitingFor` field names what the box is waiting
   for.
2. **After fifteen minutes**, pull the power to restart the box, and reopen
   the wizard. LosOS cloud's first start is restartable.
3. **A box that never gets there** after a restart is rarely missing a
   network, because it fetches nothing and everything is on the box. It
   usually has a full disk or a failed install. Check the Storage pane if
   the admin pages open, otherwise reinstall.

## A known bug

Builds from before 6 October 2026 could leave the panel on "starting" after
LosOS cloud was ready. Pull request #62 fixed it. If you reload the page and
it goes straight on, you hit it.
