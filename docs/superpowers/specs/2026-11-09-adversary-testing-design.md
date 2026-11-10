# Automated adversary testing of LosOS main — design

Date: 2026-11-09
Status: v1 of the red-team lane ships with this design —
`.github/workflows/opencode-adversary.yml` runs the **core tier** on the
discovery check's box, booted unclaimed, through the claim-window,
sections-authorization, spare-key-and-throttle, LAN-guard and brute-force
salvo rounds (drivers: `nix/tests/adversary/boot.py` and `report.py`; the
ledger seam is `adversary/coverage.md`). The edge, mesh and stack tiers,
the literal iso-install disk and the browser-driven runs are the design's
next steps, left to the builder below. Written for a builder.

## Intent and why

LosOS is an appliance whose users install it and stop thinking about it: no
SSH, no shell, self-updating. A bypass of the security model that matters is
one an attacker exercises against a running box in the field. Once a release
ISO is out, a real exploit is a user's problem (their data, their home).

So the security model is bet against an adversary in CI: on a schedule, an
OpenCode agent boots the appliance built from `main` and attacks it, and every
confirmed bypass lands as a pull request. The reward this design buys is
twofold: bypasses caught before release, and each catch growing `nix/tests`
into a regression suite, so what the adversary finds once stays caught
forever.

The agent uses OpenCode's free tier (`OPENCODE_API_KEY` as an actions
secret — see https://opencode.ai/docs/zen), the same mechanism
`.github/workflows/opencode-autofix.yml` already uses. Run cheap enough to
run forever: free models, bounded rounds, weekly.

## Goals

1. **Catch bypasses of the security model before users are hit.** The defeat
   condition is a demonstrated boundary crossing, not a suspicious opinion.
2. **Secure-by-default checks.** The adversary attacks the default
   configuration of an installed box — the iso-install path, not an
   arithmetic of opt-in toggles. A `hardening.*` opt-in the UI offers is not
   the target; what every freshly installed box does is.
3. **Findings become pull requests.** A confirmed finding opens a PR to
   `main` containing the finding, a reproducible check, and — where the fix
   is obvious — a fix sketch. The owner reviews, merges, and the finding's
   check becomes an `nix/tests` VM test (or the finding doc says exactly why
   the boundary cannot be tested in a VM test).
4. **Cheapest thing that runs weekly and keeps running.** Inert without the
   API key (green, with a notice), resilient to a saturated free model, never
   unbounded.

## Non-goals

- **Not a replacement for source review.** Code-audit opinions, OPSEC gaps
  (the OPSEC-review gap map), and hardening advice belong to the existing
  review process. The adversary's finding is about a running box.
- **Not fuzzing** the appliance beyond what a boundary check needs: no
  memory-corruption hunting, no k3s fuzzing, no DoS budget that degrades the
  box for other rounds. Those are professions of their own; a VM CI run
  cannot do them credibly. **Brute-forcing is in, as a bounded lane** (see
  below) — credential guessing aimed at the rate-limiting boundaries, with
  a budget.
- **Not touching real infrastructure.** The agent's blast radius is a QEMU
  process on the Actions runner and the disk images it was handed. No
  requests to dasmat.us, the Vercel demo or the edge in production.
- **Not re-finding documented weaknesses.** `docs/security-model.md` already
  admits the admin/apps single origin, the LAN's first-contact TLS, the
  TPM's no-PCR binding, throttling-by-spoofable-address, the audit log root
  can rewrite. The hardening "left out on purpose" list (`rp_filter 2`, user
  namespaces, `/tmp` noexec — `modules/hardening.nix`), and the deliberate
  trade-offs in modules, are excluded too. A finding that only restates a
  documented weakness is noise; the brief names the exclusions, and finding
  triage follows them — re-arguing a trade-off belongs in source-review PRs.

## What the adversary attacks

The brief derives its target list from `docs/security-model.md`, both as
sourced attack surface and as source of exclusions. Standing priorities,
boundaries the box claims today:

1. **The claim window.** `POST /api/setup/claim` on an unclaimed box must
   close on first use, refuse non-LAN sources, refuse the LAN-lookalike
   bodies (non-JSON, wrong `Host`, IP-literal host, mismatched `Origin`, the
   browser-reachable variants), and stay 503 until LosOS Login is ready.
   Attack it fresh-state, racing the window, rebinding DNS, spoofing
   addresses a source that looks like LAN.
