---
title: Changing the pages
sidebar_position: 5.1
slug: /manual/pages
---

# Changing the pages

Each page of the admin pages is written in MDX: Markdown with a few tags.
An administrator can change what a page says and in which order it shows
its parts, add buttons of their own, and give every page a stylesheet. A
page nobody changed looks exactly as LosOS ships it.

So far the Overview and Storage are pages you can change. The other screens
and the apps follow.

## Who can change them

Only administrators. The **Pages** section of **Settings → Look** is shown
to them alone, and the box refuses a change from anyone else. Everyone who
is signed in sees the pages as an administrator left them, in every
browser, with JavaScript on or off.

## Editing a page

**Settings → Look → Pages** lists each page, marked **As shipped** or
**Changed**. The pencil next to one opens the page with the editor docked
beside it (below it on a phone). The page itself is the preview: what you
type shows on it as you type, and nothing is saved until you press
**Save**.

A page is made of three kinds of thing, in the order you write them:

- **Text**: headings, paragraphs, lists, links and the rest of Markdown.
- **The page's blocks**: the parts LosOS draws, such as the apps or the
  disk, each on a line of its own, like `<Apps />`. A block you leave out
  is not shown. The editor lists the blocks of the page you are editing,
  and what each one draws.
- **Components**: the ones a LosOS document can use (Callout, Columns,
  Steps, Badge and the others), and `Button`.

The Overview as it ships is only its blocks:

```mdx
<BoxName />

<Applying />

<Apps />

<Glance />
```

Here is the same page with a welcome, a button and the apps without the
storage strip under them:

```mdx
<BoxName />

## Welcome to the office box

Files and code live here. Ask before you install an app.

<Button href="/apps" variant="secondary">Open the apps</Button>

<Apps hideStorage />
```

The editor lists what is wrong with a page, by line and column, and
**Save** waits until nothing is. The box checks every save the same way,
so a page that does not read is never stored. There is no script on a
page: MDX's `import`, `export` and `{expressions}` are refused, and only
the tags above are drawn.

## Buttons

`<Button href="…">Label</Button>` draws one of the page's own buttons. The
address is a path on this box (`/apps`, `/settings/look`) or an `https:`,
`http:` or `mailto:` link. `variant` is `primary` (the default),
`secondary`, `outline`, `ghost` or `link`; `size` is `sm`, `md` (the
default) or `lg`. A button is a link: it opens a page and changes nothing
on its own.

## The stylesheet

The **Stylesheet** row opens a CSS editor. Every page uses the stylesheet
once you save it, and it comes after LosOS's own, so a rule of yours wins
over an equal one. Each page names itself on the root element, so
`html[data-page="overview"]` is the Overview and `html[data-page="settings.storage"]`
is Storage. Text you wrote is inside `.mdx-page-text`. While the editor is
open every page you visit shows the stylesheet you are typing.

## History and going back

Every save, reset and stylesheet change is a commit, made in your name, in
a Git repository of its own on the box: `/var/lib/losos/pages`, apart from
the box's configuration. The editor's **History** lists a page's commits.
**Back to the default** puts a page back as LosOS ships it, and
**Remove the stylesheet** removes yours. Both are commits too, so the
history stays.

When a LosOS update changes the default of a page you changed, the editor
says so. Your page stays as you left it; **Open the new default** shows
the new one, so you can take what it added.
