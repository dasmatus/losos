#!/usr/bin/env python3
"""channels.py against a fake GitHub API over a real git repository.

The fake keeps its refs and objects in a bare repository and answers the
calls channels.py makes with git plumbing, refusing a ref move that is not a
fast-forward as GitHub's ruleset does. The bot's clone fetches from that
repository like the workflow's checkout fetches from GitHub, so a cut, a
hotfix's cherry-pick, its blob-by-blob upload and its promotion run as they
would there.

    python3 .github/scripts/test_channels.py
"""

import base64
import datetime
import json
import os
import re
import subprocess
import sys
import tempfile
import unittest
import urllib.parse

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import channels  # noqa: E402

START = 1_760_000_000


def run(cwd, *args, env=None, data=None):
    full = dict(os.environ)
    full.update(env or {})
    out = subprocess.run(
        ["git", "-C", cwd, *args],
        check=True,
        capture_output=True,
        input=data,
        env=full,
    )
    return out.stdout.decode().strip()


class Clock:
    def __init__(self):
        self.now = START

    def tick(self):
        self.now += 100
        return self.now

    def env(self):
        stamp = f"{self.tick()} +0000"
        return {
            "GIT_AUTHOR_DATE": stamp,
            "GIT_COMMITTER_DATE": stamp,
            "GIT_AUTHOR_NAME": "t",
            "GIT_AUTHOR_EMAIL": "t@example.org",
            "GIT_COMMITTER_NAME": "t",
            "GIT_COMMITTER_EMAIL": "t@example.org",
        }