2. **The LAN-only guard and loopback.** Admin must not answer from outside
   the LAN and must not treat loopback as a peer. Try TR-069-style upstream
   reads, `Host` header games, 6to4/NAT66 reading, source spoofing, mesh
   (`10.42.0.0/16`) reach-over — the set the model says the guard refuses.
3. **Section authorization.** The `backend/sections.json` map: a
   `losos-viewers` person must get only `GET`s, every section group must
   stay inside its route, a route missing from the file must stay
   admin-only, `losos-owner` must be unattainable through the API's people
   endpoints, removal of self must fail.
4. **Token and identity handling.** The spare key never reappears once
   `claimed` is recorded (fail-closed rules included); `X-Losos-As` must be
   inert without the admin token; `Remote-User` must only be honored from
   loopback; per-address throttle must fire and clear as the model says.
5. **Mesh `shared` isolation.** From inside a mesh-pod (or from the `shared`
   account) the box's admin surface, `notshared` user data, and other
   tenants' disk must be unreachable the way the model claims. Follow the
   setup `nix/tests/impermanence.nix` and `mesh-*` tests already cover; the
   adversary tries the paths those tests did not think of.
6. **The edge, when the budget allows.** `edge/`: a gateway's router
   allow-list must refuse unknown tenants, blank/short/stale token variants,
   and the registration/rebinding timing-matching behavior must hold. The
   proven pattern here is the existing `nix/tests/edge-*.nix` family. A run
   should not attempt the whole edge surface every week; it picks the tier
   below by cadence input.
7. **The shipped stack, UI first (RCE hunting).** Everything in the box
   serves web pages and runs user-shaped code, so the tiers above are one
   lane of two. The other lane walks the UI and every function the project
   ships, hunting XSS-to-compromise chains beyond the *documented* CSP
   failure (which is excluded, not attacked) and remote-code paths: the
   admin UI app (`admin-ui/app`), the hand-written widget sandbox and its
   postMessage contract, the Lab (the qemu-wasm engine — a guest console is
   attacker-reachable code), the app-builder, the plugins and assistant
   tool surfaces, the `losos` terminal client's call protocol, the edge
   registrar's web surface, and the market's escrow contracts. The tier
   drives a real browser (Playwright, the tooling the admin UI already ships
   in CI) on the attacker's side of the net against the appliance.

**The brute-force lane (bounded, its own round).** Credential guessing is
in scope, aimed at *rate-limit boundaries*, not at hash strength the box
cannot out-run:

- **Budget.** At most ~2,000 guesses per run, several short salvos rather
  than one long grind, all against the seeded victim accounts' own
  endpoints. Nothing runs long enough to matter to the box's workload, and
  the box is a VM the agent may not survive — testing can be generous in
  counts, never in duration.
- **What it attacks.** Authelia's own regulation on `/auth/` (window start,
  doubling, expiry, whether the correct password inside a window clears or
  keeps the lock); `lososd`'s per-address throttle on bad Bearer tokens
  (the 10-failure window, `Retry-After`, correct-token-after-window);
  the edge's 8-in-flight-per-address caps and its timing-matched decoy
  reads; the spare key's 64-hex shape (a *weakened* token form must not
  authenticate).
- **What counts.** A limiter that does not fire, re-arm, decay or stop
  exactly where the security model states it, or a callback that turns a
  refused guess into an accepted one, is a finding like any other: boundary
  behavior the model did not deliver.
- **What stays out.** The documented per-address spoofability of the
  throttle, and proofs that 256-bit entropy is entropy — a finding's
  punchline is a boundary crossed, never an arithmetic fact.

The brief also gives the agent **negative space**: where it found a boundary
solid (a refusal that held), it says so in one line, so the report doubles as
evidence that current behavior matches the model.

## The boundaries of each run

**Cadence.** A `schedule` cron, weekly, off peak, offset from the existing
scheduled workflows (`lockfile-updates` runs ~04:17 Mondays, `ci` has its
own scheduled slot) — say later on a Monday. Plus `workflow_dispatch` with
a `scope` input (`core`, `edge`, `mesh`, or `full`). Dispatch before a
release tag is encouraged (handbook note, not a gate). A `main` push does
NOT re-run anything: the cron is the trigger.

