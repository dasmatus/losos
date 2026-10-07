# Body of git-filter-repo's --message-callback, read by scrub-history.sh and
# handed over as one string. filter-repo pastes it into a function that gets
# `message` (the commit or tag message, bytes) and uses the return value as
# the new message. Keep it a function *body*: no def, no top-level return
# outside the flow below, imports are fine.
#
# The callback must return `message` untouched when nothing matched. Every
# byte that changes moves the commit's hash and every descendant's, so even
# the tidy-up at the end only runs on a message a rule actually fired on.
import re

original = message.decode("utf-8", "surrogateescape")
text = original

# Whole lines whose only job is to credit the tool or link a session.
# Co-authored-by lines are kept for people and for the other bots; only the
# ones naming Claude or an anthropic.com address go.
for pattern in (
    r"^co-authored-by:(?=[^\n]*(?:\bclaude\b|anthropic\.com))[^\n]*\n?",
    r"^claude-session:[^\n]*\n?",
    r"^🤖 generated with[^\n]*\n?",
    r"^_?generated (?:with|by) \[claude code\][^\n]*\n?",
    r"^<!-- ccr-projects-attribution:[^\n]*\n?",
    r"^_requested by [^\n]*\[project thread\][^\n]*\n?",
    r"^https://claude\.ai/code/\S*[ \t]*\n?",
):
    text = re.sub(pattern, "", text, flags=re.IGNORECASE | re.MULTILINE)

# A session link inside a sentence, with the parentheses a Markdown link or
# a prose aside puts around it.
text = re.sub(r"[ \t]*\(?https://claude\.ai/code/[^\s)]+\)?", "", text)

# GitHub's merge subjects name the head branch, and the sessions push
# branches called claude/<something>. Keep the PR number and the merged-in
# branch, drop the head.
text = re.sub(
    r"^(Merge pull request #\d+) from \S+/claude/\S+$", r"\1", text, flags=re.MULTILINE
)
text = re.sub(
    r"^(Merge (?:remote-tracking )?branch '[^']+') into claude/\S+$",
    r"\1",
    text,
    flags=re.MULTILINE,
)

if text == original:
    return message

# Removing trailer lines leaves blank runs behind; fold them and end on one
# newline like git itself writes.
text = re.sub(r"\n{3,}", "\n\n", text)
text = text.rstrip() + "\n"
return text.encode("utf-8", "surrogateescape")
