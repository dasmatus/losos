import Base

# ─ What this is ────────────────────────────────────────────────────────────
#
# The laws the Bend test port stands on. The repo's convention (AGENTS.md,
# "Laws and Proofs"): LAWS.bend states each claim as an open law, and
# PROOF.bend proves it with a `def Laws.<name>` of the same shape — the AI
# writes the proof, and `bend PROOF.bend` (the gate the git hooks and
# nix/bend.nix run) prints SOME PROOFS FAIL while any law is open or false.
#
# A law covers every input, the way a unit test's assertions cover the
# examples it names. When a test file is ported to Bend (a `*.bend` file
# beside the test it replaces), the claims that should hold for all inputs
# graduate here, and the file's `main` keeps checking the concrete examples
# against the same logic.
#
# Read a law as the reviewer-facing contract: what the code must do, in
# one sentence, checked mechanically. Do not edit a law to make a proof
# pass; change the code instead. A law with no proof is an open claim, so
# a half-finished port shows up as a red gate, never as a silent pass.
#
# No laws yet: the first ported test adds the first one.