**Who the scheduled run attacks (my call, veto-able):** the cron is a
rotation, not one tier. Quoting the ISO week number: `core` every week;
`edge` on one parity and `mesh` on the other. A week therefore costs one
appliance most weeks, the union on a `full` dispatch. Disputes are cheap:
any boundary edge/mesh misses waits at most a fortnight, and a dispatch
collapses that to an afternoon.

**Tiers by scope:**

| Tier | Boots | Rounds |
| ---- | ----- | ------ |
| `core` | appliance (installed disk; see below) | claim window fresh-state; claimed box, viewer victim; claimed box, spare-key and throttle |
| `edge` | an edge gateway VM of `edge/` (pattern: `nix/tests/edge-*.nix`) + the appliance behind it | registration, token handling, unknown tenants, LAN-lookalike |
| `mesh` | two appliances, one lending, networked | `shared` isolation, cross-reads, mesh pod reach-back |
| `stack` | the appliance, driven by a real browser on the attacker side | admin UI XSS chains past the documented baseline, widget sandbox, Lab engine, app-builder, plugins and assistant tool surfaces, the terminal client's call protocol, escrow contracts — RCE hunting |
| `full` | the union; bounded rounds | as the run gets to them |

Round count is bounded by the run's concurrency budget and wall-clock, so a
stuck agent's tier ends early without failing the workflow; findings already
reported still land.