class FakeGitHub:
    """The repository-scoped slice of GitHub's REST API channels.py uses."""

    def __init__(self, bare, clock):
        self.bare = bare
        self.clock = clock
        self.prs = {}
        self.runs = []
        self.dispatched = []
        self.comments = []

    def git(self, *args, env=None, data=None):
        return run(self.bare, *args, env=env, data=data)

    def resolve(self, name):
        if re.fullmatch(r"[0-9a-f]{40}", name):
            return name
        return self.git("rev-parse", f"refs/heads/{name}")

    def ancestor(self, a, b):
        return (
            subprocess.run(
                ["git", "-C", self.bare, "merge-base", "--is-ancestor", a, b],
                capture_output=True,
            ).returncode
            == 0
        )

    def commit_doc(self, sha):
        raw = self.git("cat-file", "commit", sha)
        head, _, message = raw.partition("\n\n")
        tree = re.search(r"^tree (\w+)", head, re.M).group(1)
        parents = re.findall(r"^parent (\w+)", head, re.M)
        at = int(re.search(r"^committer .* (\d+) [+-]\d{4}$", head, re.M).group(1))
        date = datetime.datetime.fromtimestamp(at, datetime.timezone.utc).isoformat()
        return {
            "sha": sha,
            "tree": {"sha": tree},
            "parents": [{"sha": p} for p in parents],
            "message": message + "\n" if message else "",
            "committer": {"date": date.replace("+00:00", "Z")},
        }

    def call(self, method, path, body=None):
        url = urllib.parse.urlsplit(path)
        q = dict(urllib.parse.parse_qsl(url.query))
        p = url.path
        if method == "GET" and (m := re.fullmatch(r"git/ref/heads/(.+)", p)):
            try:
                return {"object": {"sha": self.git("rev-parse", "--verify", "-q", f"refs/heads/{m[1]}")}}
            except subprocess.CalledProcessError:
                raise channels.NotFound(p) from None
        if method == "POST" and p == "git/refs":
            if subprocess.run(["git", "-C", self.bare, "rev-parse", "--verify", "-q", body["ref"]], capture_output=True).returncode == 0:
                raise RuntimeError(f"422 {body['ref']} exists")
            self.git("update-ref", body["ref"], body["sha"])
            return {}
        if method == "PATCH" and (m := re.fullmatch(r"git/refs/heads/(.+)", p)):
            old = self.git("rev-parse", f"refs/heads/{m[1]}")
            assert body["force"] is False
            if not self.ancestor(old, body["sha"]):
                raise RuntimeError(f"422 {m[1]}: not a fast-forward")
            self.git("update-ref", f"refs/heads/{m[1]}", body["sha"], old)
            return {}
        if method == "DELETE" and (m := re.fullmatch(r"git/refs/heads/(.+)", p)):
            self.git("update-ref", "-d", f"refs/heads/{m[1]}")
            return None
        if method == "GET" and p == "git/matching-refs/heads/hotfix/":
            out = self.git("for-each-ref", "--format=%(refname) %(objectname)", "refs/heads/hotfix/")
            return [{"ref": r, "object": {"sha": s}} for r, s in (line.split() for line in out.splitlines())]
        if method == "GET" and (m := re.fullmatch(r"git/commits/(\w+)", p)):
            return self.commit_doc(m[1])
        if method == "POST" and p == "git/commits":
            args = ["commit-tree", body["tree"]]
            for parent in body["parents"]:
                args += ["-p", parent]
            sha = self.git(*args, env=self.clock.env(), data=body["message"].encode())
            return {"sha": sha}
        if method == "POST" and p == "git/blobs":
            data = base64.b64decode(body["content"])
            return {"sha": self.git("hash-object", "-w", "--stdin", data=data)}
        if method == "POST" and p == "git/trees":
            with tempfile.TemporaryDirectory() as d:
                env = {"GIT_INDEX_FILE": os.path.join(d, "index")}
                self.git("read-tree", body["base_tree"], env=env)
                lines = [
                    f"0 {'0' * 40}\t{e['path']}" if e["sha"] is None else f"{e['mode']} {e['sha']}\t{e['path']}"
                    for e in body["tree"]
                ]
                self.git("update-index", "--index-info", env=env, data=("\n".join(lines) + "\n").encode())
                return {"sha": self.git("write-tree", env=env)}
        if method == "GET" and (m := re.fullmatch(r"compare/(.+)\.\.\.(.+)", p)):
            a, b = self.resolve(m[1]), self.resolve(m[2])
            if a == b:
                return {"status": "identical"}
            if self.ancestor(a, b):
                return {"status": "ahead"}
            if self.ancestor(b, a):
                return {"status": "behind"}
            return {"status": "diverged"}
        if method == "GET" and (m := re.fullmatch(r"pulls/(\d+)", p)):
            return self.prs[int(m[1])]
        if method == "GET" and p == "commits":
            since = int(channels.parse_time(q["since"]))
            out = self.git("log", f"refs/heads/{q['sha']}", f"--since=@{since}", "--format=%H%x00%B%x01")
            return [
                {"sha": e.split("\0")[0].strip(), "commit": {"message": e.split("\0")[1]}}
                for e in out.split("\x01")
                if e.strip()
            ]
        if method == "GET" and p == "actions/workflows/ci.yml/runs":
            found = [
                r
                for r in reversed(self.runs)
                if r["head_branch"] == q["branch"]
                and ("head_sha" not in q or r["head_sha"] == q["head_sha"])
                and ("status" not in q or r["conclusion"] == q["status"])
            ]
            return {"workflow_runs": found}
        if method == "GET" and (m := re.fullmatch(r"actions/runs/(\d+)/jobs", p)):
            r = next(r for r in self.runs if r["id"] == int(m[1]))
            return {"jobs": r["jobs"]}
        if method == "POST" and p == "actions/workflows/ci.yml/dispatches":
            self.dispatched.append(body["ref"])
            return None
        if method == "POST" and (m := re.fullmatch(r"issues/(\d+)/comments", p)):
            self.comments.append((int(m[1]), body["body"]))
            return {}
        raise AssertionError(f"unexpected call {method} {path}")

    # ── Test helpers ────────────────────────────────────────────────────────

    def ci(self, branch, sha, conclusion="success", cache=True, status="completed", event="push"):
        job = {"name": channels.CACHE_JOB, "conclusion": "success" if cache else "skipped"}
        self.runs.append(
            {
                "id": len(self.runs) + 1,
                "head_branch": branch,
                "head_sha": sha,
                "event": event,
                "status": status,
                "conclusion": conclusion if status == "completed" else None,
                "html_url": f"https://example.org/runs/{len(self.runs) + 1}",
                "jobs": [job],
            }
        )


