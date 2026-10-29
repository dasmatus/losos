# nixos-test-vms config for lososd's plugins (backend/plugins/README.md).
#
# A box with the real lososd, the real plugin package from the store, nginx,
# LosOS Login and the built admin UI, and a laptop on the LAN with a
# browser. It asserts what only a booted box can show:
#
#   * lososd loads `.#losos-plugins` from the store, which is a read-only
#     mount, and serves the look from it: through nginx and LosOS Login,
#     the box's own background where it always was and the owner's copy of
#     it in `looks/`;
#   * the same lososd binary refuses to start on a plugin directory outside
#     the store, a writable one, or one whose files are not root's, and a
#     release build ignores the debug-only stand-in for the store;
#   * a plugin that tries what its manifest does not grant (the probe, in a
#     second store package) is answered `denied`, and one that spins or
#     panics costs a 500, not the daemon;
#   * the look pane, in a browser on the laptop, works as before.
#
# The laptop's pictures are the ones a pull request shows: the derivation's
# output holds them (screens/).
{ pkgs }:

let
  lososPkgs = import ../packages.nix { inherit pkgs; };
  screens = import ./screens { inherit pkgs; };

  # What the laptop photographs: the look pane as it comes, with a shipped
  # picture chosen, and the home screen under that picture, its sidebar
  # carrying the pins the test put there.
  plan = pkgs.writeText "plan.json" (
    builtins.toJSON {
      width = 1280;
      height = 800;
      shots = [
        {
          name = "look-plain";
          path = "/settings/look";
          waitFor = {
            role = "radiogroup";
            name = "Your background";
          };
        }
        {
          name = "look-tide";
          path = "/settings/look";
          waitFor = {
            role = "radiogroup";
            name = "Your background";
          };
          steps = [
            {
              click = {
                role = "radio";
                name = "Tide";
              };
            }
            { waitText = "Background set."; }
            { sleep = 4500; }
          ];
        }
        {
          name = "home-tide";
          path = "/";
          settle = 1500;
        }
      ];
    }
  );
in

