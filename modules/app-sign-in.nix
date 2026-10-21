# Signing in to LosOS cloud and LosOS Git with LosOS Login, as plain data.
#
# nginx asks LosOS Login (modules/identity.nix) who is calling and tells the
# two apps in three request headers. Nextcloud reads them through user_saml
# in its environment-variable mode; LosOS Git (backend/src/forge/auth.rs)
# reads them itself. Both make an account the first time a person arrives
# and sign them in to it after that.
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
#   * Nextcloud listens on 127.0.0.1 only, and the firewall turns away any
#     other process on the box that tries to reach its port
#     (modules/identity.nix), the way it does for lososd;
#   * LosOS Git reads them only on a Unix socket that nginx alone may open
#     (modules/git.nix), and never on its loopback port.
#
# Nothing here reads `config`: nix/images.nix bakes the Nextcloud half into
# the pod's entrypoint, where there is no module system.
{ lib }:

rec {
  # The group lososd puts the claimant in (backend/src/people.rs).
  ownerGroup = "losos-owner";
  # Their account in both apps (modules/nextcloud-stack.nix, adminUser; the
  # LosOS Git account lososd keeps, backend/src/config_repo.rs).
  ownerAccount = "notshared";

  header = {
    user = "X-Losos-Cloud-User";
    name = "X-Losos-Cloud-Name";
    email = "X-Losos-Cloud-Email";
  };

  # How PHP spells a request header in $_SERVER.
  serverVar = h: "HTTP_" + lib.toUpper (builtins.replaceStrings [ "-" ] [ "_" ] h);

  # The occ calls that switch Nextcloud's half on ("yes") or off ("no").
  # `occ` is the command to run occ with, `yes` a shell word that expands to
  # yes or no. user_saml's environment-variable mode reads provider 1 for
  # the name of the variable holding the account (SessionService
  # ENVIRONMENT_IDENTITY_PROVIDER_ID), and `saml:config:set` makes that row
  # if it is missing. `general-require_provisioned_account` 0 lets it make
  # an account for a person it has not seen. Non-fatal: a failed call leaves
  # the app's own sign-in page, which still works.
  nextcloud.occ = occ: yes: ''
    if [ ${yes} = yes ]; then
      ${occ} app:enable user_saml >/dev/null &&
        ${occ} config:app:set user_saml type --value=environment-variable >/dev/null &&
        ${occ} config:app:set user_saml general-require_provisioned_account --value=0 >/dev/null &&
        ${occ} saml:config:set 1 \
          --general-uid_mapping=${serverVar header.user} \
          --saml-attribute-mapping-displayName_mapping=${serverVar header.name} \
          --saml-attribute-mapping-email_mapping=${serverVar header.email} >/dev/null ||
        echo "losos: could not switch on sign-in with LosOS Login; LosOS cloud keeps its own sign-in page." >&2
    else
      ${occ} app:disable user_saml >/dev/null 2>&1 || true
    fi
  '';
}
