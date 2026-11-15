#!/usr/bin/env python3
"""The optional agent workflows stay out of core CI's way.

opencode-fix, tinycode-autofix and opencode-autofix are conveniences on top
of `ci`; none of them may decide whether `ci` (and its required `result`
check) is green. This checks the wiring that guarantees it, and runs the
agents' own shell steps against fakes:

  * ci.yml never waits on, triggers from or names an agent workflow, and
    `result` needs no agent job; no agent concurrency group shares `ci-`.
  * a model that cannot answer (both free models out of quota) ends the
    agent step green with ran=false, and every later step is gated on it.
  * agent edits under .github/workflows/ are squashed out before any push:
    GITHUB_TOKEN may not push them, and one such commit refuses the push.

Standard library only; text parsing of the workflows, no PyYAML needed.

    python3 .github/scripts/test_agent_isolation.py
"""

import os
import re
import subprocess
import tempfile
import unittest

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
WF = os.path.join(ROOT, ".github", "workflows")
AGENTS = {
    "opencode-fix.yml": ("opencode", "ci-fix"),
    "tinycode-autofix.yml": ("tinycode", "ci-fix"),
    "opencode-autofix.yml": ("tinycode", "autofix"),
}
AGENT_STEP = {
    "opencode-fix.yml": "run OpenCode (primary, then fallback free model)",
    "tinycode-autofix.yml": "run tinycode (primary, then fallback free model)",
    "opencode-autofix.yml": "tinycode works against the description (primary, then fallback)",
}
STOCK_STEP = {
    "opencode-fix.yml": "take stock of the changes",
    "tinycode-autofix.yml": "take stock of the changes",
    "opencode-autofix.yml": "Commit, rebase, and take stock",
}


def read(name):
    with open(os.path.join(WF, name), encoding="utf-8") as f:
        return f.read()


def job_block(text, job):
    """The lines of one job under `jobs:`, up to the next job."""
    m = re.search(rf"^  {re.escape(job)}:\n(.*?)(?=^  [A-Za-z0-9_-]+:\n|\Z)", text, re.M | re.S)
    assert m, f"job {job} not found"
    return m.group(1)


def steps(text, job):
    """(name, if, run) of each named step of a job, by indentation."""
    out = []
    block = job_block(text, job)
    for m in re.finditer(r"^      - (?:name: (.+?)\n|uses: .+?\n)(.*?)(?=^      - |\Z)", block, re.M | re.S):
        name, body = m.group(1), m.group(2)
        cond = re.search(r"^        if: (.+)$", body, re.M)
        run = re.search(r"^        run: \|\n((?:^(?:          .*|)\n)+)", body, re.M)
        script = None
        if run:
            script = "\n".join(l[10:] for l in run.group(1).split("\n"))
        out.append((name, cond.group(1) if cond else None, script))
    return out


def step(text, job, name):
    for n, cond, script in steps(text, job):
        if n == name:
            return cond, script
    raise AssertionError(f"step {name!r} not found in {job}")


def git(cwd, *args):
    env = dict(os.environ, GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@example.org",
               GIT_COMMITTER_NAME="t", GIT_COMMITTER_EMAIL="t@example.org")
    return subprocess.run(["git", "-C", cwd, *args], check=True, capture_output=True,
                          text=True, env=env).stdout.strip()


def outputs(path):
    with open(path, encoding="utf-8") as f:
        return dict(l.split("=", 1) for l in f.read().splitlines() if "=" in l)


class Wiring(unittest.TestCase):
    def test_ci_does_not_reference_the_agents(self):
        ci = read("ci.yml")
        self.assertNotIn("workflow_run", ci)
        for word in ("opencode", "tinycode"):
            self.assertNotIn(word, ci.lower(), f"ci.yml names {word}")

    def test_result_needs_no_agent_job(self):
        block = job_block(read("ci.yml"), "result")
        needs = re.search(r"needs:\n((?:\s+- .+\n)+)", block).group(1)
        names = re.findall(r"- (\S+)", needs)
        self.assertTrue(names)
        for n in names:
            self.assertNotRegex(n, r"opencode|tinycode|autofix|ci-fix|merge-main")

    def test_agent_concurrency_never_shares_ci_group(self):
        for name in AGENTS:
            for g in re.findall(r"group:\s*(\S+)", read(name)):
                self.assertFalse(g.startswith("ci-"), f"{name}: {g}")

    def test_no_model_does_not_exit_1_and_gates_the_rest(self):
        for name, (_, job) in AGENTS.items():
            text = read(name)
            _, script = step(text, job, AGENT_STEP[name])
            branch = script.split('if [ -z "$used" ]; then', 1)[1].split("\nfi\n", 1)[0]
            self.assertNotIn("exit 1", branch, name)
            self.assertIn("ran=false", branch, name)
            self.assertIn("exit 0", branch, name)
            after = False
            for n, cond, _ in steps(text, job):
                if n == AGENT_STEP[name]:
                    after = True
                    continue
                if after:
                    self.assertIsNotNone(cond, f"{name}: step {n!r} ungated")
                    self.assertTrue("steps.agent.outputs.ran == 'true'" in cond
                                    or "steps.changes.outputs" in cond, f"{name}: {n!r}: {cond}")

    def test_every_push_path_guards_workflow_files(self):
        for name, (_, job) in AGENTS.items():
            _, script = step(read(name), job, STOCK_STEP[name])
            self.assertIn("-- .github/workflows/", script, name)
            self.assertIn("git restore --source=", script, name)


