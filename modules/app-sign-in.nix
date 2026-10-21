# Signing in to LosOS Cloud and LosOS Git with LosOS Login, as plain data.
#
# nginx asks LosOS Login (modules/identity.nix) who is calling and tells the
# apps in three request headers. LosOS Cloud reads them itself
# (backend/src/cloud/auth.rs), Forgejo through its reverse-proxy sign-in.
# Both take a person they have not seen as a new account and sign them in to
# it after that.
#
# The person who claimed the box is in `ownerGroup` and arrives as the
# account that was always theirs, `ownerAccount`, which holds their files
# and repositories. Everyone else arrives under their own sign-in name.
#
# Forgejo believes these headers from whoever sends them, so three things
# keep it honest:
#
#   * nginx sets all three on every request it passes to it, empty (so not
#     sent) unless LosOS Login vouched for the person, whatever the client
#     put there;
#   * it listens on 127.0.0.1 only;
#   * the firewall turns away any other process on the box that tries to
#     reach its port (modules/identity.nix), the way it does for lososd.
# LosOS Cloud listens on a unix socket only nginx can open (modules/cloud.nix).
#
# Nothing here reads `config`, so the VM tests can import it as it is.
_:

rec {
  # The group lososd puts the claimant in (backend/src/people.rs).
  ownerGroup = "losos-owner";
  # Their account in both apps (LosOS Cloud's files under this name, the
  # Forgejo account lososd keeps in step, backend/src/config_repo.rs).
  ownerAccount = "notshared";

  header = {
    user = "X-Losos-Cloud-User";
    name = "X-Losos-Cloud-Name";
    email = "X-Losos-Cloud-Email";
  };

  # app.ini sections for Forgejo. Its reverse-proxy sign-in comes before its
  # own session, so the person LosOS Login names is the one signed in, and an
  # unknown name gets an account (`AUTO_REGISTRATION` is not held back by
  # DISABLE_REGISTRATION, which keeps the sign-up form closed). E-mail is left
  # out on purpose: with it on, Forgejo also looks people up by the address
  # header, a second name for the same account.
  forgejo = {
    service = {
      ENABLE_REVERSE_PROXY_AUTHENTICATION = true;
      ENABLE_REVERSE_PROXY_AUTO_REGISTRATION = true;
      ENABLE_REVERSE_PROXY_FULL_NAME = true;
    };
    security = {
      REVERSE_PROXY_AUTHENTICATION_USER = header.user;
      REVERSE_PROXY_AUTHENTICATION_FULL_NAME = header.name;
      REVERSE_PROXY_LIMIT = 1;
      REVERSE_PROXY_TRUSTED_PROXIES = "127.0.0.0/8,::1/128";
    };
  };
}
