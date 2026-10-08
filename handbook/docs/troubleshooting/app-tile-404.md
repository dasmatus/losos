---
title: An app tile gives 404, or "untrusted domain"
sidebar_position: 8
slug: /troubleshooting/app-tile-404
---

# An app tile gives 404, or "untrusted domain"

<div className="losos-symptom">

**What you see:** clicking **Tasks**, **Calendar** or another tile on the
Overview opens a plain "Not Found" page, or LosOS cloud shows "Access
through untrusted domain".

</div>

![The Apps pane with LosOS cloud and LosOS Git; the Overview's app tiles open the same apps.](../img/apps.png)

## "Not Found" from an app tile

There are two causes. Pull requests #65 and #66 fixed both on 6 October
2026, and the nightly update brings the fixes:

- The tiles linked to the short `/nextcloud/apps/<app>/` form, which LosOS
  cloud's web server did not answer in the default
  [app mode](../types/app-modes). The tiles now go through
  `/nextcloud/index.php/apps/<app>/`, which every configuration answers.
- The app itself is not installed or is disabled in LosOS cloud. Open LosOS
  cloud → Apps and enable it.

Until the update lands, open LosOS cloud from the **Files** tile or at
`/nextcloud`, and use its own app menu. Its links work.

## "Access through untrusted domain"

LosOS cloud answers only to names it trusts. On each request the box tells
it which address the request arrived on, so both `<name>.local` and the IP
address work. The box never hands it a wildcard. You see this page when:

- you reached the box by a name that is not its own: an alias from your
  router, a hosts-file entry under another name, or a domain through an edge
  that the box was not told about. Use `<name>.local` or the address.
- you changed the box's name a moment ago and LosOS cloud has not
  restarted yet. Wait for the rebuild to finish.
- you reached a box from before 6 October 2026 by IP address. Pull
  request #65 added the per-request trust. Use `<name>.local` until the
  update.

## LosOS Git links show the wrong name

LosOS Git prints clone URLs with the box's `.local` name even when you reached
it by address. The pages work either way. For `git clone`, put the address in
place of the name, or add the name to your hosts file as
[The name does not resolve](name-does-not-resolve) describes.
