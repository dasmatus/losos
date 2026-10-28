#!/usr/bin/env python3
"""The release channels a box follows: cuts, hotfixes and their promotion.

A box's nightly update takes the newest commit of one branch
(`losos.configRepo.upstream.branch`, picked by `losos.updates.channel`):

    channel/daily     moved every night
    channel/weekly    moved on Mondays
    channel/monthly   moved on the 1st

Each move, a *cut*, takes the newest commit of main whose `ci` run passed,
cache upload included, so a box never builds a commit that failed its checks
and never compiles what CI already built. A cut fast-forwards the channel
when it can. After a hotfix it cannot, and the cut is then a merge commit
whose tree is exactly main's, with the channel as first parent. Nothing here
ever force-pushes: the repository refuses non-fast-forward pushes, and a box
copies the tree, not the history, so it never sees the difference.

A merged pull request labelled `hotfix` reaches every channel the same day.
`hotfix` cherry-picks its merge commit onto each channel that lacks it and
puts the result on a candidate branch, `hotfix/<channel>/<number>`, then
starts `ci` there. `promote` fast-forwards the channel to the candidate once
that run passed, cache upload included, and deletes the candidate.

Every commit this makes names the main commit its channel was cut from:

    Channel-Base: <sha> <unix seconds of that commit>

lososd reads it (backend/src/config_repo.rs, `base_of`) so that a box which
moves to a slower channel keeps its version until the channel passes it,
rather than running its data under older code.

Every write goes through GitHub's API, not `git push`. The commits are then
GitHub's, signed, as the repository's ruleset wants, and a push made with
the workflow's token would start no `ci` run anyway. The only local git is
the cherry-pick itself, in the workflow's full clone, whose resulting tree is
uploaded blob by blob.

    channels.py cut [daily weekly monthly | all]   the channels due now, or these
    channels.py hotfix NUMBER                      put a merged hotfix on every channel
    channels.py promote                            move channels to passed candidates

Environment: GITHUB_TOKEN, GITHUB_REPOSITORY, and GITHUB_API_URL (GitHub's by
default). `test_channels.py` beside this runs all three against a fake API
over a real repository.
"""

import base64
import datetime
import json
import os
import re
import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request

CHANNELS = ("daily", "weekly", "monthly")
LABEL = "hotfix"
CI_WORKFLOW = "ci.yml"
# The job in ci.yml that uploads the built paths. A channel only moves to a
# commit this passed on, or every box on it compiles overnight.
CACHE_JOB = "nix: publish built paths to GHCR"
# How long a candidate may wait for its `ci` run to show up before it is
# started again: the dispatch can be lost, and a run listing lags a little.
REDISPATCH_AFTER = datetime.timedelta(minutes=30)

BASE_TRAILER = "Channel-Base:"
HOTFIX_TRAILER = "Hotfix:"


def branch(channel):
    return f"channel/{channel}"


def candidate(channel, number):
    return f"hotfix/{channel}/{number}"


# ── Pure parts ──────────────────────────────────────────────────────────────


def due(now):
    """The channels a cut at `now` (UTC) moves."""
    out = ["daily"]
    if now.weekday() == 0:
        out.append("weekly")
    if now.day == 1:
        out.append("monthly")
    return out


def base_of(committed_at, message):
    """When the main a channel commit was cut from was made: its last
    `Channel-Base:` trailer, else the commit's own time (a commit of main)."""
    for line in reversed(message.splitlines()):
        line = line.strip()
        if not line.startswith(BASE_TRAILER):
            continue
        parts = line[len(BASE_TRAILER):].split()
        if len(parts) >= 2 and parts[1].isdigit():
            return int(parts[1])
    return committed_at


def base_line(message, committed_at, sha):
    """The `Channel-Base:` trailer a commit on top of this one carries."""
    for line in reversed(message.splitlines()):
        if line.strip().startswith(BASE_TRAILER):
            return line.strip()
    return f"{BASE_TRAILER} {sha} {committed_at}"


def cut_message(channel, sha, committed_at):
    return (
        f"Cut {branch(channel)} from main {sha[:10]}\n\n"
        f"{BASE_TRAILER} {sha} {committed_at}\n"
    )


