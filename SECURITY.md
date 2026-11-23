# Security policy

LosOS is a NixOS appliance: no SSH and no login shell, administered from
web pages, self-upgrading. The [security model](docs/security-model.md)
states the trust boundaries and the tradeoffs the design already accepts —
including that the admin token is equivalent to root. A report that
describes what the model already accepts is not a vulnerability. But the
model is prose, not proof: if you can show a real person crossing a
boundary the doc promises, that is exactly what we want to hear.

## Reporting a vulnerability

Use GitHub's private vulnerability reporting on this repository
(**Security → Report a vulnerability**). Please do not open a public
issue, and please do not sit on a working exploit hoping for a bounty —
there is none.

Include, in whatever form is quickest for you: what you did, what the box
did, the version (the release tag, or what Settings shows), and what a fix
or a test would need to show. An untrusted proof of concept that runs on
your own box, or a VM test under `nix/tests/` that fails on a current
build, is worth more than a write-up.

## Scope

In scope, the appliance's own surfaces:

- lososd's API and everything `/api/apply` can do
- the admin pages and sign-in (LosOS Login), including the
  `notshared`/`shared` data boundary `nix/tests/impermanence.nix` checks
- the installer, the disk unlock path and the TPM sealing
- the edge registrar, its passport/attestation and the escrow programs
- the terminal envelope (`backend/access`), which signs and wraps every
  remote call

Out of scope: the demo hosts, a box whose owner gave you an account, and
reports that the admin token is powerful — the model documents that it is.

One boundary belongs to someone else. LosOS Login is Authelia, as this
repository wires it (`modules/identity.nix`): a flaw in Authelia itself —
its sessions, its storage backends, its web server — should be reported
upstream, at
[Authelia's security policy](https://github.com/authelia/authelia/blob/master/SECURITY.md).
How the box wires it is still ours: the `auth_request` pass nginx makes,
the people file lososd writes, the bearer-token bypass that keeps the
spare key working. If you cannot tell which side of that line your finding
is on, report it here and it will be forwarded.

## Handling

One maintainer reviews today; expect a reply within days rather than
hours. A confirmed fix lands on main with a VM test pinned to the report —
`nix/tests/impermanence.nix` is the example to read — and a release
carries it from there. Credit is yours for the asking; say so in the
report. Until the release ships the fix, coordinated disclosure: please do
not publish first.