pkgs.testers.nixosTest {
  name = "losos-plugins";

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
        ];
        losos = {
          hostName = "mattbox";
          backend.package = lososPkgs.losos-ctl;
          backend.plugins = lososPkgs.losos-plugins;
          admin.ui = lososPkgs.losos-admin-ui;
          git.enable = false;
        };
        systemd.services.lososd.path = lib.mkForce [ pkgs.coreutils ];
        # A second package with the probe in it, for the refusals below. In
        # the store like the real one, and referenced so it is in the VM's.
        environment.etc."losos-test/probe-plugins".source = lososPkgs.losos-plugins-probe;
        environment.systemPackages = [ pkgs.jq ];
        virtualisation = {
          memorySize = 1536;
          cores = 2;
          vlans = [ 1 ];
        };
      };

    laptop =
      { ... }:
      {
        imports = [ screens.browser ];
        environment.systemPackages = [
          pkgs.curl
          pkgs.jq
        ];
        environment.etc."losos-test/plan.json".source = plan;
        virtualisation.vlans = [ 1 ];
      };
  };

  testScript = ''
    import json
    import re

    BOX = "192.168.1.1"
    PASSWORD = "Correct-Horse-Battery-9"
    PLUGINS = "${lososPkgs.losos-plugins}"
    PROBE = "${lososPkgs.losos-plugins-probe}"
    LOSOSD = "${lososPkgs.losos-ctl}/bin/lososd"
    PNG = (
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg=="
    )

    start_all()
    box.wait_for_unit("lososd.service")
    # lososd's unit is up before its API: under emulation loading the
    # plugins takes it a minute, and the claim would meet nginx's 502.
    box.wait_for_open_port(8082)
    box.wait_for_unit("nginx.service")
    box.wait_for_open_port(443)
    laptop.wait_for_unit("multi-user.target")

    def curl(args):
        return laptop.succeed(f"curl -sk {args}")

    def api(path, cookie=None, method="GET", data=None, extra=""):
        args = f"-X {method} https://{BOX}{path} {extra}"
        if cookie:
            args += f" -H 'Cookie: {cookie}'"
        if data is not None:
            args += f" -H 'Content-Type: application/json' -d '{json.dumps(data)}'"
        out = curl(f"-w '\\n%{{http_code}}' {args}")
        body, code = out.rsplit("\n", 1)
        return int(code), body

    def sign_in(name, password):
        body = json.dumps({"username": name, "password": password})
        head = curl(
            f"-D - -o /dev/null https://{BOX}/auth/api/firstfactor "
            f"-H 'Content-Type: application/json' -d '{body}'"
        )
        m = re.search(r"(?im)^set-cookie: (losos_session=[^;]+)", head)
        assert m, f"no session cookie for {name}: {head}"
        return m.group(1)

    def alone(env, port):
        """The same lososd binary on its own port and state, by hand."""
        box.succeed(f"mkdir -p /tmp/lososd-{port}/access")
        return (
            f"env LOSOS_NO_DBUS=1 LOSOS_STATE_DIR=/tmp/lososd-{port} "
            f"LOSOS_ACCESS_DIR=/tmp/lososd-{port}/access "
            f"LOSOS_ADMIN_TOKEN_FILE=/tmp/lososd-{port}/token LOSOS_ADMIN_PORT={port} "
            f"{env} {LOSOSD}"
        )

    def start_alone(env, port):
        box.succeed(f"{alone(env, port)} >/tmp/lososd-{port}/log 2>&1 & echo $! > /tmp/lososd-{port}/pid")
        box.wait_until_succeeds(f"curl -fs localhost:{port}/api/health", timeout=120)

    def stop_alone(port):
        box.succeed(f"kill $(cat /tmp/lososd-{port}/pid)")

    with subtest("lososd loads the plugin package from the read-only store"):
        env = box.succeed("systemctl show lososd -p Environment --value")
        assert f"LOSOS_PLUGIN_DIR={PLUGINS}" in env, env
        for name in ["look", "pins"]:
            box.succeed(f"test -f {PLUGINS}/{name}.wasm -a -f {PLUGINS}/{name}.json")
        box.succeed("findmnt -no OPTIONS /nix/store | tr , '\\n' | grep -qx ro")
        box.wait_until_succeeds("journalctl -u lososd | grep -q 'plugin loaded.*look'")
        box.wait_until_succeeds("journalctl -u lososd | grep -q 'plugin loaded.*pins'")

    with subtest("claim the box and sign in"):
        code, body = api(
            "/api/setup/claim",
            method="POST",
            data={"name": "matus", "password": PASSWORD},
            extra=f"-H 'Host: {BOX}'",
        )
        assert code == 200, (code, body)
        spare = json.loads(body)["token"]
        box.wait_for_unit("authelia-losos.service")
        box.systemctl("start losos-identity-setup.service")
        box.wait_until_succeeds(f"grep -q \"domain: '{BOX}'\" /run/losos-identity/session.yml")
        box.wait_until_succeeds("systemctl is-active authelia-losos.service")
        box.wait_for_open_port(9091)
        cookie = sign_in("matus", PASSWORD)

    with subtest("the look is served by its plugin, through nginx and LosOS Login"):
        code, _ = api("/api/look")
        assert code == 401, code
        bearer = f"-H 'Authorization: Bearer {spare}'"
        laptop.succeed(f"echo {PNG} | base64 -d > /tmp/bg.png")
        # The box's own background is the spare key's, in the files where
        # lososd always kept it.
        code, body = api(
            "/api/look/background",
            method="PUT",
            extra=f"{bearer} -H 'Content-Type: image/png' --data-binary @/tmp/bg.png",
        )
        assert code == 200, (code, body)
        assert json.loads(body)["personal"] is False, body
        box.succeed("cmp /var/lib/losos/background.img <(base64 -d <<< " + PNG + ")")
        # The owner's first look is a copy of it, theirs from then on.
        code, body = api("/api/look", cookie)
        assert code == 200, (code, body)
        look = json.loads(body)
        assert look["personal"] is True, look
        assert look["background"]["url"] == "/api/look/background/matus?v=1", look
        assert look["shipped"] == ["tide", "grid", "dusk"], look
        code, _ = api("/api/look/background/matus?v=1", cookie, extra="-o /tmp/got.png")
        assert code == 200, code
        laptop.succeed("cmp /tmp/bg.png /tmp/got.png")
        code, body = api("/api/look", cookie, "POST", {"veil": 35})
        assert code == 200 and json.loads(body)["veil"] == 35, (code, body)
        on_disk = json.loads(box.succeed("cat /var/lib/losos/looks/matus.json"))
        assert on_disk["veil"] == 35, on_disk
        box.succeed("test $(stat -c %a /var/lib/losos/looks/matus.json) = 600")
        assert json.loads(box.succeed("cat /var/lib/losos/look.json"))["veil"] == 60
        # Dropping theirs leaves the box's.
        code, body = api("/api/look/background", cookie, "DELETE")
        assert code == 200 and json.loads(body)["background"]["kind"] == "none", body
        box.fail("test -e /var/lib/losos/looks/matus.img")
        box.succeed("test -e /var/lib/losos/background.img")
        code, body = api("/api/look/background", method="DELETE", extra=bearer)
        assert code == 200 and json.loads(body)["background"]["kind"] == "none", body
        box.fail("test -e /var/lib/losos/background.img")
        # The veil back to its default for the pictures below.
        code, body = api("/api/look", cookie, "POST", {"veil": 60})
        assert code == 200 and json.loads(body)["veil"] == 60, (code, body)

    with subtest("the pins are served by their plugin"):
        code, body = api("/api/pins", cookie)
        assert code == 200 and json.loads(body) == {"pins": [], "limit": 24}, (code, body)
        pins = [
            {"kind": "tile", "app": "files"},
            {"kind": "tile", "app": "calendar"},
            {"kind": "repo", "owner": "matus", "name": "site"},
            {"kind": "file", "id": 42, "path": "/Documents/plan.pdf", "folder": False},
        ]
        code, body = api("/api/pins", cookie, "PUT", {"pins": pins})
        assert code == 200 and json.loads(body)["pins"] == pins, (code, body)
        code, body = api("/api/pins", cookie, "PUT", {"pins": pins + pins[:1]})
        assert code == 400 and "pinned already" in body, (code, body)
        # Where the list always was; the pictures below show it.
        on_disk = json.loads(box.succeed("cat /var/lib/losos/pins.json"))
        assert on_disk["pins"] == pins, on_disk

    with subtest("lososd will not start on plugins from anywhere but the store"):
        # A copy outside the store, sealed like a store path.
        box.succeed(
            f"mkdir -p /var/tmp/plugins && cp {PLUGINS}/* /var/tmp/plugins/ "
            "&& chmod 444 /var/tmp/plugins/* && chmod 555 /var/tmp/plugins"
        )
        for port, env, why in [
            (9101, "LOSOS_PLUGIN_DIR=/var/tmp/plugins", "outside /nix/store"),
            # The debug-only stand-in for the store means nothing to the
            # build the box runs.
            (
                9102,
                "LOSOS_PLUGIN_DIR=/var/tmp/plugins LOSOS_PLUGIN_TRUST_ROOT=/var/tmp",
                "outside /nix/store",
            ),
            (9103, "LOSOS_PLUGIN_DIR=/nonexistent", "resolving"),
        ]:
            status, out = box.execute(f"timeout 60 {alone(env, port)} 2>&1")
            print(out)
            assert status not in (0, 124), f"lososd started with {env}: {out}"
            assert why in out, f"{env}: expected {why!r} in {out!r}"
        # On the store package it starts.
        start_alone(f"LOSOS_PLUGIN_DIR={PLUGINS}", 9104)
        stop_alone(9104)

    with subtest("a plugin gets only what its manifest grants, and cannot take lososd down"):
        box.succeed("mkdir -p /tmp/lososd-9106/probe/sealed && echo secret > /root/secret")
        start_alone(
            "LOSOS_PROBE_DIR=/tmp/lososd-9106/probe LOSOS_PLUGIN_DIR=/etc/losos-test/probe-plugins",
            9106,
        )
        token = box.succeed("cat /tmp/lososd-9106/token").strip()

        def probe(route, body=None, method="POST"):
            data = f"-d '{json.dumps(body)}'" if body is not None else ""
            out = box.succeed(
                f"curl -s -w '\\n%{{http_code}}' -X {method} localhost:9106{route} "
                f"-H 'Authorization: Bearer {token}' -H 'Content-Type: application/json' {data}"
            )
            text, code = out.rsplit("\n", 1)
            return int(code), text

        code, text = probe("/api/probe/read", {"path": "/root/secret"})
        assert code == 200 and "denied" in text, (code, text)
        code, text = probe("/api/probe/read", {"path": "/etc/shadow"})
        assert code == 200 and "denied" in text, (code, text)
        code, text = probe("/api/probe/write", {"path": "/tmp/lososd-9106/probe/sealed/x", "data": "x"})
        assert code == 200 and "denied" in text, (code, text)
        code, text = probe("/api/probe/exec", {"program": "id"})
        assert code == 200 and "denied" in text, (code, text)
        code, text = probe("/api/probe/exec", {"program": "echo", "args": ["hi"]})
        assert code == 200 and "hi" in text, (code, text)
        code, text = probe("/api/probe/http", {"url": "http://127.0.0.1:8082/api/state"})
        assert code == 200 and "denied" in text, (code, text)
        for runaway in ["spin", "grow", "panic"]:
            code, text = probe(f"/api/probe/{runaway}", method="GET")
            assert code == 500, (runaway, code, text)
        box.succeed("curl -fs localhost:9106/api/health")
        code, text = probe("/api/probe/echo", method="GET")
        assert code == 200, (code, text)
        stop_alone(9106)

    with subtest("the look pane in a browser on the laptop"):
        laptop.succeed(
            f"losos-capture --base https://{BOX} --plan /etc/losos-test/plan.json "
            f"--out /tmp/screens --user matus --password '{PASSWORD}'",
            timeout=1800,
        )
        laptop.copy_from_machine("/tmp/screens", "")
        code, body = api("/api/look", cookie)
        assert json.loads(body)["background"] == {"kind": "shipped", "name": "tide"}, body
  '';
}