def hotfix_message(channel, number, title, merge_sha, base):
    return (
        f"Hotfix #{number} on {branch(channel)}: {title}\n\n"
        f"{base}\n"
        f"{HOTFIX_TRAILER} #{number} {merge_sha}\n"
    )


def carries_hotfix(message, number):
    return any(
        line.strip().startswith(f"{HOTFIX_TRAILER} #{number} ")
        for line in message.splitlines()
    )


def plan_cut(head, target, head_is_ancestor):
    """What a cut of a channel at `head` (None: no channel yet) to the main
    commit `target` does: create, none, fast-forward or merge.

    `head` and `target` are dicts with sha, tree and base (unix seconds of
    the main they stand for); `head_is_ancestor` says whether the channel's
    head is in target's history."""
    if head is None:
        return "create"
    if head["sha"] == target["sha"] or head["tree"] == target["tree"]:
        return "none"
    # Never back: the channel already stands for this main or a newer one.
    if target["base"] <= head["base"]:
        return "none"
    if head_is_ancestor:
        return "fast-forward"
    return "merge"


def parse_time(stamp):
    return int(datetime.datetime.fromisoformat(stamp.replace("Z", "+00:00")).timestamp())


# ── GitHub ──────────────────────────────────────────────────────────────────


class NotFound(Exception):
    pass


class GitHub:
    def __init__(self, token, repo, url="https://api.github.com"):
        self.token = token
        self.repo = repo
        self.url = url.rstrip("/")

    def call(self, method, path, body=None):
        """One request; `path` is relative to the repository (`git/refs`)
        unless it starts with `/`. A 404 is NotFound, a 204 is None."""
        full = path if path.startswith("/") else f"/repos/{self.repo}/{path}"
        data = None if body is None else json.dumps(body).encode()
        req = urllib.request.Request(self.url + full, data=data, method=method)
        req.add_header("Authorization", f"Bearer {self.token}")
        req.add_header("Accept", "application/vnd.github+json")
        req.add_header("X-GitHub-Api-Version", "2022-11-28")
        if data is not None:
            req.add_header("Content-Type", "application/json")
        try:
            with urllib.request.urlopen(req) as res:
                text = res.read()
        except urllib.error.HTTPError as err:
            if err.code == 404:
                raise NotFound(full) from None
            detail = err.read().decode(errors="replace")
            raise RuntimeError(f"{method} {full}: {err.code} {detail}") from None
        return json.loads(text) if text else None


