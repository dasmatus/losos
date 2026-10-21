---
title: Calls
sidebar_position: 11
slug: /manual/calls
---

# Calls

People on the box can call each other from LosOS chat, by voice or with
video, in a direct message or in a room of up to eight people. The call
goes through the box, and the box cannot see or hear it. Every frame of
picture and sound is encrypted in the browser that sends it and decrypted
in the browsers that receive it.

## When calls are there

Calls are part of chat, so they need what chat needs: the box shares its
storage and has joined a mesh. They also need
**Settings → Advanced → `losos.calls.enable`**, which is on unless you turn
it off. With any of these off, rooms have no call buttons and the box runs
nothing for calls. Turning one back on brings them back after the next
Apply.

## Starting a call

Open a room or a direct message. The header has two buttons, **Voice call**
and **Video call**. Your browser asks once for the microphone, and for the
camera when you start with video.

Everyone in the room gets a card in the corner of their chat page that
rings for a minute. **Answer** joins the way the call was started. On a
video call, **Answer without video** joins with the microphone only.
**Decline** only hides the card.

The room's timeline says that a call started. While it lasts, the line has
a **Join** button, so someone who missed the ring, or who left, can come
back. When the last person leaves, the timeline says the call ended.

## During a call

Each person has a tile. A tile shows their camera, or their picture while
their camera is off. The three buttons at the bottom turn your microphone
and your camera off and on, and leave the call. Turning the camera off
stops it, so its light goes out, and sends nothing until you turn it on
again.

The **End-to-end encrypted** label at the top says how many frames your
browser has sent encrypted, and how many it has received and opened.

## What the box sees

The box forwards each frame to the others in the call without opening it.
It knows who is in which call and when, how large the frames are and how
often they come. It never has the picture or the sound.

Each person's browser makes its own key for the call. Browsers hand their
keys to each other in chat's encrypted messages between devices, which
the box carries but cannot read. The keys change while the call runs:

- When someone joins, everyone moves to the next key, which the new person
  gets. They cannot open anything sent before they came.
- When someone leaves, everyone makes a new random key and gives it only to
  the people who stayed. The person who left cannot open anything sent
  after.
- Every five minutes, everyone moves to the next key anyway.

Keys live in the browser's memory only and are gone when the call ends.
Nothing is saved, so a key found later opens nothing from the call.

One limit is worth knowing. A browser sends its key to the devices the
box's chat server lists for each person in the call. Someone who had taken
over the box's software could list a device of their own. Verify each
other's devices in chat (**Secure messages**, then **Verify**) to know that a
device is really the other person's.

## Browsers

A browser needs to be able to encrypt frames to join a call. Current
Firefox, Chrome, Edge and Safari can. A browser that cannot shows no call
buttons, and it never sends a frame without encryption.

## Where calls work

Calls work between computers that reach the box on its own network. The
box takes the media on UDP port 7882
(**Settings → Advanced → `losos.calls.udpPort`**), which is open on its
firewall while calls are on. Calls from outside that network do not work
yet.
