---
title: Widget builder
sidebar_position: 4.7
mdx:
  format: md
---

# Widget builder

You describe a widget in plain words and an AI agent writes it. The widget
editor (**Add a widget**, then **Write one**) has a **Build with AI** tab
next to **Write**. The widget's files land in the editor, where you can read
them, try them in the preview and change them. Nothing is saved until you
press Save, and a widget the AI wrote runs in the same sandboxed frame as one
written by hand (see [Administration](administration.md#widgets-written-by-hand)).

The tab works when the box's edge offers the builder. Otherwise it says so.

## What it costs

The tab shows the price per million tokens the AI reads and writes, and the
most one build can cost. Builds are paid from a prepaid balance. You top it
up with one of the packs the tab offers, on Stripe's payment page, and the
balance changes once Stripe confirms the payment.

A build is charged for what it used, also when the AI ends without a usable
widget. A build that cannot be finished is not charged. When a build reaches
its spending cap, the tab says the widget may be unfinished.

## Anthropic's terms

The description and the files you send for a change go to Anthropic and are
subject to Anthropic's [Usage Policy](https://www.anthropic.com/legal/aup) and
[Commercial Terms](https://www.anthropic.com/legal/commercial-terms). The tab says so under the Build button.

## Good to know

- A build takes a minute or two and carries on if you close the window.
- A box runs one build at a time.
- To change a widget, keep it in the editor and tick the box that asks for
  a change. The AI gets all of the widget's files and sends back the ones it
  keeps.
- The AI's widget can read the box's readings like any widget. It cannot
  change the box.