class Channels:
    def __init__(self, gh, git_dir=".", out=print):
        self.gh = gh
        self.git_dir = git_dir
        self.say = out

    # ── Reading ─────────────────────────────────────────────────────────────

    def commit(self, sha):
        c = self.gh.call("GET", f"git/commits/{sha}")
        at = parse_time(c["committer"]["date"])
        return {
            "sha": c["sha"],
            "tree": c["tree"]["sha"],
            "parents": [p["sha"] for p in c["parents"]],
            "message": c["message"],
            "at": at,
            "base": base_of(at, c["message"]),
        }

    def ref(self, name):
        try:
            return self.gh.call("GET", f"git/ref/heads/{name}")["object"]["sha"]
        except NotFound:
            return None

    def head(self, channel):
        sha = self.ref(branch(channel))
        return None if sha is None else self.commit(sha)

    def is_ancestor(self, a, b):
        """Whether commit `a` is in the history of `b`."""
        if a == b:
            return True
        status = self.gh.call("GET", f"compare/{a}...{b}")["status"]
        return status in ("ahead", "identical")

    def cache_passed(self, run):
        jobs = self.gh.call("GET", f"actions/runs/{run['id']}/jobs?filter=latest&per_page=100")
        return any(
            j["name"] == CACHE_JOB and j["conclusion"] == "success" for j in jobs["jobs"]
        )

    def green_main(self):
        """The newest commit of main whose `ci` run passed, cache upload
        included. A run on a commit main no longer holds (history-scrub.yml
        rewrote it) does not count."""
        runs = self.gh.call(
            "GET",
            f"actions/workflows/{CI_WORKFLOW}/runs?branch=main&status=success&per_page=50",
        )["workflow_runs"]
        for run in runs:
            if run["event"] not in ("push", "schedule", "workflow_dispatch"):
                continue
            if not self.is_ancestor(run["head_sha"], "main"):
                continue
            if not self.cache_passed(run):
                continue
            return self.commit(run["head_sha"])
        return None

    def landed(self, pr):
        """The commit of main that brought pull request `pr` in. The pull
        request's own `merge_commit_sha` stops being on main when
        history-scrub.yml rewrites the merge, so it is looked up by the
        subject GitHub gave it, which the rewrite keeps: "Merge pull request
        #N" for a merge, "… (#N)" for a squash."""
        number = pr["number"]
        merged = parse_time(pr["merged_at"])
        since = datetime.datetime.fromtimestamp(merged - 3600, datetime.timezone.utc)
        commits = self.gh.call(
            "GET", f"commits?sha=main&since={since.strftime('%Y-%m-%dT%H:%M:%SZ')}&per_page=100"
        )
        subject = re.compile(rf"^Merge pull request #{number}\b|\(#{number}\)$")
        for c in commits:
            first = c["commit"]["message"].splitlines()[0] if c["commit"]["message"] else ""
            if subject.search(first):
                return c["sha"]
        return pr["merge_commit_sha"]

    def carried(self, channel, number, since):
        """Whether the channel already carries hotfix `number`, found by its
        trailer among the commits since the pull request merged."""
        commits = self.gh.call(
            "GET",
            f"commits?sha={branch(channel)}&since={urllib.parse.quote(since)}&per_page=100",
        )
        return any(carries_hotfix(c["commit"]["message"], number) for c in commits)

    # ── Writing ─────────────────────────────────────────────────────────────

    def move(self, name, sha):
        # force: false. GitHub refuses anything but a fast-forward.
        self.gh.call("PATCH", f"git/refs/heads/{name}", {"sha": sha, "force": False})

    def make_ref(self, name, sha):
        self.gh.call("POST", "git/refs", {"ref": f"refs/heads/{name}", "sha": sha})

    def drop_ref(self, name):
        try:
            self.gh.call("DELETE", f"git/refs/heads/{name}")
        except NotFound:
            pass

    def comment(self, number, text):
        self.gh.call("POST", f"issues/{number}/comments", {"body": text})

    def start_ci(self, name):
        self.gh.call(
            "POST",
            f"actions/workflows/{CI_WORKFLOW}/dispatches",
            {"ref": name, "inputs": {"release_channel": "auto"}},
        )

    # ── cut ─────────────────────────────────────────────────────────────────

    def cut(self, channels):
        target = self.green_main()
        if target is None:
            self.say("no commit of main has passed ci with its cache upload; nothing to cut")
            return
        self.say(f"newest passed main: {target['sha']}")
        for channel in channels:
            head = self.head(channel)
            ancestor = head is not None and self.is_ancestor(head["sha"], target["sha"])
            action = plan_cut(head, target, ancestor)
            name = branch(channel)
            if action == "create":
                self.make_ref(name, target["sha"])
            elif action == "fast-forward":
                self.move(name, target["sha"])
            elif action == "merge":
                made = self.gh.call(
                    "POST",
                    "git/commits",
                    {
                        "message": cut_message(channel, target["sha"], target["at"]),
                        "tree": target["tree"],
                        "parents": [head["sha"], target["sha"]],
                    },
                )
                self.move(name, made["sha"])
            self.say(f"{name}: {action}")

    # ── hotfix ──────────────────────────────────────────────────────────────

    def git(self, *args, check=True):
        # A name for git to be content with; nothing is committed locally.
        ident = {"name": "losos-channels", "email": "losos-channels@users.noreply.github.com"}
        env = dict(os.environ)
        for who in ("AUTHOR", "COMMITTER"):
            env[f"GIT_{who}_NAME"] = ident["name"]
            env[f"GIT_{who}_EMAIL"] = ident["email"]
        return subprocess.run(
            ["git", "-C", self.git_dir, *args],
            check=check,
            capture_output=True,
            text=True,
            env=env,
        )

    def pick(self, head, merge_sha):
        """Cherry-pick `merge_sha` onto `head` in the local clone: the new
        tree's sha, "conflict" when it does not apply, or "empty" when it
        changes nothing there."""
        self.git("checkout", "-q", "--detach", head["sha"])
        parents = self.git("rev-list", "--parents", "-n", "1", merge_sha).stdout.split()[1:]
        args = ["-m", "1"] if len(parents) > 1 else []
        picked = self.git("cherry-pick", "--no-commit", *args, merge_sha, check=False)
        tree = self.git("write-tree", check=False).stdout.strip() if picked.returncode == 0 else None
        self.git("cherry-pick", "--abort", check=False)
        self.git("reset", "-q", "--hard", head["sha"])
        if tree is None:
            return "conflict"
        return "empty" if tree == head["tree"] else tree

    def upload_tree(self, base_tree, tree):
        """Make `tree` on GitHub as `base_tree` plus the entries that differ,
        uploading each changed file. Returns GitHub's sha for it, which must
        be `tree`'s."""
        diff = self.git("diff-tree", "-r", "--no-renames", "-z", base_tree, tree).stdout
        fields = diff.split("\0")
        entries = []
        for meta, path in zip(fields[0::2], fields[1::2]):
            if not meta:
                continue
            _, new_mode, _, new_sha, status = meta.lstrip(":").split(" ")
            if status == "D":
                entries.append({"path": path, "mode": "100644", "type": "blob", "sha": None})
                continue
            if new_mode == "160000":
                entries.append({"path": path, "mode": new_mode, "type": "commit", "sha": new_sha})
                continue
            content = subprocess.run(
                ["git", "-C", self.git_dir, "cat-file", "blob", new_sha],
                check=True,
                capture_output=True,
            ).stdout
            blob = self.gh.call(
                "POST",
                "git/blobs",
                {"content": base64.b64encode(content).decode(), "encoding": "base64"},
            )
            if blob["sha"] != new_sha:
                raise RuntimeError(f"{path}: GitHub stored {blob['sha']}, git made {new_sha}")
            entries.append({"path": path, "mode": new_mode, "type": "blob", "sha": new_sha})
        made = self.gh.call("POST", "git/trees", {"base_tree": base_tree, "tree": entries})
        if made["sha"] != tree:
            raise RuntimeError(f"GitHub made tree {made['sha']}, the cherry-pick {tree}")
        return made["sha"]

    def hotfix(self, number, channels=CHANNELS):
        pr = self.gh.call("GET", f"pulls/{number}")
        labels = {label["name"] for label in pr.get("labels", [])}
        if not pr.get("merged") or LABEL not in labels:
            self.say(f"#{number} is not a merged pull request labelled {LABEL}; nothing to do")
            return
        merge_sha = self.landed(pr)
        queued, failed = [], []
        for channel in channels:
            outcome = self.hotfix_one(pr, merge_sha, channel)
            self.say(f"{branch(channel)}: {outcome}")
            if outcome == "queued":
                queued.append(channel)
            elif outcome == "conflict":
                failed.append(channel)
        if queued:
            names = ", ".join(f"`{branch(c)}`" for c in queued)
            self.comment(
                number,
                f"Hotfix queued for {names}. CI runs on each first, and the channel "
                "moves once it passes; boxes take it at their next nightly update.",
            )
        if failed:
            names = ", ".join(f"`{branch(c)}`" for c in failed)
            self.comment(
                number,
                f"This hotfix does not apply cleanly to {names}. Open a pull request "
                "against that branch with the change made to fit.",
            )

    def hotfix_one(self, pr, merge_sha, channel):
        number = pr["number"]
        head = self.head(channel)
        if head is None:
            return "no channel yet"
        if self.ref(candidate(channel, number)) is not None:
            return "waiting for ci"
        if self.is_ancestor(merge_sha, head["sha"]) or self.carried(channel, number, pr["merged_at"]):
            return "already there"
        # The workflow's clone may be older than main and the channel.
        self.git(
            "fetch", "-q", "origin",
            "+refs/heads/main:refs/remotes/origin/main",
            f"+refs/heads/{branch(channel)}:refs/remotes/origin/{branch(channel)}",
            check=False,
        )
        tree = self.pick(head, merge_sha)
        if tree == "conflict":
            return "conflict"
        if tree == "empty":
            return "already there"
        self.upload_tree(head["tree"], tree)
        made = self.gh.call(
            "POST",
            "git/commits",
            {
                "message": hotfix_message(
                    channel,
                    number,
                    pr["title"],
                    merge_sha,
                    base_line(head["message"], head["at"], head["sha"]),
                ),
                "tree": tree,
                "parents": [head["sha"]],
            },
        )
        name = candidate(channel, number)
        self.make_ref(name, made["sha"])
        self.start_ci(name)
        return "queued"

    # ── promote ─────────────────────────────────────────────────────────────

    def promote(self, now=None):
        now = now or datetime.datetime.now(datetime.timezone.utc)
        refs = self.gh.call("GET", "git/matching-refs/heads/hotfix/")
        for r in refs:
            m = re.fullmatch(r"refs/heads/hotfix/([a-z]+)/(\d+)", r["ref"])
            if not m or m.group(1) not in CHANNELS:
                continue
            channel, number = m.group(1), int(m.group(2))
            outcome = self.promote_one(channel, number, r["object"]["sha"], now)
            self.say(f"{candidate(channel, number)}: {outcome}")

    def promote_one(self, channel, number, sha, now):
        name = candidate(channel, number)
        runs = self.gh.call(
            "GET", f"actions/workflows/{CI_WORKFLOW}/runs?branch={name}&head_sha={sha}&per_page=5"
        )["workflow_runs"]
        made = self.commit(sha)
        if not runs:
            age = now - datetime.datetime.fromtimestamp(made["at"], datetime.timezone.utc)
            if age > REDISPATCH_AFTER:
                self.start_ci(name)
                return "ci started again"
            return "waiting for ci to start"
        run = runs[0]
        if run["status"] != "completed":
            return "ci running"
        if run["conclusion"] != "success" or not self.cache_passed(run):
            self.comment(
                number,
                f"CI failed on `{name}`, so `{branch(channel)}` does not take this hotfix: "
                f"{run['html_url']}",
            )
            self.drop_ref(name)
            return "ci failed"
        head = self.head(channel)
        if head is not None and head["sha"] == made["parents"][0]:
            self.move(branch(channel), sha)
            self.drop_ref(name)
            self.comment(number, f"Released on `{branch(channel)}`.")
            return "released"
        # The channel moved while CI ran: a cut (which brings main, and so
        # this fix) or another hotfix. Pick it again onto the new head.
        self.drop_ref(name)
        pr = self.gh.call("GET", f"pulls/{number}")
        return "channel moved; " + self.hotfix_one(pr, self.landed(pr), channel)


def main(argv):
    if len(argv) < 2 or argv[1] not in ("cut", "hotfix", "promote"):
        print(__doc__)
        return 2
    gh = GitHub(
        os.environ["GITHUB_TOKEN"],
        os.environ["GITHUB_REPOSITORY"],
        os.environ.get("GITHUB_API_URL", "https://api.github.com"),
    )
    ch = Channels(gh)
    task, rest = argv[1], argv[2:]
    if task == "cut":
        if rest == ["all"]:
            names = list(CHANNELS)
        elif rest:
            names = [n for n in rest if n in CHANNELS]
            if len(names) != len(rest):
                print(f"channels are {', '.join(CHANNELS)} or all", file=sys.stderr)
                return 2
        else:
            names = due(datetime.datetime.now(datetime.timezone.utc))
        ch.cut(names)
    elif task == "hotfix":
        if len(rest) != 1 or not rest[0].isdigit():
            print("hotfix NUMBER", file=sys.stderr)
            return 2
        ch.hotfix(int(rest[0]))
    else:
        ch.promote()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