class Repo:
    """A work tree that pushes to the fake's bare repository, standing in
    for whoever merges pull requests into main."""

    def __init__(self, root, clock):
        self.clock = clock
        self.bare = os.path.join(root, "github.git")
        self.work = os.path.join(root, "work")
        run(root, "init", "-q", "--bare", "-b", "main", self.bare)
        run(root, "init", "-q", "-b", "main", self.work)
        run(self.work, "remote", "add", "origin", self.bare)

    def commit(self, subject, files):
        for name, text in files.items():
            path = os.path.join(self.work, name)
            if text is None:
                os.remove(path)
            else:
                with open(path, "w") as f:
                    f.write(text)
        run(self.work, "add", "-A")
        run(self.work, "commit", "-q", "-m", subject, env=self.clock.env())
        run(self.work, "push", "-q", "origin", "main")
        return run(self.work, "rev-parse", "HEAD")

    def show(self, ref, name):
        return run(self.bare, "show", f"{ref}:{name}")

    def message(self, ref):
        return run(self.bare, "log", "-1", "--format=%B", ref)


class Pure(unittest.TestCase):
    def test_due(self):
        d = lambda *a: channels.due(datetime.datetime(*a, tzinfo=datetime.timezone.utc))
        self.assertEqual(d(2026, 10, 28), ["daily"])
        self.assertEqual(d(2026, 11, 2), ["daily", "weekly"])  # a Monday
        self.assertEqual(d(2026, 12, 1), ["daily", "monthly"])
        self.assertEqual(d(2027, 2, 1), ["daily", "weekly", "monthly"])

    def test_base_of(self):
        self.assertEqual(channels.base_of(7, "Merge pull request #1"), 7)
        msg = channels.cut_message("weekly", "a" * 40, 1234)
        self.assertEqual(channels.base_of(7, msg), 1234)
        fix = channels.hotfix_message("weekly", 9, "Fix", "b" * 40, channels.base_line(msg, 7, "c" * 40))
        self.assertEqual(channels.base_of(99, fix), 1234)
        self.assertTrue(channels.carries_hotfix(fix, 9))
        self.assertFalse(channels.carries_hotfix(fix, 90))
        # A commit of main hands on its own time.
        self.assertEqual(channels.base_line("Merge", 55, "d" * 40), f"Channel-Base: {'d' * 40} 55")

    def test_plan_cut(self):
        head = {"sha": "h", "tree": "t1", "base": 10}
        self.assertEqual(channels.plan_cut(None, {"sha": "m"}, False), "create")
        self.assertEqual(channels.plan_cut(head, {"sha": "h", "tree": "t1", "base": 10}, True), "none")
        self.assertEqual(channels.plan_cut(head, {"sha": "m", "tree": "t1", "base": 20}, False), "none")
        self.assertEqual(channels.plan_cut(head, {"sha": "m", "tree": "t2", "base": 5}, False), "none")
        self.assertEqual(channels.plan_cut(head, {"sha": "m", "tree": "t2", "base": 20}, True), "fast-forward")
        self.assertEqual(channels.plan_cut(head, {"sha": "m", "tree": "t2", "base": 20}, False), "merge")


