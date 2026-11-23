# Adversary coverage

The frontier the adversary picks its boundaries from: one line each, the
last week attempted and the verdict. The brief reads this first, oldest
attempt first; the run's last step rewrites it (every attempted boundary:
`held`, `crossed <finding file>`, `skipped: reason`, `budget-hit`). The
schema and the exclusions live in the design:
docs/superpowers/specs/2026-11-09-adversary-testing-design.md.

| tier | boundary | last attempted | verdict |
| ---- | -------- | -------------- | ------- |
| core | claim window — race, bodies, hosts, ready-false spine | — | not attempted yet |
| core | claim fail-closed reads (every state file answer reads as claimed) | — | not attempted yet |
| core | section authorization (sections.json map, viewers, owner) | — | not attempted yet |
| core | spare key, X-Losos-As and Remote-User handling | — | not attempted yet |
| core | token throttle windows (bad Bearer, Retry-After, doubling, clear) | — | not attempted yet |
| core | lan-only guard (tap, forwarded headers, mesh catch, loopback shape) | — | not attempted yet |
| core | brute-force salvos — auth regulation, throttle, edge caps | — | not attempted yet |
| edge | registrar tenants, tokens, timing decoys | never | skipped: edge tier not wired yet |
| mesh | shared isolation, cross-reads, pod reach-back | never | skipped: mesh tier not wired yet |
| stack | admin UI XSS chains, widget sandbox, Lab engine, tool surfaces | never | skipped: stack tier not wired yet |
