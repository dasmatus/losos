import Base
import LAWS.bend as LAWS

# ─ What this is ────────────────────────────────────────────────────────────
#
# The proofs of LAWS.bend: one `def Laws.<name>` per `law <name>` there.
# `bend PROOF.bend` is the repo's Bend gate — the git hooks, devenv and
# nix/bend.nix all run it — so a law left open, or proven false, fails the
# gate instead of passing silently. The matcher (`bend PROOF.bend --verdict`)
# rechecks every def with the Lean kernel, but needs Lean 4.34.0 on the
# PATH (`elan toolchain install leanprover/lean4:v4.34.0`); the plain check
# does not.
#
# No proofs yet: each ported test adds its law to LAWS.bend and its proof
# here, together.