class AgentStepRuns(unittest.TestCase):
    """The agent step itself, with a fake CLI that is out of quota."""

    def run_agent(self, name):
        _, job = AGENTS[name]
        _, script = step(read(name), job, AGENT_STEP[name])
        with tempfile.TemporaryDirectory() as d:
            fake = os.path.join(d, AGENTS[name][0])
            with open(fake, "w") as f:
                f.write("#!/bin/sh\necho 'Error: usage limit reached (weekly, resets in 3d)' >&2\nexit 1\n")
            os.chmod(fake, 0o755)
            out, summ = os.path.join(d, "out"), os.path.join(d, "summary")
            open(out, "w").close()
            open(summ, "w").close()
            env = dict(os.environ, PATH=f"{d}:{os.environ['PATH']}", GITHUB_OUTPUT=out,
                       GITHUB_STEP_SUMMARY=summ, GITHUB_WORKSPACE=d,
                       OC_MODEL="a", OC_FALLBACK_MODEL="b",
                       TINYCODE_MODEL="a", TINYCODE_FALLBACK_MODEL="b")
            r = subprocess.run(["bash", "-e", "-c", script], env=env, cwd=d,
                               capture_output=True, text=True)
            with open(summ) as f:
                summary = f.read()
            return r, outputs(out), summary

    def test_quota_exhaustion_is_a_warning_not_a_failure(self):
        for name in AGENTS:
            r, out, summary = self.run_agent(name)
            self.assertEqual(r.returncode, 0, f"{name}: {r.stdout}{r.stderr}")
            self.assertEqual(out.get("ran"), "false", name)
            self.assertIn("::warning::", r.stdout, name)
            self.assertNotIn("::error::", r.stdout, name)
            self.assertIn("usage limit", summary, name)


class WorkflowGuardRuns(unittest.TestCase):
    """The take-stock step on a real repository with agent commits."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        d = self.tmp.name
        self.origin = os.path.join(d, "origin.git")
        self.repo = os.path.join(d, "repo")
        git(d, "init", "-q", "--bare", "-b", "main", self.origin)
        git(d, "clone", "-q", self.origin, self.repo)
        os.makedirs(os.path.join(self.repo, ".github", "workflows"))
        self.write(".github/workflows/ci.yml", "name: ci\n")
        self.write("src.txt", "broken\n")
        git(self.repo, "add", "-A")
        git(self.repo, "commit", "-qm", "base")
        git(self.repo, "push", "-q", "origin", "HEAD:main")
        self.base = git(self.repo, "rev-parse", "HEAD")

    def tearDown(self):
        self.tmp.cleanup()

    def write(self, path, text):
        with open(os.path.join(self.repo, path), "w") as f:
            f.write(text)

    def agent(self, fix_src):
        # An agent commit touching a workflow, then an uncommitted new one.
        self.write(".github/workflows/ci.yml", "name: ci\n# agent was here\n")
        if fix_src:
            self.write("src.txt", "fixed\n")
        git(self.repo, "commit", "-qam", "fix: agent")
        self.write(".github/workflows/new.yml", "name: new\n")

    def run_stock(self, name):
        _, job = AGENTS[name]
        _, script = step(read(name), job, STOCK_STEP[name])
        out = os.path.join(self.tmp.name, "out")
        summ = os.path.join(self.tmp.name, "summary")
        open(out, "w").close()
        open(summ, "w").close()
        env = dict(os.environ, GITHUB_OUTPUT=out, GITHUB_STEP_SUMMARY=summ,
                   HEAD_SHA=self.base, PRE_SHA=self.base, BRANCH="main",
                   GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@example.org",
                   GIT_COMMITTER_NAME="t", GIT_COMMITTER_EMAIL="t@example.org")
        r = subprocess.run(["bash", "-e", "-c", script], env=env, cwd=self.repo,
                           capture_output=True, text=True)
        self.assertEqual(r.returncode, 0, f"{name}: {r.stdout}{r.stderr}")
        return r, outputs(out)

    def pushed_paths(self):
        return git(self.repo, "log", "--name-only", "--format=", f"{self.base}..HEAD").split()

    def test_workflow_edits_are_dropped_and_the_fix_kept(self):
        for name in AGENTS:
            with self.subTest(name):
                git(self.repo, "reset", "-q", "--hard", self.base)
                git(self.repo, "clean", "-qfd")
                self.agent(fix_src=True)
                r, out = self.run_stock(name)
                self.assertEqual(out.get("changed"), "true")
                self.assertIn(".github/workflows/ci.yml", out.get("dropped", ""))
                self.assertIn(".github/workflows/new.yml", out.get("dropped", ""))
                self.assertIn("workflow edits dropped", r.stdout)
                paths = self.pushed_paths()
                self.assertIn("src.txt", paths)
                self.assertFalse([p for p in paths if p.startswith(".github/workflows/")], paths)
                self.assertEqual(git(self.repo, "status", "--porcelain"), "")

    def test_only_workflow_edits_push_nothing(self):
        for name in AGENTS:
            with self.subTest(name):
                git(self.repo, "reset", "-q", "--hard", self.base)
                git(self.repo, "clean", "-qfd")
                self.agent(fix_src=False)
                r, out = self.run_stock(name)
                self.assertEqual(out.get("changed"), "false")
                self.assertIn("nothing is left to push", r.stdout)
                self.assertEqual(self.pushed_paths(), [])


if __name__ == "__main__":
    unittest.main()
