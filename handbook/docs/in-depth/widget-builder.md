---
title: Widget builder
sidebar_position: 4.7
mdx:
  format: md
---

# Widget builder

You describe a widget in plain words and a language model on the box writes
it. The widget editor (**Add a widget**, then **Write one**) has a **Build
with AI** tab next to **Write**. The widget's files land in the editor, where
you can read them, try them in the preview and change them. Nothing is saved
until you press Save, and a widget the model wrote runs in the same sandboxed
frame as one written by hand (see
[Administration](administration.md#widgets-written-by-hand)).

## It runs on the box

The model is Meta's Llama 3.2 with 3 billion parameters, served by
[Ollama](https://ollama.com) on the box itself. What you type and the widget
you ask to change never leave the box, and a build costs nothing.

The box downloads the model, about 2 GB, once, the first time it is online
after the install. Until then the tab says the model is still downloading.
The model is kept on the encrypted disk, so a reboot does not download it
again.

## What to expect

A build runs on the box's processor. On a mini-PC it takes a few minutes, and
the tab shows how many lines the model has written so far. You can stop it.

The model is small. It does well with one clear thing per widget: a clock, a
countdown, the disk as a bar, the list of apps, the weather for one town. Ask
for a single thing in a sentence or two. Look at the result in the preview
before you save it; sometimes a widget shows nothing or shows the wrong
reading, and then building again or a small edit by hand fixes it.

## Memory

While it writes, the model takes about 4 GiB of memory on top of what the box
uses anyway. It gives the memory back five minutes after the last build. With
8 GiB of memory or more there is room for both. On a box with less, a build
can fail for lack of memory, and the tab says so.

**Settings, Advanced** has `losos.builder.enable`. Turn it off and the box
neither downloads nor runs the model, and the tab says the builder is off.

## Llama's terms

Llama 3.2 is licensed under the
[Llama 3.2 Community License](https://www.llama.com/llama3_2/license/) and its
[Acceptable Use Policy](https://www.llama.com/llama3_2/use-policy/). The tab
says "Built with Llama" and links both under the Build button.

## Good to know

- A build carries on if you close the window.
- A box runs one build at a time.
- To change a widget, keep it in the editor and tick the box that asks for
  a change. The model gets all of the widget's files and sends back the ones
  it keeps.
- The model's widget can read the box's readings like any widget. It cannot
  change the box.
