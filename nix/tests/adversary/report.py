"""Validates the adversary's finding files and prints the run summary.

The adversary writes one Markdown file per finding into
`adversary/findings/`, each fronted by a fenced YAML block with the schema
docs/superpowers/specs/2026-11-09-adversary-testing-design.md fixes
(boundary, claim, severity, status, state). This is the check between the
agent's prose and the pull request: untidy files fail the run, confirmed
findings must carry their repro directory, and the summary is what the
workflow works into the PR body from `report.json`.

    python3 report.py adversary/findings report.json

A confirmed finding names its repro as `adversary/tooling/<slug>/`, where
<slug> is the file name's stem. A suspicion needs no repro and never opens
a PR on its own; the workflow counts both, PRs the confirmed ones.
"""

import json
import re
import sys
from pathlib import Path

BOUNDARIES = {
    "claim window",
    "lan-only guard",
    "section authorization",
    "token and identity",
    "mesh shared isolation",
    "the edge",
    "shipped stack",
    "brute-force lane",
    "other",
}
SEVERITIES = {"critical", "high", "medium", "low"}
STATUSES = {"confirmed", "suspicion"}
STATES = {
    "fresh-claim",
    "claimed-viewer",
    "claimed-no-creds",
    "edge",
    "mesh",
    "stack",
    "source-review",
}
REQUIRED = ["boundary", "claim", "severity", "status", "state"]


def frontmatter(path):
    """Returns the fenced YAML block as a flat dict, or raises ValueError.

    The schema is flat `key: value` lines and nothing else, so tiny parsing
    beats a dependency the runner may not have.
    """
    text = path.read_text()
    m = re.match(r"^---\n(.*?)\n(---|\.\.\.)\s*\n", text, re.DOTALL)
    if not m:
        raise ValueError("no fenced frontmatter block at the top")
    fields = {}
    for i, line in enumerate(m.group(1).splitlines()):
        if not line.strip():
            continue
        kv = re.match(r'^([a-z-]+): *(.+)$', line.rstrip())
        if not kv:
            raise ValueError(f"frontmatter line {i + 1} is not `key: value`: {line!r}")
        if kv.group(1) in fields:
            raise ValueError(f"repeated key {kv.group(1)!r}")
        fields[kv.group(1)] = kv.group(2).strip().strip('"')
    return fields


def check(path):
    """Validates one finding file; returns it as a summary dict."""
    fields = frontmatter(path)
    missing = [k for k in REQUIRED if not fields.get(k)]
    if missing:
        raise ValueError(f"missing {', '.join(missing)}")
    if fields["boundary"] not in BOUNDARIES:
        raise ValueError(f"boundary {fields['boundary']!r} is not one of {sorted(BOUNDARIES)}")
    if fields["severity"] not in SEVERITIES:
        raise ValueError(f"severity {fields['severity']!r} is not one of {sorted(SEVERITIES)}")
    if fields["status"] not in STATUSES:
        raise ValueError(f"status {fields['status']!r} is not one of {sorted(STATUSES)}")
    if fields["state"] not in STATES:
        raise ValueError(f"state {fields['state']!r} is not one of {sorted(STATES)}")
    if fields["status"] == "confirmed":
        slug = path.name.removesuffix(".md")
        repro = Path(path).parent.parent / "tooling" / slug
        if not repro.is_dir() or not any(repro.iterdir()):
            raise ValueError(f"confirmed, but the repro directory {repro} is missing or empty")
    return {k: fields[k] for k in REQUIRED} | {"file": str(path.relative_to(path.parent.parent))}


def main():
    findings, out_json = Path(sys.argv[1]), Path(sys.argv[2])
    files = sorted(findings.glob("*.md"))
    rows, errors = [], []
    for f in files:
        try:
            rows.append(check(f))
        except ValueError as e:
            errors.append(f"{f.name}: {e}")
    for e in errors:
        print(f"error: {e}")
    out_json.write_text(json.dumps({"findings": rows, "errors": errors}, indent=2) + "\n")
    confirmed = [r for r in rows if r["status"] == "confirmed"]
    suspicions = [r for r in rows if r["status"] == "suspicion"]
    print(f"{len(rows)} finding file(s): {len(confirmed)} confirmed, {len(suspicions)} suspicion, {len(files) - len(rows)} invalid")
    for r in rows:
        print(f"- {r['file']}: {r['boundary']} / {r['severity']} / {r['status']} / {r['state']}")
    if errors:
        sys.exit(1)


if __name__ == "__main__":
    main()
