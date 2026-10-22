# Signing in to LosOS Cloud and LosOS Git with LosOS Login, as plain data.
#
# nginx asks LosOS Login (modules/identity.nix) who is calling and tells the
# apps in three request headers. LosOS Cloud (backend/src/cloud/auth.rs)
# and LosOS Git (backend/src/forge/auth.rs) read them themselves. Both take
# a person they have not seen as a new account and sign them in to it after
# that.
#
# The person who claimed the box is in `ownerGroup` and arrives as the
# account that was always theirs, `ownerAccount`, which holds their files
# and repositories. Everyone else arrives under their own sign-in name.
#
# Both apps believe these headers from whoever sends them, so three things
# keep them honest:
#
#   * nginx sets all three on every request it passes to either app, empty
#     (so not sent) unless LosOS Login vouched for the person, whatever the
#     client put there;
#   * LosOS Cloud listens on a unix socket only nginx can open
#     (modules/cloud.nix);
#   * LosOS Git reads them only on a unix socket that nginx alone may open
#     (modules/git.nix), and never on its loopback port.
#
# Nothing here reads `config`, so the VM tests can import it as it is.
_:

{
  # The group lososd puts the claimant in (backend/src/people.rs).
  ownerGroup = "losos-owner";
  # Their account in both apps (LosOS Cloud's files under this name, the
  # LosOS Git account lososd keeps, backend/src/config_repo.rs).
  ownerAccount = "notshared";

  header = {
    user = "X-Losos-Cloud-User";
    name = "X-Losos-Cloud-Name";
    email = "X-Losos-Cloud-Email";
  };
}
