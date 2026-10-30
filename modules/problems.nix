# Problem reports from the box (backend/plugins/problems,
# handbook/docs/manual/problem-reports.md).
#
# The problems plugin reads the journal every minute for services that
# failed, programs that crashed and programs the kernel killed for memory,
# and keeps each by its fingerprint. Settings › Problems lists them; for one
# the handbook does not already explain, the page shows the whole report and
# sends it to the repository's issues once the owner signs in to GitHub with
# the device flow's code. The token lives in the plugin's job for that one
# report.
#
# This module tells lososd where reports go, and gives it the GitHub App's
# client ID as the systemd credential `report-client-id`. The installer
# leaves the value it carries (modules/installer.nix) at the `legacy` path
# below, and the box's first boot seals it into /etc/credstore.encrypted
# (modules/credentials.nix) and deletes that copy; lososd imports it from
# there. A box built with losos.report.clientId (a test, a build of one's
# own) has that as the fallback. With neither, or with losos.report.enable
# off, the plugin answers `reporting: null` and the page offers no Send.
# Crashes leave no core file here: modules/hardening.nix sends cores nowhere
# on purpose, so the report carries the signal, the unit's state and its
# journal instead.
{
  config,
  lib,
  ...
}:

let
  cfg = config.losos.report;
  fallback = cfg.clientId != null && cfg.clientId != "";
in
{
  config = lib.mkIf (config.losos.backend.package != null) (
    lib.mkMerge [
      {
        systemd.services.lososd.environment = {
          # The release a report names (nix/version.nix, as os-release has it).
          LOSOS_RELEASE = import ../nix/version.nix;
        };
      }
      (lib.mkIf cfg.enable {
        losos.credentials.entries.report-client-id = {
          app = "admin";
          description = "The client ID of the GitHub App problem reports sign in with, as the installer handed it over.";
          pane = "/settings/problems";
          legacy = [ "/var/lib/losos/report-client-id" ];
        };
        systemd.services.lososd = {
          after = [ "losos-credentials.service" ];
          environment = {
            LOSOS_REPORT_REPOSITORY = cfg.repository;
            LOSOS_REPORT_LOGIN = cfg.login;
            LOSOS_REPORT_API = cfg.api;
            LOSOS_REPORT_CLIENT_ID_FILE = "%d/report-client-id";
          };
          # Import, not LoadCredentialEncrypted=: a box with no value yet must
          # still start lososd, and a missing import is no error.
          serviceConfig.ImportCredential = [ "report-client-id" ];
          serviceConfig.SetCredential = lib.mkIf fallback [ "report-client-id:${cfg.clientId}" ];
        };
      })
    ]
  );
}
