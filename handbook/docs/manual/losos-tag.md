---
title: LosOS Tag
sidebar_position: 4.7
slug: /manual/losos-tag
---

# LosOS Tag

LosOS Tag is the assistant you tag in LosOS Chat. Write `@LosOS` in a
message — or accept the suggestion the composer shows when a room's
questions have gone unanswered — and the box answers in the thread with
what its own search found: your files in LosOS Cloud, the code in LosOS
Git, your mail, the rooms of chat. Nothing leaves the box; the answer
runs on the box's own model.

Tag lives in **Settings › LosOS Tag**, where you shape four things.

## Levels and areas

A *level* is one of the box's words for how private a piece of data is,
least private first (`everyday` by default; an administrator adds more —
`personal`, `secret`, whatever the box's own words are). An *area*
grades one piece of the box's data with one level: a folder in your
files, a repository, a room, a mailbox, a journal unit.

The rule is one line: **a tag reads an area only when its clearance
reaches the area's level.** The more private an area, the higher the
clearance the assistant needs. Your own clearance (what the tag and the
companion in your AI tools read as you) is yours to set on the same
pane.

## Assistants

Every assistant is a real chat participant — `@LosOS` is the built-in
one — with a persona of its own, a clearance, and a maker. Tag one in a
message and it answers; tag two and they answer, and an answer that
tags another assistant hands the turn on, three handoffs at most, so
two assistants can carry a thread between them without looping forever.
An assistant reads only up to its own clearance, whatever its persona
says: the levels are the box's rule, not the model's mood.

## Skills

A skill is an instruction an ask can carry — the seeded ones come with
the box (*Find where I wrote it*, *Summarise this thread*, *What did I
miss*, *Plan it*), and you may write your own. A skill never raises a
clearance and never grants a tool; it only says how to go about the
answer.

## What the tag never does

Whatever its clearance, a tag never calls `sudo` or a shell, never buys,
sells, rents or lends storage or compute, and never touches anything on
disk in the owner's directories — their files are read and written
through LosOS Cloud's own application, not by an assistant. A web
search, when the box runs one (its own SearXNG, never the open
internet's), takes the subject alone: no names, files or mail from the
box ever go into a query.

## Bigger models, over the mesh

The tag's model is the box's one unified model (Bonsai 27B by default —
a 1-bit recording of Qwen3.6-27B: 4.4 GB, tools, thinking, vision). A
box too small to hold it asks over the network instead: in a mesh, the
edge runs a head per model group and the boxes lend their memory to it
([the mesh](../in-depth/mesh), [AI models](../in-depth/ai-models)), so a
4 GiB box can ask Qwen 3.8 Max or DeepSeek R1 — the head answers, the
lenders hold the weights. The ask queue pauses what nobody waits on
while the box is short of memory, and resumes it on its own; nothing is
dropped.
