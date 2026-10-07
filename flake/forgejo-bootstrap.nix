# The LosOS Git bootstrap: the account lososd talks to Forgejo as.
#
# One shell snippet, used by both places Forgejo is started from — the
# workload image's entrypoint (flake/images.nix, before `exec forgejo web`)
# and the native service's preStart (modules/services.nix) — so the two
# modes cannot drift on it.
#
# What it makes, once: a site administrator named `losos` with a random
# password nobody is told (the account is only ever used through its token),
# and an access token for it at `tokenFile`, 0600, in Forgejo's own state
# directory. lososd reads the token (backend/src/config_repo.rs) to create
# the owner's account and the configuration repository, keep the owner's
# password in step, and authenticate its pushes. Forgejo is persisted under
# /var (modules/impermanence.nix), so the token survives a reboot with the
# database it belongs to.
#
# Why the token is minted here and not by lososd: `forgejo admin` is the
# only way to make the first administrator on an instance with
# DISABLE_REGISTRATION and INSTALL_LOCK (both set, on purpose — see
# modules/workloads.nix), and it has to run with Forgejo's database in reach,
# which is inside the pod in container mode.
#
# `forgejo migrate` must have run first: both commands touch the database.
#
# Never fatal. The snippet runs inside `set -e` scripts in both places, and
# a bootstrap that fails must not stop Forgejo itself from starting; it is
# retried on the next start, and until then lososd reports LosOS Git as
# "not up yet" on the History pane.
{
  # How to invoke the forgejo binary: `forgejo` on PATH in the image, the
  # package's exe in the native unit.
  forgejo ? "forgejo",
  tokenFile,
}:
''
  # ── The account lososd uses (flake/forgejo-bootstrap.nix) ──────────────
  losos_bootstrap() {
    if [ -s ${tokenFile} ]; then
      return 0
    fi
    if ! ${forgejo} admin user list --admin 2>/dev/null | awk 'NR > 1 { print $2 }' | grep -qx losos; then
      ${forgejo} admin user create --admin --username losos --random-password \
        --email losos@localhost --must-change-password=false > /dev/null
    fi
    # A fresh name each time: Forgejo refuses a second token of the same
    # name, and this branch is reached again only when the file was lost.
    (
      umask 077
      ${forgejo} admin user generate-access-token --username losos \
        --token-name "lososd-$(date +%s)" --scopes all --raw > ${tokenFile}.new
    )
    mv ${tokenFile}.new ${tokenFile}
  }
  if ! losos_bootstrap; then
    echo "losos: the LosOS Git bootstrap did not complete; lososd will report LosOS Git as not up until the next start" >&2
    rm -f ${tokenFile}.new
  fi
''
