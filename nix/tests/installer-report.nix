# nixos-test-vms config for problem reports from the installer medium
# (backend/src/install_report.rs, backend/src/install_screen.rs,
# modules/installer.nix, handbook/docs/manual/problem-reports.md).
#
# The medium of nix/tests/installer-screen.nix, under its boot screen, with a
# stand-in for GitHub on its loopback (github-stand-in.py) and the spike
# thresholds turned down so that any run crosses them. Two installs:
#
#   * one whose disk step fails with something the handbook does not
#     explain: the panel offers a report, the report and the tools' output
#     are on screen 7 to read, R sends it with the device flow's code on the
#     panel, and the issue is filed only once the code is entered;
#   * one that finishes: the panel says what spiked, and offers a report of
#     that.
#
# The installer runs through its --disko-script seam, with a script that
# holds until the test says to break or to go on.
{
  pkgs,
  disko,
}:

let
  github = 8765;
in
pkgs.testers.nixosTest {
  name = "losos-installer-report";
  enableOCR = true;

  nodes.medium =
    { config, pkgs, ... }:
    let
      lososPkgs = import ../packages.nix { inherit pkgs; };
    in
    {
      imports = [
        disko.nixosModules.disko
        ../../modules/options.nix
        ../../modules/disko.nix
        ../../modules/installer.nix
        ../../modules/branding.nix
        ../../modules/splash.nix
      ];
      losos = {
        installer.package = lososPkgs.losos-ctl;
        installer.autorun = true;
        targetDrives = [ "/dev/vdb" ];
        tpm.enable = false;
        bios = true;
        report = {
          clientId = "Iv23test";
          login = "http://127.0.0.1:${toString github}";
          api = "http://127.0.0.1:${toString github}";
        };
      };
      disko.enableConfig = false;
      services.lvm.enable = true;

      environment.etc."losos/disko-script".source = pkgs.writeShellScript "disko-held" ''
        echo "losos-test: disko script started"
        while [ ! -e /run/losos-go ] && [ ! -e /run/losos-break ]; do sleep 1; done
        if [ -e /run/losos-break ]; then
          rm /run/losos-break
          echo "losos-test: the pretend partitioner gave up on 10.0.2.15"
          exit 7
        fi
        exec ${config.system.build.diskoScript}
      '';
      systemd.services.losos-installer = {
        scriptArgs = "--disko-script /etc/losos/disko-script --drives /dev/vdb";
        # Any use is a spike here: memory over 1 % for two seconds.
        environment = {
          LOSOS_SPIKE_MEMORY_PERCENT = "1";
          LOSOS_SPIKE_MEMORY_SECS = "2";
        };
      };
      systemd.services.github-stand-in = {
        wantedBy = [ "multi-user.target" ];
        serviceConfig.ExecStart = "${pkgs.python3}/bin/python3 ${./github-stand-in.py} ${toString github} /var/lib/github";
      };

      boot.initrd.kernelModules = [ "bochs" ];
      boot.initrd.systemd.enable = true;
      virtualisation = {
        memorySize = 2048;
        cores = 2;
        emptyDiskImages = [ 2048 ];
      };
    };

  testScript = ''
    import json

    def menu_shown(times):
        def shown(_):
            return medium.get_tty_text("1").count("firmware menu on the boot screen") >= times
        retry(shown)

    def requests():
        out = medium.succeed("cat /var/lib/github/requests.jsonl 2>/dev/null || true")
        return [json.loads(line) for line in out.splitlines() if line.strip()]

    def filed():
        return json.loads(medium.succeed("cat /var/lib/github/issues.json 2>/dev/null || echo '[]'"))

    medium.start()
    medium.wait_for_unit("losos-installer.service")
    medium.wait_for_unit("github-stand-in.service")

    with subtest("a failure the handbook does not explain offers a report"):
        menu_shown(1)
        medium.sleep(8)
        medium.send_key("1")
        medium.wait_until_tty_matches("2", "losos-test: disko script started")
        # Long enough for the watcher to see two seconds over the line.
        medium.sleep(5)
        medium.succeed("touch /run/losos-break")
        medium.wait_for_text("Press R to send a report")
        medium.screenshot("failed-offer")
        report = medium.succeed("cat /run/losos-install/report.md")
        print(report)
        title = report.splitlines()[0]
        assert title.startswith("# Installer: "), title
        assert "Erasing and encrypting the disks" in report, report
        assert "losos-test: the pretend partitioner gave up on [IP]" in report, report
        assert "10.0.2.15" not in report, report
        assert "memory spike while erasing and encrypting the disks" in report, report
        assert " while ." not in report and "see tty2" not in report, report
        assert "| LosOS | v" in report, report
        # Nothing has left the machine.
        assert requests() == [], requests()

    with subtest("the whole report is on screen 7"):
        medium.succeed("chvt 7")
        medium.wait_until_tty_matches("7", "whole report the installer would send")
        medium.screenshot("failed-screen7")
        medium.succeed("chvt 1")

    with subtest("R sends it: the code on the panel, the issue once it is entered"):
        medium.send_key("r")
        medium.wait_for_text("WDJB-MJHT")
        medium.screenshot("failed-code")
        assert filed() == [], filed()
        medium.succeed("touch /var/lib/github/approved")
        medium.wait_for_text("Report sent")
        medium.screenshot("failed-sent")
        issues = filed()
        assert len(issues) == 1, issues
        assert issues[0]["title"] == title.removeprefix("# "), (issues, title)
        assert "10.0.2.15" not in issues[0]["body"], issues
        paths = [r["path"].split("?")[0] for r in requests()]
        assert paths[0] == "/login/device/code", paths
        assert paths[-1] == "/repos/dasmatus/losos/issues", paths
        medium.send_key("ret")
        medium.wait_until_fails("plymouth --ping")
        medium.wait_until_tty_matches("1", "This is a root shell")

    with subtest("an install that finishes still says what spiked"):
        medium.succeed("systemctl restart plymouth-start.service")
        medium.succeed("rm -rf /run/losos-install")
        medium.succeed("systemctl restart losos-installer.service")
        menu_shown(2)
        medium.send_key("1")
        medium.wait_until_tty_matches("2", "losos-test: disko script started")
        medium.sleep(5)
        medium.succeed("touch /run/losos-go")
        medium.wait_for_text("The disks are ready")
        medium.wait_for_text("While it ran")
        medium.screenshot("finished-spike")
        report = medium.succeed("cat /run/losos-install/report.md")
        print(report)
        assert report.startswith("# Installer: memory use reached"), report
        assert "The install finished." in report, report
        medium.send_key("ret")
        medium.wait_until_fails("plymouth --ping")
  '';
}