**The installed disk is the iso-install result, literally.** The driver
builds the installer from `main` (the same export `install` CI builds into,
minus the check's interactive driver), runs the real install in QEMU under
KVM the way `nix/tests/iso-install.nix` does, and hands the adversary the
booted result — not a declarative `nix/tests` box configured from scratch
(the `discovery/box.nix` shortcut). A declarative stand-in would differ
exactly where the boundaries live: TPM presence, LosOS Login's start order,
the claim's 503-until-ready window. Runtime is the price; it is a one-time
"become the box" pass per tier, not per attack.

**States, and how fresh is regained.** One QEMU instance per attack-side
state (my call): the claim tier reinstalls (that is what makes state fresh
again, and it is bounded by the round budget); the claimed tiers install
once, the driver claims with a seeded owner and the `losos-viewers` victim
person, then snapshots the disk, so claimed tiers reuse the snapshot across
rounds. The agent owns nothing inside the box (root is never available); it
knows the victim viewer's password so the baseline paths can be walked, but
the spare key's value stays outside its reach, so admin routes cannot be
"tested" trivially as root.

## The workflow

`.github/workflows/opencode-adversary.yml`, modeled on the two existing
opencode workflows and the `install`/`discovery` CI jobs. Two jobs, not
three (my call, after the reviewer caught it): a GitHub workflow's jobs run
on separate runners, so a QEMU process and its disks cannot survive a job
boundary.

1. **Gate job.** `HAS_API_KEY` from the secret, same pattern as autofix:
   without the key, the run exits green with a notice that the adversary
   stayed inert. Concurrency group is `opencode-adversary` (in-progress
   cancelled: a manual mid-week dispatch supersedes last week's run if
   it's still going).
2. **Boot-and-attack job (one job, several steps).** As CI does for
   `losos-iso-install` and `discovery`: `./.github/actions/setup-nix`, the
   udev rule that opens `/dev/kvm`, disk space freed, then `nix build` of
   the check the new `nix/tests/adversary` target exports. In this same
   job, the driver boots the tier's VMs in QEMU and leaves them running
   while `anomalyco/opencode/github@latest` runs as a sibling step, the way
   autofix interleaves its own steps. `share: "false"` on the action,
   `OC_MODEL`/`OC_FALLBACK_MODEL` from the free-tier note:
   `opencode/glm-5.3-flash-free`, fallback `opencode/deepseek-v4.1-flash-free`
   (re-check https://opencode.ai/models/#free-models when these rotate, as
   autofix's header comment already says).
   The agent has workspace access, but its brief tells it the only repo
   write is the findings directory (`adversary/findings/`) and its scratch
   reports (`adversary-tool/`). The brief skeleton is written once; the
   workflow interpolates **addresses, credentials, tier, scope, and the
   coverage ledger, nothing else.** A full log of the tier's boot (ports,
   states, seeded credentials) travels as the run's artifact, not inside
   the agent's environment.
3. **Report job.** Validates every finding file's schema (see below),
   refuses unschema'd files, opens one PR to `main` with all of the run's
   confirmed findings if any exist, with the run summary in the body as a
   headline plus the findings table as an artifact in either case. The
   PR opening follows the autofix precedent of `losos-ci-bot`.
   Whatever the agent wrote in the findings files is not trusted markdown:
   the report job renders the run's headline through `github-script` from
   the validated frontmatter, so the PR body is never lost to injection.
   **Auto-merge (the owner's call, recorded):** the report job then enables
   auto-merge (`gh pr merge --auto --squash`) on the findings PR it opens,
   so a green run's report lands the moment required checks pass; findings
   PRs touch only `adversary/`, so nothing slow gates them, and a `hold`
   label blocks the merge until a human asks the question it names.
   (The repo needs "Allow auto-merge" on in its settings for this to have
   an effect; the workflow warns in its log when the setting is off, it
   does not fail.)

**The coverage ledger.** `adversary/coverage.md` is committed, and the
brief reads it first (my call): the frontier, not aspiration, carries
coverage across weeks. Each line names a tier, a boundary, the last week
attempted, and a verdict (`held` / `crossed <PR>` / `skipped: reason`).
The report job updates it: as part of the findings PR when findings exist,
or as a one-file `chore:` commit to `main` when the run found nothing worth
a PR. Both are additive and tiny; a saturated-model week then costs a stale
date in a text file, not the coverage argument itself.

## The finding schema

Each finding is a file `adversary/findings/YYYY-MM-DD-<slug>.md`, begins
with a fenced YAML frontmatter,:

```yaml
boundary: "security model section name crossed"   # one of the standing priorities above, or "other"
claim: "one sentence: actor, action, expected refusal"
severity: critical | high | medium
status: confirmed | suspicion
state: fresh-claim | claimed-viewer | claimed-no-creds | edge | mesh
```

- **`confirmed`** requires a demo in the body: the exact requests in text
  transcript form or as a rerunnable script under
  `adversary/tooling/<slug>/`, a statement of what the model says should
  have happened instead, and the boundary that the check, converted into
  a VM test, would pin.
- **`suspicion`** files ride along in the artifact but do NOT open PRs on
  their own.

The frontmatter is so a downstream consumer (the report job, a human, a
future CVSS-ish triage) can sort and validate mechanically.

## The blue-team lane

Two halves, one scheme (the owner's call, recorded): the adversary reports
and the blue team mitigates — findings do not queue for a human to remember
them.

**Triggered mitigation.** A second trigger on the same workflow fires when
the findings PR merges (`pull_request: closed` with `merged == true` on
`adversary/**` paths), plus a monthly set-piece cron and a manual
`workflow_dispatch`. The blue-team run's brief: read the ledger and every
open finding, then work them — each mitigation lands as *its own* PR to
`main` (module or code change, a `nix/tests` VM test that pins the fixed
boundary, and the matching `docs/security-model.md` edit where behavior
changed). Fix PRs are **not** auto-merged; only the findings report is,
and the `hold` label stops that too. The blue-team run closes each
finding in the ledger (`fixed <PR>` / `declined: reason`) and, when a
finding turned out wrong, leaves a note instead of silence.

**The standing pass over the shipped stack.** Beyond reacting to findings,
the blue-team lane keeps a slow, human-paced review running over
everything the project ships — the admin UI, the widget sandbox, the Lab,
the app-builder, plugins and assistant tooling, the terminal client
protocol, the registrar and the escrow contracts — a hunt for RCEs, the
known XSS-to-compromise chain, and cross-boundary leaks the source shows
frankly: analysis and proof-of-concept over the code and its tests, so
suspicions arrive in the findings schema as `state: source-review` and
become briefs for the next adversary run to demonstrate live.

**First dispatch, out-of-band.** The scheme's first blue-team pass ran the
day of this design: an isolated checkout, the top implementable
mitigations from the security model's known limitations (respecting every
trade-off that model declined on purpose), plus the source-review pass
above; its commits come to `main` after human review, the same rule as
always.

## Follow-on: the same scheme for LosOS Desktop

Once the first findings PR is up against this repository, port the whole
scheme to `losos-project/desktop` — the second project the OPSEC review
serves. The workflow, brief skeleton, finding schema and coverage ledger
copy port; the surface list maps onto Desktop's own (`derisk`, `Danube`,
the libvirt machines and their colour-label networks, `pm`,
`losos-adblock`, homed), the boot targets build from Desktop's
`nixos/tests` instead of `nix/tests`, and the repo keeps its own
`OPENCODE_API_KEY` secret and its own ledger, so the two schemes never
share a frontier.

## Where things land in the repo

```text
.github/workflows/opencode-adversary.yml   the runner
nix/tests/adversary/                       boot targets and driver, exports a check per tier
adversary/findings/                        committed findings (one PR each time one lands)
adversary/coverage.md                      the ledger the brief reads and the report job updates
docs/security-model.md                     updated by a fix's PR, never by the adversary's
```

Every confirmed finding's conversion into a lasting regression test is part
of the fix's review checklist if the finding's boundary can be pinned in a
`nix/tests` VM test (the OPSEC house rule); the finding's `confirmed` demo
states, at minimum, which flow a VM test would exercise. Not every boundary
is testable in a VM (the edge's Let's Encrypt handshake, DNS rebinding with
a real upstream) — the finding doc admits that explicitly with a note of
what a test would observe instead.

## Guards, and where they bite

- **Secrets stay out of the agent.** `OPENCODE_API_KEY` reaches the action;
  the agent receives no GitHub token (the report job is its own step),
  no registry/logins, no listen ports beyond the VM tier's addresses.
- **The agent wants to escalate into a reported PR.** Because the report
  job re-renders the PR, the agent's markdown cannot inject links or
  badges; the PR body's headline comes from the validated frontmatter.
- **No silent hop-off.** The action's `share: "false"` stays on every call,
  autofix's same note in its header comment, so a public repo's share does
  not become a publishing channel.
- **Bounded costs.** Job `timeout-minutes` and the round budget
  interplay: a saturated primary model logs a notice, the fallback runs
  the same brief used by autofix's pattern; if both free models fail, the
  run files an empty-runs notice rather than retry-with-backoff all week.
- **Nobody's release breaks.** The workload is additive: no builds' checks
  gate on the adversary except a CI-green PR for a confirmed finding.

## Open trade-offs recorded, not hidden

- **A confirmed PR's exploit detail is public.** The body says what
  boundary was crossed and the artifact carries the requests; if the
  severity is critical and the release is live, the practical mitigations
  are the fix's speed and the choice to run a `core` tier dispatch before
  tagging rather than after. The design takes that knowingly.
- **The PR touches only `adversary/`.** The security-model wording a
  finding suggests is quoted inside the finding's markdown as *proposed
  text*, inert; `docs/security-model.md` itself is edited by the fix's PR,
  never by the adversary's. The fix's review checklist is where the
  proposal converts into reality.
- **Findings self-merge, by request.** The owner traded the pause before
  the report lands for the coverage argument: a nonsense finding that
  slips past schema validation commits itself. The ledger and the squash
  history make such a commit visible for seconds — cheap to revert — and
  the `hold` label is the way to read a run before its PR unpauses.
- **Free model saturation means quiet weeks.** Some weeks produce
  artifact-only reports, few findings. Weekly repeats mean the next week
  picks up where it left off (the ledger), not a permanent drop in
  coverage.
- **GitHub disables schedules after 60 days of no repo activity.** This
  repo's own bots (`package-freshness` daily, `lockfile-updates` weekly)
  keep activity flowing, so the design accepts the risk rather than adding
  a keep-alive bot; if the bots ever go quiet, a monthly no-op branch
  touch is the workaround, recorded here rather than built now.

## Left to the builder

- The exact NixOS VM check names inside `nix/tests/adversary/` and the
  driver's plumbing: the tier VMs, their seeded states, and what the
  `boot` job's `GITHUB_OUTPUT` emits. Advice: mirror `nix/tests/discovery`
  (`nix/tests/discovery/check.py`) and `nix/tests/iso-install.nix`,
  booted by CI the way the `install` and `discovery` jobs do it.
- The exact wording of the agent brief, once the tier
  inputs' names and addresses are known; the brief must name the excluded,
  documented weaknesses (the list above) and require the schema (above).
- Artifact layout and retention inside the run (suggest 14 days, artifact
  per tier, as the discovery job's naming works).
- Any tightening of the cron that keeps weekly cadence and doesn't run during
  the primary `main` push window of the existing workflows.