class Flow(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.clock = Clock()
        self.repo = Repo(self.tmp.name, self.clock)
        self.gh = FakeGitHub(self.repo.bare, self.clock)
        self.lines = []
        clone = os.path.join(self.tmp.name, "clone")
        self.c1 = self.repo.commit("Merge pull request #1", {"a.txt": "one\n", "b.txt": "tunnel v1\n", "c.txt": "c\n"})
        run(self.tmp.name, "clone", "-q", self.repo.bare, clone)
        self.bot = channels.Channels(self.gh, git_dir=clone, out=self.lines.append)

    def tearDown(self):
        self.tmp.cleanup()

    def head(self, channel):
        return self.gh.git("rev-parse", f"refs/heads/channel/{channel}")

    def tree(self, ref):
        return self.gh.git("rev-parse", f"{ref}^{{tree}}")

    def merged(self, number, title, sha, labels=("hotfix",)):
        self.gh.prs[number] = {
            "number": number,
            "title": title,
            "merged": True,
            "merged_at": datetime.datetime.fromtimestamp(self.clock.now, datetime.timezone.utc)
            .strftime("%Y-%m-%dT%H:%M:%SZ"),
            "merge_commit_sha": "f" * 40,  # rewritten away, as after history-scrub
            "labels": [{"name": n} for n in labels],
        }
        del sha

    def test_cuts_hotfixes_and_promotion(self):
        # Nothing has passed ci yet: no channel is made.
        self.bot.cut(list(channels.CHANNELS))
        self.assertIn("nothing to cut", self.lines[-1])
        # A run that passed but did not upload the cache does not count.
        self.gh.ci("main", self.c1, cache=False)
        self.bot.cut(list(channels.CHANNELS))
        self.assertIn("nothing to cut", self.lines[-1])

        self.gh.ci("main", self.c1)
        self.bot.cut(list(channels.CHANNELS))
        for ch in channels.CHANNELS:
            self.assertEqual(self.head(ch), self.c1)

        # Main moves on. The fix to b.txt is a hotfix; c3 is an ordinary
        # change, and c3 is red.
        c2 = self.repo.commit("Fix the tunnel (#5)", {"b.txt": "tunnel v2\n", "new.txt": "added\n", "c.txt": None})
        self.merged(5, "Fix the tunnel", c2)
        c3 = self.repo.commit("Merge pull request #6", {"a.txt": "two\n"})
        self.gh.ci("main", c3, conclusion="failure")

        # A pull request without the label is left alone.
        self.gh.prs[8] = dict(self.gh.prs[5], number=8, labels=[])
        self.bot.hotfix(8)
        self.assertEqual(self.gh.dispatched, [])

        self.bot.hotfix(5)
        self.assertEqual(
            self.gh.dispatched, ["hotfix/daily/5", "hotfix/weekly/5", "hotfix/monthly/5"]
        )
        for ch in channels.CHANNELS:
            ref = f"refs/heads/hotfix/{ch}/5"
            # The channel's tree plus exactly the fix, uploaded file by file.
            self.assertEqual(self.repo.show(ref, "b.txt"), "tunnel v2")
            self.assertEqual(self.repo.show(ref, "new.txt"), "added")
            self.assertEqual(self.repo.show(ref, "a.txt"), "one")
            self.assertNotIn("c.txt", self.gh.git("ls-tree", "--name-only", ref))
            message = self.repo.message(ref)
            self.assertIn(f"Channel-Base: {self.c1} ", message)
            self.assertIn("Hotfix: #5 ", message)
            self.assertEqual(self.head(ch), self.c1, "the channel waits for ci")
        self.assertIn("Hotfix queued for", self.gh.comments[-1][1])

        # Run again: nothing twice.
        self.bot.hotfix(5)
        self.assertEqual(len(self.gh.dispatched), 3)

        # ci on the candidates: daily passes, weekly fails, monthly runs.
        cand = {ch: self.gh.git("rev-parse", f"refs/heads/hotfix/{ch}/5") for ch in channels.CHANNELS}
        self.gh.ci("hotfix/daily/5", cand["daily"], event="workflow_dispatch")
        self.gh.ci("hotfix/weekly/5", cand["weekly"], conclusion="failure", event="workflow_dispatch")
        self.gh.ci("hotfix/monthly/5", cand["monthly"], status="in_progress", event="workflow_dispatch")
        self.bot.promote()
        self.assertEqual(self.head("daily"), cand["daily"])
        self.assertEqual(self.head("weekly"), self.c1)
        self.assertEqual(self.head("monthly"), self.c1)
        refs = self.gh.git("for-each-ref", "--format=%(refname)", "refs/heads/hotfix/")
        self.assertEqual(refs, "refs/heads/hotfix/monthly/5")
        said = [c for _, c in self.gh.comments]
        self.assertIn("Released on `channel/daily`.", said)
        self.assertTrue(any("CI failed on `hotfix/weekly/5`" in c for c in said))
        # Asked again, daily already carries it and monthly is still in ci;
        # weekly, whose ci failed, is tried once more.
        self.bot.hotfix(5)
        self.assertEqual(self.gh.dispatched[3:], ["hotfix/weekly/5"])

        # c3 passes. The daily cut cannot fast-forward over the hotfix, so it
        # is a merge commit whose tree is exactly main's.
        self.gh.ci("main", c3)
        self.bot.cut(["daily"])
        head = self.head("daily")
        self.assertEqual(self.tree(head), self.tree(c3))
        self.assertEqual(
            self.gh.git("rev-list", "--parents", "-n", "1", head).split()[1:], [cand["daily"], c3]
        )
        self.assertIn(f"Channel-Base: {c3} ", self.repo.message(head))
        # Again: nothing to do.
        self.bot.cut(["daily"])
        self.assertEqual(self.head("daily"), head)
        self.assertEqual(self.lines[-1], "channel/daily: none")

        # The weekly cut fast-forwards: it carries no hotfix.
        self.bot.cut(["weekly"])
        self.assertEqual(self.head("weekly"), c3)

        # Monthly moves while its candidate runs; the candidate's ci then
        # passes, and the channel already holds the fix through main.
        self.bot.cut(["monthly"])
        self.assertEqual(self.head("monthly"), c3)
        for r in self.gh.runs:
            if r["head_branch"] == "hotfix/monthly/5":
                r.update(status="completed", conclusion="success")
        retry = self.gh.git("rev-parse", "refs/heads/hotfix/weekly/5")
        self.gh.ci("hotfix/weekly/5", retry, event="workflow_dispatch")
        self.bot.promote()
        self.assertEqual(self.head("monthly"), c3)
        self.assertEqual(self.head("weekly"), c3)
        self.assertIn("hotfix/monthly/5: channel moved; already there", self.lines)
        self.assertIn("hotfix/weekly/5: channel moved; already there", self.lines)
        self.assertEqual(self.gh.git("for-each-ref", "refs/heads/hotfix/"), "")

    def test_a_hotfix_that_does_not_apply_is_reported(self):
        self.gh.ci("main", self.c1)
        self.bot.cut(["monthly"])
        self.repo.commit("Merge pull request #6", {"a.txt": "two\n"})
        c4 = self.repo.commit("Merge pull request #7", {"a.txt": "three\n"})
        self.merged(7, "Change a", c4)
        self.bot.hotfix(7)
        self.assertEqual(self.gh.dispatched, [])
        self.assertIn("does not apply cleanly to `channel/monthly`", self.gh.comments[-1][1])
        self.assertEqual(self.head("monthly"), self.c1)

    def test_a_lost_dispatch_is_started_again(self):
        self.gh.ci("main", self.c1)
        self.bot.cut(["daily"])
        c2 = self.repo.commit("Fix (#9)", {"b.txt": "v3\n"})
        self.merged(9, "Fix", c2)
        self.bot.hotfix(9)
        self.assertEqual(self.gh.dispatched, ["hotfix/daily/9"])
        later = datetime.datetime.fromtimestamp(self.clock.now + 3600, datetime.timezone.utc)
        self.bot.promote(now=later)
        self.assertEqual(self.gh.dispatched, ["hotfix/daily/9", "hotfix/daily/9"])


if __name__ == "__main__":
    unittest.main()
