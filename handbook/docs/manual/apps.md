---
title: The apps
sidebar_position: 4
---

# The apps

## LosOS cloud

Your files, photos, calendar, contacts, notes, tasks, mail and music, at
`http://<address>/nextcloud`. It is Nextcloud with the box's colours and
name. Sign in with the owner's password. LosOS cloud does everything
Nextcloud does. The desktop client syncs a folder on your computer, the phone
apps upload photos, and calendars and contacts sync over CalDAV and CardDAV.

![The marks of LosOS cloud and LosOS Git in the light and the dark theme, drawn from the plate of salmon.](../img/cloud-and-git-marks.png)

The app tiles on the Overview open each app. Their links go through
`index.php` on purpose, because every configuration answers that path.

You make accounts for family or colleagues inside LosOS cloud, under Users,
not on the admin pages. Only the owner's account unlocks the admin pages.

## LosOS Git

Your repositories, at `http://<address>/forgejo/`. It is Forgejo in the same
colours. Sign up inside it the first time. It keeps its own accounts. Clone
URLs use the box's name, so a computer that does not resolve `.local` should
use the address instead. Turn it off in the **Apps** pane if the box is not
for code.

### Federation

LosOS Git speaks ActivityPub with other Forgejo servers. Forgejo still calls
this experimental. Stars that people on another server give your repositories
count here. People on other servers can follow an account on your box and see
what it does. The box also answers the fediverse's discovery address,
`/.well-known/nodeinfo`. All of this reaches as far as the box does. Through
an [edge](../types/official-edge) that is the internet. On a box that is only
on your local network, it is the other boxes on that network. The box never
publishes how many accounts it has or how active they are.

To take part, list the other servers whose stars count on a repository's
**Settings → Federation** page. To turn federation off entirely, use
`losos.forgejo.federation.enable` on the **Advanced** pane.

## The Apps pane

For each app the pane shows whether it is on, which
[mode](../types/app-modes) it runs in, and a link. Below that, **Find more**
searches LosOS cloud's app catalogue. The box fetches the catalogue, so the
search needs the internet. To install an app you like, sign in to LosOS cloud
with the owner account and install it there.

![The Apps pane with Find more: a search of LosOS cloud's app catalogue and the results with where each comes from.](../img/apps-find-more.png)

## Trusted addresses

LosOS cloud answers on the box's name and on whichever address the request
arrived at. The box passes that address along with each request. LosOS cloud
never answers on a wildcard. That is why a box reached by its IP address
shows no "untrusted domain" page. If you ever see one, [Untrusted domain](../troubleshooting/app-tile-404#access-through-untrusted-domain)
says why.
