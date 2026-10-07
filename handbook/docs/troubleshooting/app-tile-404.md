---
title: An app tile gives 404, or "untrusted domain"
sidebar_position: 7
---

# An app tile gives 404, or "untrusted domain"

<div className="losos-symptom">

**What you see:** clicking **Tasks**, **Calendar** or another tile on the
Overview opens a plain "Not Found" page, or LosOS cloud shows "Access
through untrusted domain".

</div>

## "Not Found" from an app tile

Two causes, both fixed on 6 October 2026 (pull requests #65 and #66), both
brought by the nightly update:

- The tiles linked to the short `/nextcloud/apps/<app>/` form, which LosOS
  cloud's web server did not answer in the default
  [app mode](../types/app-modes). The tiles now go through
  `/nextcloud/index.php/apps/<app>/`, which every configuration answers.
- The app itself is not installed or is disabled in LosOS cloud. Open LosOS
  cloud → Apps and enable it.

Until the update lands, open LosOS cloud from the **Files** tile (or
`/nextcloud`) and use its own app menu, which generates working links.

## "Access through untrusted domain"

LosOS cloud only answers to names it trusts. The box tells it, per request,
which address the request arrived on, so both `<name>.local` and the IP
address work, and never a wildcard. You see this page when:

- you reached the box by a name that is not its own: an alias from your
  router, a hosts-file entry under another name, or a domain through an edge
  that the box was not told about. Use `<name>.local` or the address.
- the box's name was changed a moment ago and LosOS cloud has not been
  restarted yet. Wait for the rebuild to finish.
- a box from before 6 October 2026, reached by IP address: pull request #65
  added the per-request trust. Use `<name>.local` until the update.

## LosOS Git links show the wrong name

LosOS Git prints clone URLs with the box's `.local` name even when you reached
it by address. The pages work either way; for `git clone`, substitute the
address, or add the name to your hosts file
([The name does not resolve](name-does-not-resolve)).
