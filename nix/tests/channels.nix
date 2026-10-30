# nixos-test-vms config for the release channels (.github/workflows/channels.yml,
# losos.updates.channel).
#
# A box with the real lososd, nginx, LosOS Login and the built admin UI,
# following a published repository on the box itself (file:///srv/losos.git)
# whose channel branches are laid out the way channels.py lays them out: a
# cut is a commit of main or a merge commit with main's tree, a hotfix a
# commit on top, and each names the main commit it was cut from in a
# `Channel-Base:` trailer. It asserts what only a booted box can show:
#
#   * the box follows channel/daily by default and takes its tree, keeping
#     its own overrides.nix;
#   * moved to the weekly channel (a second generation, as Apply makes one),
#     whose last cut is older than what the box runs, the box keeps its
#     version and says "Waiting for the channel" until the next weekly cut,
#     which it then takes;
#   * the same holds for a monthly channel carrying a hotfix, and on the way
#     back to daily;
#   * the History pane shows the channel picker and the state, in a browser
#     on the laptop, whose pictures are in the output (screens/).
#
# `before = true` runs the same box on another tree's lososd and admin UI
# (`lososPkgs`) and only records what happens, for the pull request's Before
# pictures:
#
#   nix build --impure --expr '(import ./nix/tests/channels.nix {
#     pkgs = …; before = true; lososPkgs = import <old tree>/nix/packages.nix { inherit pkgs; }; })'
{
  pkgs,
  lososPkgs ? import ../packages.nix { inherit pkgs; },
  before ? false,
}:

