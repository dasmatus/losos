---
title: The apps
sidebar_position: 4
slug: /manual/apps
---

# The apps

## LosOS Cloud

Your files, photos, calendars, contacts and boards, at
`https://<address>/cloud/`. Everyone who can sign in with LosOS Login
([Signing in](sign-in-and-spare-key)) gets their own. The Nextcloud desktop
client and phone apps sync with it, and calendar apps reach it over CalDAV
and CardDAV. [LosOS Cloud](losos-cloud) has the details. On the Overview,
the **Files** and **Photos** tiles open it.

## LosOS Git

Your repositories, at `http://<address>/git/<account>/<repository>.git`.
LosOS Git is the box's own Git server. A person LosOS Login knows has an
account in it from their first visit, under the same name. A computer clones
and pushes with a token of its own, which git asks for as the password:
`losos git setup` makes one ([The terminal client](terminal)). The box's own
configuration is the private repository `notshared/losos-config`
([Administration](../in-depth/administration)). Clone addresses follow the
address you reached the box by. Turn it off in the **Apps** pane if the box is not for code.

## The Apps pane

For each app the pane shows whether it is on, and a link. Under them,
LosOS Cloud and LosOS Git each have a fold of their own, closed until you
open it. Below that, **Find more** searches [Artifact Hub](https://artifacthub.io) for Helm charts. The box
fetches the results, so the search needs the internet. Every row names who
published the chart. Nobody on the LosOS side has checked any of them.

![The Apps pane with one installed app above Find more, and search results from Artifact Hub, each with who published it and an Install or Change button.](../img/apps-find-more.png)

## Settings for everyone

In the LosOS Cloud and LosOS Git folds, the Apps pane has the settings that
hold for every account in each app.

For LosOS Cloud: how many days the trash keeps a deleted file. An empty
field keeps it for 30 days.

For LosOS Git: whether only signed-in accounts see anything, whether a new
repository starts private or public or like the account's last one, and
whether pushing to a repository that does not exist yet creates it.

![The Apps pane with the settings for everyone in LosOS Cloud and LosOS Git.](../img/apps-site-settings.png)

Press **Apply** to use them, like every other setting. The box sets them
again each time the app starts, so a change made inside an app does not
last. Each account's own settings stay in the app.

## Installing an app from the search

A row the box can fetch has an **Install** button. It opens a dialog in
two steps.

The first step reads the chart and shows three things.

- **Name on this box.** It starts as the chart's name. It also names the
  app's folder, so it cannot change later.
- **Run it as.** `notshared` or `shared`, the box's two users. A
  `notshared` app keeps its data on the owner's side of the disk, next to
  your files. A `shared` app keeps it on the side the box lends to the
  mesh. The dialog greys `shared` out while the box does not share its disk.
- **Properties.** Every setting in the chart's values file, with its
  default, folded by section. When the chart ships a schema, each setting
  also gets its description and its choices. **As text** shows the whole
  values file to edit instead. A property you change on the form wins over
  the same line in the text.

![The install dialog: the name, the choice between notshared and shared with shared greyed out, and the chart's properties folded by section.](../img/apps-install.png)

The second step asks once more. It says that the box will download and run
code the publisher wrote, as the user you chose, with no admin rights. The
box gets nothing until you press **Install** there.

What runs the app follows from the user. A `notshared` app runs on
rootless Podman, as `notshared` itself, and needs no cluster. A `shared`
app runs in the box's own cluster, because that is the side of the box the
mesh touches. Either way the box changes the chart to run the way LosOS
Git does.

- Every part of the app runs as the chosen user, never as root, with no
  extra privileges and no access to a cluster.
- The storage the chart asks for becomes folders under
  `/home/<user>/data/apps/<name>`.
- The app gets one port of its own from `losos.apps.ports`, 30000 to 30099
  by default, and only your local network can reach it.

:::note[Charts the box refuses]
The box refuses a chart that needs more than that and says why. Typical
reasons are admin rights, a port below 1024, a port another app already
uses, or a folder of the system. A `notshared` app's services reach each
other by name, on the box itself, so they have to listen on different
ports. A `shared` app built from several services that call each other by
name may not start, because the box's own cluster has no internal network.
A scheduled job (a CronJob) runs only as `shared`.
:::

**Installed apps**, above the search, lists each app with its state.
**Open** goes to the app's port on the address you are using. **Change**
opens the same dialog on the app's current properties. **Remove** asks
first, then takes the app off the box. Its data folder stays, so an install
under the same name finds the data again.

:::note[Running as shared needs a mesh]
A `notshared` app installs on any box. `shared` needs the box's own
cluster, which runs only while the box shares its disk with a mesh
([Where the apps run](../types/app-modes)).
To turn installing off, set `losos.apps.enable` to false on the
**Advanced** pane.
:::

## Apps made for the box

Apps can also come as a Compose file in the
[LosOS app format](../in-depth/app-format), one you write or one a program
sends for you. They run as `notshared`, show in
**Installed apps** with the line "made for this box", and have **Open**
and **Remove** but no **Change**, since they have no properties.
