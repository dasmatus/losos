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

This runs on your computer, in the terminal client. The admin pages cannot
look at your computer, so they only make the token the companion uses.

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

## Before you start

The terminal client must be connected to the box first (see
[The terminal client](terminal)). Then make one more token, just
for the companion: in **Settings → Access tokens**, choose **AI agents**
under **Access**. A token of that kind can read files and repositories and
nothing else, and it works only for the companion.

## Put the companion in

```sh
losos agents install
```

It asks about each tool it found, one at a time. Nothing is changed in a
tool you say no to. Then it asks for the AI agents token once.

Name the tools to do only those, and add `--yes` to skip the questions:

```sh
losos agents install claude-code zed --yes
```

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

The companion has a GPG key of its own, made on your computer. Your own key
signs once, when you set it up, to say that this key may read this one box
with this one token. The box accepts the companion's key only together with
that AI agents token, and an AI agents token only for reading files,
repositories and the box's status. So an AI tool never holds your own key,
and what it can reach is far less than what you can.

The key and the token are kept in your desktop's keychain, the same place
the terminal client keeps everything else.

## Take it out again

```sh
losos agents remove
```

It asks about each tool the companion is in and takes it out, and puts each
settings file back as it was. Once the companion is in no tool, the client
forgets its key and token. Revoke the AI agents token in **Settings →
Access tokens** as well.
