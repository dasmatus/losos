---
title: AI tools on your computer
sidebar_position: 10.5
slug: /manual/ai-agents
---

# AI tools on your computer

The terminal client can find the AI tools on your computer and put the
**LosOS companion** in them. The companion lets a tool look through your
files in LosOS Cloud and your repositories in LosOS Git, so you can ask it
about your own notes and code. It can only look. It cannot change, move or
delete anything on the box.

This runs on your computer, in the terminal client, because the admin pages
cannot look at your computer. The companion signs in with LosOS Login, the
box's own sign-in, in your browser.

## Which tools

| Tool | How the companion goes in |
|---|---|
| Claude Code | as a plugin, through `claude plugin` |
| GitHub Copilot CLI | as a plugin, through `copilot plugin` |
| Codex | as a plugin, through `codex plugin` |
| Gemini CLI | as an extension, through `gemini extensions` |
| Qwen Code | as an extension, through `qwen extensions` |
| opencode | an entry in `~/.config/opencode/opencode.json` |
| Claude Desktop | an entry in `~/.config/Claude/claude_desktop_config.json` |
| Cursor | an entry in `~/.cursor/mcp.json` |
| VS Code and VSCodium | an entry in `mcp.json` in the editor's user settings |
| Cline | an entry in its MCP settings inside VS Code |
| Windsurf | an entry in `~/.codeium/windsurf/mcp_config.json` |
| Zed | an entry under `context_servers` in Zed's `settings.json` |
| LM Studio | an entry in `~/.lmstudio/mcp.json` |

A tool is found by its program on your `PATH` or by its settings folder.
`losos agents` lists what it found:

```sh
losos agents
```

## Put the companion in

```sh
losos agents install --host https://mattbox.local
```

Use your box's address. If the terminal client is already connected to the
box (see [The terminal client](terminal)), you can leave `--host` out.

It asks about each tool it found, one at a time. Nothing is changed in a
tool you say no to. Name the tools to do only those, and add `--yes` to
skip the questions:

```sh
losos agents install claude-code zed --yes
```

If this computer has never connected to the box, the client shows the
fingerprint of the box's certificate. Compare it with the one your browser
shows for the box, and say yes if they match. From then on the companion
talks only to a box with that certificate.

Then the companion signs in. The client prints a link and a code:

```text
The companion signs in to https://mattbox.local with LosOS Login. Open this link in a browser, sign in and approve the LosOS companion:
  https://mattbox.local/auth/consent/openid/device-authorization?user_code=WDJBMJHT
The page shows the code WDJBMJHT. Waiting for the approval…
```

Open the link, sign in to LosOS Login as you always do, with your second
step if you have one, check that the page shows the same code, and choose
**Accept**. The terminal carries on by itself.

Before it changes a settings file, the client copies it to
`~/.local/share/losos/companion/backups/`. It changes only its own entry
and leaves your comments and your other servers as they were. Restart a
tool that was open so it picks the companion up.

Gemini CLI asks whether you trust the folder it installs from. Your yes to
Gemini CLI answers that too: the client adds only the companion's own
folder to Gemini's list of trusted folders, and takes it off the list again
when you remove the companion. Gemini CLI starts MCP servers, the companion
among them, only in folders you trust.

## What the companion can do

Once it is in, the tool has these tools from the companion:

- list a folder in LosOS Cloud, find files by name, and search the text
  inside them;
- list your repositories in LosOS Git and search the code on their
  default branches;
- say how the box is doing, and which account the companion reads as.

The companion reads what you can read. A person with their own account on
the box gets their own files, not yours.

## How it stays safe

The companion never sees your password. You type it into LosOS Login in your
browser, and LosOS Login gives the companion two tokens: one that opens the
box for an hour, and one that gets a new pair when that runs out. If the
companion goes 30 days without being used, it has to sign in again.

The box lets those tokens read files, repositories and the box's status, and
nothing else, even when you are an administrator. The companion sends only
reads, and the box refuses anything more. A person whose account is waiting
for a second step or a new password cannot sign the companion in until that
is done.

The tokens are kept in your desktop's keychain, the same place the terminal
client keeps everything else. They are never written into an AI tool's
settings: each tool starts the companion, and the companion reads the
keychain.

## Take it out again

```sh
losos agents remove
```

It asks about each tool the companion is in and takes it out, and puts each
settings file back as it was. Once the companion is in no tool, the client
signs it out: it hands both tokens back to LosOS Login and forgets them. The
box stops taking them within a minute.