let
  screens = import ./screens { inherit pkgs; };

  history = {
    path = "/settings/history";
    waitFor = {
      text = "Updates";
    };
    settle = 1500;
  };
  variants = [
    { suffix = "light-desktop"; }
    {
      suffix = "dark-phone";
      colorScheme = "dark";
      width = 390;
      height = 844;
    }
  ];

  # Taken right after the box first took channel/daily.
  planFollowing = pkgs.writeText "plan-following.json" (
    builtins.toJSON {
      width = 1280;
      height = 900;
      colorScheme = "light";
      inherit variants;
      shots = [
        (history // { name = "history-daily"; })
      ]
      ++ pkgs.lib.optional (!before) (
        history
        // {
          name = "history-pick-weekly";
          steps = [
            {
              select = {
                label = "Release channel";
                value = "weekly";
              };
            }
            { waitText = "A new version each Monday. Hotfixes still arrive the same day."; }
            { sleep = 800; }
          ];
        }
      );
    }
  );

  # Taken after the move to the weekly channel, whose cut is older.
  planHolding = pkgs.writeText "plan-holding.json" (
    builtins.toJSON {
      width = 1280;
      height = 900;
      colorScheme = "light";
      inherit variants;
      shots = [ (history // { name = "history-weekly-older"; }) ];
    }
  );

  # A generation that follows another channel, as Apply would build one. The
  # old tree has no losos.updates.channel; it names the branch.
  follow =
    channel:
    if before then
      { losos.configRepo.upstream.branch = pkgs.lib.mkForce "channel/${channel}"; }
    else
      { losos.updates.channel = channel; };
in

pkgs.testers.nixosTest {
  name = if before then "losos-channels-before" else "losos-channels";

  nodes = {
    box =
      { lib, ... }:
      {
        imports = [
          ../../modules/options.nix
          ../../modules/containers.nix
          ../../modules/tls.nix
          ../../modules/daemon.nix
          ../../modules/identity.nix
          ../../modules/config-repo.nix
        ];
        losos = {
          hostName = "mattbox";
          backend.package = lososPkgs.losos-ctl;
          admin.ui = lososPkgs.losos-admin-ui;
          git.enable = false;
          configRepo.upstream.url = "file:///srv/losos.git";
        };
        networking.hostName = lib.mkForce "mattbox";
        specialisation = {
          weekly.configuration = follow "weekly";
          monthly.configuration = follow "monthly";
          daily.configuration = follow "daily";
        };
        # The configuration repository the installer leaves behind.
        systemd.services.config-tree = {
          wantedBy = [ "multi-user.target" ];
          before = [ "lososd.service" ];
          unitConfig.ConditionPathExists = "!/etc/nixos/.git";
          serviceConfig.Type = "oneshot";
          path = [ pkgs.git ];
          script = ''
            mkdir -p /etc/nixos/modules
            cd /etc/nixos
            printf '{ ... }:\n{\n}\n' > modules/overrides.nix
            git init -q -b main
            git add -A
            git -c user.name=LosOS -c user.email=losos@mattbox.local commit -qm "The box as installed"
          '';
        };
        environment.systemPackages = [
          pkgs.git
          pkgs.jq
        ];
        virtualisation = {
          memorySize = 2048;
          cores = 2;
          vlans = [ 1 ];
        };
      };

    laptop =
      { ... }:
      {
        imports = [ screens.browser ];
        environment.systemPackages = [ pkgs.curl ];
        environment.etc."losos-test/plan-following.json".source = planFollowing;
        environment.etc."losos-test/plan-holding.json".source = planHolding;
        virtualisation.vlans = [ 1 ];
      };
  };

  testScript = ''
    import json

    BOX = "192.168.1.1"
    PASSWORD = "Correct-Horse-Battery-9"
    BEFORE = ${if before then "True" else "False"}
    DAY = 86400
    T0 = 1792800000  # 2026-10-24

    start_all()
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("nginx.service")
    box.wait_for_open_port(443)
    laptop.wait_for_unit("multi-user.target")

    def git(args, at=None):
        env = "GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@example.org GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@example.org"
        if at is not None:
            env += f" GIT_AUTHOR_DATE='@{at} +0000' GIT_COMMITTER_DATE='@{at} +0000'"
        return box.succeed(f"cd /srv/work && env {env} git {args}").strip()

    def main_commit(subject, name, text, at):
        box.succeed(f"echo '{text}' > /srv/work/{name}")
        git("add -A")
        git(f"commit -q -m '{subject}'", at)
        git("push -q origin HEAD:main")
        return git("rev-parse HEAD")

    def cut(channel, sha, at, cut_at):
        """A cut that is not a fast-forward: main's tree, the channel first."""
        head = git(f"rev-parse refs/remotes/origin/channel/{channel}")
        msg = f"Cut channel/{channel} from main {sha[:10]}\n\nChannel-Base: {sha} {at}\n"
        box.succeed(f"printf '%s' '{msg}' > /tmp/msg")
        new = git(f"commit-tree {sha}^{{tree}} -p {head} -p {sha} -F /tmp/msg", cut_at)
        git(f"push -q origin {new}:refs/heads/channel/{channel}")
        git("fetch -q origin")
        return new

    def update():
        box.succeed("losos-ctl config --update >/dev/null")
        return json.loads(box.succeed("losos-ctl config"))["upstream"]

    def switch(generation):
        box.succeed(f"/run/booted-system/specialisation/{generation}/bin/switch-to-configuration test")
        box.wait_for_unit("lososd.service")
        box.wait_until_succeeds(
            f"systemctl show lososd -p Environment --value | grep -q LOSOS_UPSTREAM_BRANCH=channel/{generation}"
        )
        if not BEFORE:
            # The line Apply writes, so the picker shows what the box runs.
            box.succeed(
                "printf '{ ... }:\\n{\\n  losos.updates.channel = \"%s\";\\n}\\n' "
                f"{generation} > /etc/nixos/modules/overrides.nix"
            )

    def shots(plan):
        laptop.succeed(
            f"losos-capture --base https://{BOX} --plan /etc/losos-test/{plan}.json "
            f"--out /tmp/screens --user matus --password '{PASSWORD}'",
            timeout=1800,
        )

    def tracked(name):
        return box.succeed(f"git -C /etc/nixos show HEAD:{name}").strip()

    with subtest("a published repository with three channels"):
        box.succeed("git init -q --bare -b main /srv/losos.git && git clone -q /srv/losos.git /srv/work")
        m1 = main_commit("Merge pull request #1", "version", "one", T0)
        m2 = main_commit("Merge pull request #2", "version", "two", T0 + 2 * DAY)
        m3 = main_commit("Merge pull request #3", "version", "three", T0 + 4 * DAY)
        # Daily took m3 last night; weekly was cut from m1 last Monday;
        # monthly from m1 too, with hotfix #2 on top.
        git(f"push -q origin {m3}:refs/heads/channel/daily {m1}:refs/heads/channel/weekly")
        git(f"checkout -q --detach {m1}")
        box.succeed("echo hotfixed > /srv/work/fix")
        git("add -A")
        msg = f"Hotfix #2 on channel/monthly: Fix the tunnel\n\nChannel-Base: {m1} {T0}\nHotfix: #2 {m2}\n"
        box.succeed(f"printf '%s' '{msg}' > /tmp/msg")
        git("commit -q -F /tmp/msg", T0 + 3 * DAY)
        git("push -q origin HEAD:refs/heads/channel/monthly")
        git("checkout -q main")
        git("fetch -q origin")

    with subtest("claim the box"):
        body = json.dumps({"name": "matus", "password": PASSWORD})
        laptop.succeed(
            f"curl -skf -X POST https://{BOX}/api/setup/claim -H 'Host: {BOX}'"
            f" -H 'Content-Type: application/json' -d '{body}'"
        )
        box.wait_for_unit("authelia-losos.service")
        box.systemctl("start losos-identity-setup.service")
        box.wait_until_succeeds(f"grep -q \"domain: '{BOX}'\" /run/losos-identity/session.yml")
        box.wait_until_succeeds("systemctl is-active authelia-losos.service")
        box.wait_for_open_port(9091)

    with subtest("the box follows the daily channel by default"):
        # Before channels a box followed main, whose head is daily's here.
        branch = "main" if BEFORE else "channel/daily"
        env = box.succeed("systemctl show lososd -p Environment --value")
        assert "LOSOS_UPSTREAM_URL=file:///srv/losos.git" in env, env
        assert f"LOSOS_UPSTREAM_BRANCH={branch}" in env, env
        up = update()
        print(up)
        assert up["branch"] == branch, up
        assert up["report"]["state"] == "ok", up
        assert up["report"]["remoteHead"] == m3, up
        assert tracked("version") == "three"
        # The box's own file is the box's.
        assert "{ ... }:" in tracked("modules/overrides.nix")
        shots("plan-following")

    with subtest("a slower channel whose cut is older: the box keeps its version"):
        switch("weekly")
        head = box.succeed("git -C /etc/nixos rev-parse HEAD").strip()
        up = update()
        print(up)
        if BEFORE:
            # The box goes back four days.
            assert tracked("version") == "one", up
        else:
            assert up["report"]["state"] == "holding", up
            assert "2026-10-24" in up["report"]["detail"] and "2026-10-28" in up["report"]["detail"], up
            assert tracked("version") == "three"
            # Only the owner's own change was committed, the tree is the same.
            assert box.succeed(f"git -C /etc/nixos diff --stat {head} HEAD -- version").strip() == ""
        shots("plan-holding")

    with subtest("the next weekly cut passes the box, and it moves"):
        m4 = main_commit("Merge pull request #4", "version", "four", T0 + 7 * DAY)
        cut("weekly", m4, T0 + 7 * DAY, T0 + 7 * DAY + 600)
        up = update()
        print(up)
        assert up["report"]["state"] == "ok", up
        assert tracked("version") == "four"

    if not BEFORE:
        with subtest("a monthly channel with a hotfix, then daily again: both are older"):
            switch("monthly")
            up = update()
            print(up)
            assert up["report"]["state"] == "holding", up
            assert tracked("version") == "four"
            box.fail("git -C /etc/nixos cat-file -e HEAD:fix")
            switch("daily")
            up = update()
            assert up["report"]["state"] == "holding", up
            # Daily's next cut is newer: the box takes it.
            m5 = main_commit("Merge pull request #5", "version", "five", T0 + 8 * DAY)
            git(f"push -q origin {m5}:refs/heads/channel/daily")
            up = update()
            assert up["report"]["state"] == "ok", up
            assert tracked("version") == "five"

    laptop.copy_from_machine("/tmp/screens", "")
  '';
}
