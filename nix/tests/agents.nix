# nixos-test-vms config for the AI agent companion (docs/terminal-access.md,
# "The AI agent companion"; handbook/docs/manual/ai-agents.md).
#
# A box with the real lososd, nginx, LosOS Login and the built admin UI,
# and a laptop on the LAN with the real `losos` client, a desktop keychain
# (gnome-keyring on the user's session bus) and real AI tools from nixpkgs:
# Claude Code, Codex, Gemini CLI, Qwen Code and opencode, plus the settings
# folders of Cursor, VS Code and Zed. It asserts:
#
#   * `losos agents` finds the tools by their programs and their folders;
#   * `losos agents install --yes` puts the companion in each, through the
#     tool's own plugin or extension command where it has one, and each
#     tool then starts `losos mcp` and lists it as connected;
#   * the command line each tool was given, run as the tool runs it, answers
#     MCP and reads the box as the owner, with the companion's own key;
#   * the box holds that key to its token: an AI agents token reaches only
#     its few read-only routes, does nothing signed by the owner's own key,
#     and once revoked the companion is refused;
#   * Gemini CLI is told to trust the companion's folder and nothing else;
#   * `losos agents remove --yes` puts every settings file back as it was
#     and the keychain forgets the companion.
#
# The laptop also photographs the Access tokens pane for the pull request
# (screens/). With `shotsOnly`, only that part runs, so the same pictures
# can be taken of an older admin UI.
{
  pkgs,
  shotsOnly ? false,
}:

let
  inherit (pkgs) lib;
  lososPkgs = import ../packages.nix { inherit pkgs; };
  screens = import ./screens { inherit pkgs; };

  # Claude Code is unfree; this test needs nothing else from that import.
  claudeCode =
    (import pkgs.path {
      inherit (pkgs.stdenv.hostPlatform) system;
      config.allowUnfreePredicate = p: lib.getName p == "claude-code";
    }).claude-code;

  stubOcc = pkgs.writeShellScriptBin "nextcloud-occ" ''
    case "$1" in
      status) echo '{"installed":true,"maintenance":false,"needsDbUpgrade":false}' ;;
      user:resetpassword) echo "Successfully reset password for $2" ;;
      *) echo "stub occ: $*" >&2; exit 1 ;;
    esac
  '';
  standIn =
    name:
    pkgs.hello
    // {
      imageName = name;
      imageTag = "test";
    };

  # The Access tokens pane: the form with the kind of token chosen, and the
  # token it made. `scope` is the one picked; an older pane has no AI agents.
  plan = pkgs.writeText "plan.json" (
    builtins.toJSON {
      width = 1280;
      height = 800;
      shots = [
        {
          name = "access-form";
          path = "/settings/access";
          fullPage = true;
          waitFor = {
            role = "button";
            name = "Create token";
          };
          steps = [
            {
              fill = {
                label = "Name";
                value = "AI tools";
              };
            }
          ]
          ++ lib.optional (!shotsOnly) {
            select = {
              label = "Access";
              value = "agent";
            };
          };
        }
        {
          name = "access-created";
          path = "/settings/access";
          waitFor = {
            role = "button";
            name = "Create token";
          };
          steps = [
            {
              fill = {
                label = "Name";
                value = "AI tools";
              };
            }
          ]
          ++ lib.optional (!shotsOnly) {
            select = {
              label = "Access";
              value = "agent";
            };
          }
          ++ [
            {
              click = {
                role = "button";
                name = "Create token";
              };
            }
            { waitText = "Token created"; }
          ];
        }
      ];
    }
  );

  # A small MCP client: starts one server as a tool would, from the
  # command and arguments the tool was given, and makes a few calls.
  probe = pkgs.writeText "mcp-probe.py" ''
    import json, subprocess, sys

    command = json.loads(sys.argv[1])
    calls = json.loads(sys.argv[2])
    server = subprocess.Popen(
        command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True
    )

    def ask(id, method, params=None):
        msg = {"jsonrpc": "2.0", "id": id, "method": method}
        if params is not None:
            msg["params"] = params
        server.stdin.write(json.dumps(msg) + "\n")
        server.stdin.flush()
        return json.loads(server.stdout.readline())

    out = {"init": ask(0, "initialize", {
        "protocolVersion": "2025-06-18", "capabilities": {},
        "clientInfo": {"name": "probe", "version": "1"}})}
    server.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
    out["tools"] = ask(1, "tools/list")
    out["calls"] = [
        ask(2 + i, "tools/call", {"name": name, "arguments": args})
        for i, (name, args) in enumerate(calls)
    ]
    server.stdin.close()
    server.wait(timeout=30)
    print(json.dumps(out))
  '';

  # One value out of a JSON file with comments and trailing commas, which
  # Zed's settings have: the comments go, strings and all they hold stay.
  jsoncRead = pkgs.writeText "jsonc.py" ''
    import json, re, sys

    text, out, i = open(sys.argv[1]).read(), [], 0
    while i < len(text):
        c = text[i]
        if c == '"':
            j = i + 1
            while text[j] != '"':
                j += 2 if text[j] == "\\" else 1
            out.append(text[i : j + 1])
            i = j + 1
        elif text.startswith("//", i):
            i = text.find("\n", i)
            i = len(text) if i < 0 else i
        elif text.startswith("/*", i):
            i = text.index("*/", i) + 2
        else:
            out.append(c)
            i += 1
    value = json.loads(re.sub(r",(\s*[}\]])", r"\1", "".join(out)))
    for key in sys.argv[2:]:
        value = value[key]
    print(json.dumps(value))
  '';

  # Zed's settings as a person keeps them, comments and all.
  zedSettings = pkgs.writeText "zed-settings.json" ''
    // Zed settings
    {
      "theme": "One Dark", // the dark one
      "context_servers": {
        /* the owner's own */
        "github": { "command": "gh-mcp", "args": [] },
      },
    }
  '';
in

pkgs.testers.nixosTest {
  name = if shotsOnly then "losos-agents-screens" else "losos-agents";

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
          ../../modules/workloads.nix
        ];
        losos = {
          hostName = "mattbox";
          backend.package = lososPkgs.losos-ctl;
          admin.ui = lososPkgs.losos-admin-ui;
          nextcloud.mode = "container";
          forgejo.enable = false;
          workloads.nextcloudImage = standIn "losos-nextcloud-stand-in";
        };
        systemd.services.lososd.path = lib.mkForce [
          stubOcc
          pkgs.coreutils
        ];
        systemd.services.lososd.environment = {
          LOSOS_NEXTCLOUD_MODE = lib.mkForce "native";
          # The owner's key is made on the laptop when the test runs, so it
          # is seeded here then, the way the installer would have.
          LOSOS_ACCESS_KEYS_FILE = lib.mkForce "/var/lib/losos-test/keys.asc";
        };
        systemd.tmpfiles.rules = [ "d /var/lib/losos-test 0755 root root -" ];
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
        users.users.matus = {
          isNormalUser = true;
          # Its session bus and keychain are up without anyone logging in.
          linger = true;
        };
        services.gnome.gnome-keyring.enable = true;
        environment.systemPackages = [
          pkgs.curl
          pkgs.jq
          pkgs.gnupg
          pkgs.libsecret
          pkgs.python3
        ]
        ++ lib.optionals (!shotsOnly) [
          lososPkgs.losos
          claudeCode
          pkgs.codex
          pkgs.gemini-cli
          pkgs.qwen-code
          pkgs.opencode
        ];
        environment.etc."losos-test/plan.json".source = plan;
        environment.etc."losos-test/mcp-probe.py".source = probe;
        environment.etc."losos-test/zed-settings.json".source = zedSettings;
        environment.etc."losos-test/jsonc.py".source = jsoncRead;
        environment.etc."losos-test/gemini-trust.json".text = builtins.toJSON {
          "/home/matus/work" = "TRUST_FOLDER";
        };
        virtualisation = {
          memorySize = lib.mkForce 3072;
          vlans = [ 1 ];
        };
      };
  };

  testScript = ''
    import json
    import re
    import shlex

    SHOTS_ONLY = ${if shotsOnly then "True" else "False"}
    BOX = "192.168.1.1"
    URL = f"https://{BOX}"
    PASSWORD = "Correct-Horse-Battery-9"
    BUS = "unix:path=/run/user/1000/bus"

    start_all()
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("nginx.service")
    box.wait_for_open_port(443)
    laptop.wait_for_unit("multi-user.target")
    laptop.wait_for_unit("user@1000.service")

    def curl(args):
        return laptop.succeed(f"curl -sk {args}")

    def api(path, cookie, method="GET", data=None):
        args = f"-X {method} {URL}{path} -H 'Cookie: {cookie}'"
        if data is not None:
            args += f" -H 'Content-Type: application/json' -d '{json.dumps(data)}'"
        out = curl(f"-w '\\n%{{http_code}}' {args}")
        body, code = out.rsplit("\n", 1)
        return int(code), body

    def me(cmd, stdin=None, check=True, timeout=900):
        """A command as matus, on his session bus, as his terminal would run it."""
        script = f"export DBUS_SESSION_BUS_ADDRESS={BUS}; cd; {cmd}"
        if stdin is not None:
            script = f"printf '%s\\n' {shlex.quote(stdin)} | ({cmd})"
            script = f"export DBUS_SESSION_BUS_ADDRESS={BUS}; cd; {script}"
        full = f"su - matus -c {shlex.quote(script)}"
        if check:
            return laptop.succeed(full, timeout=timeout)
        return laptop.execute(full, timeout=timeout)

    with subtest("the owner's key, seeded on the box"):
        me("gpg --batch --passphrase ''' --quick-gen-key 'Matus <matus@example.org>' default default never")
        owner_fpr = me("gpg --with-colons --fingerprint matus@example.org | awk -F: '/^fpr/ {print $10; exit}'").strip()
        public = me("gpg --armor --export matus@example.org")
        box.succeed(f"cat > /var/lib/losos-test/keys.asc <<'EOF'\n{public}\nEOF")
        box.succeed("systemctl restart lososd.service")
        box.wait_until_succeeds("curl -fs localhost:8082/api/health")

    with subtest("claim the box and sign in"):
        claim = json.dumps({"name": "matus", "password": PASSWORD})
        out = curl(
            f"-w '\\n%{{http_code}}' -X POST {URL}/api/setup/claim -H 'Host: {BOX}' "
            f"-H 'Content-Type: application/json' -d '{claim}'"
        )
        body, code = out.rsplit("\n", 1)
        assert code == "200", (code, body)
        box.wait_for_unit("authelia-losos.service")
        box.systemctl("start losos-identity-setup.service")
        box.wait_until_succeeds(f"grep -q \"domain: '{BOX}'\" /run/losos-identity/session.yml")
        box.wait_until_succeeds("systemctl is-active authelia-losos.service")
        box.wait_for_open_port(9091)
        first = json.dumps({"username": "matus", "password": PASSWORD})
        head = curl(
            f"-D - -o /dev/null {URL}/auth/api/firstfactor "
            f"-H 'Content-Type: application/json' -d '{first}'"
        )
        found = re.search(r"(?im)^set-cookie: (losos_session=[^;]+)", head)
        assert found, head
        cookie = found.group(1)

    with subtest("the Access tokens pane, photographed"):
        laptop.succeed(
            f"losos-capture --base {URL} --plan /etc/losos-test/plan.json "
            f"--out /tmp/screens --user matus --password '{PASSWORD}'",
            timeout=1800,
        )
        laptop.copy_from_machine("/tmp/screens", "")

    if not SHOTS_ONLY:
        def token(scope, name):
            code, body = api(
                "/api/access/tokens", cookie, "POST",
                {"name": name, "fingerprint": owner_fpr, "scope": scope, "expiresInDays": 90},
            )
            assert code == 200, (code, body)
            made = json.loads(body)
            assert made["record"]["scope"] == scope, made
            return made["token"], made["record"]["id"]

        with subtest("the terminal client signs in with a token of the owner's"):
            full, _ = token("full", "laptop")
            agent, agent_id = token("agent", "AI tools")
            # A keychain on matus's session bus, unlocked, as a desktop has.
            me("printf hunter2 | gnome-keyring-daemon --unlock --replace --daemonize --components=secrets")
            me("printf v | secret-tool store --label=probe service losos-probe kind probe && secret-tool lookup service losos-probe kind probe")
            served = json.loads(curl(f"{URL}/api/access/host-key"))
            me("losos key import --gpg matus@example.org")
            print(me(f"losos login {URL} --fingerprint {served['fingerprint']}", stdin=full))
            assert json.loads(me("losos call GET /api/me"))["name"] == "matus"

        with subtest("an AI agents token does nothing signed by the owner's own key"):
            me(f"losos logout {URL}")
            status, out = me(
                f"losos login {URL} --fingerprint {served['fingerprint']} 2>&1 && losos call GET /api/me 2>&1",
                stdin=agent, check=False,
            )
            assert status != 0 and "works only for the LosOS companion" in out, out
            me(f"losos logout {URL} || true")
            me(f"losos login {URL} --fingerprint {served['fingerprint']}", stdin=full)

        with subtest("losos agents finds the tools by their programs and their folders"):
            me("mkdir -p ~/.cursor ~/.config/Code ~/.config/zed ~/.config/opencode")
            me("cp /etc/losos-test/zed-settings.json ~/.config/zed/settings.json && chmod 644 ~/.config/zed/settings.json")
            me("mkdir -p ~/work ~/.gemini && cp /etc/losos-test/gemini-trust.json ~/.gemini/trustedFolders.json && chmod 644 ~/.gemini/trustedFolders.json")
            listed = me("losos agents")
            print(listed)
            for name in ["Claude Code", "Codex", "Gemini CLI", "Qwen Code", "opencode", "Cursor", "VS Code", "Zed"]:
                assert name in listed, f"{name} not found:\n{listed}"
            assert "Windsurf" not in listed and "LM Studio" not in listed, listed

        with subtest("losos agents install puts the companion in every tool"):
            out = me("losos agents install --yes 2>&1", stdin=agent)
            print(out)
            for name in ["Claude Code", "Codex", "Gemini CLI", "Qwen Code", "opencode", "Cursor", "VS Code", "Zed"]:
                assert f"The LosOS companion is in {name}." in out, f"{name}:\n{out}"
            zed = me("cat ~/.config/zed/settings.json")
            for kept in ["// Zed settings", "// the dark one", "/* the owner's own */", '"github"']:
                assert kept in zed, zed
            me("ls ~/.local/share/losos/companion/backups/zed/")
            assert "companion in" in me("losos agents")

        with subtest("each tool starts the companion and sees it answer"):
            # Gemini CLI starts MCP servers only in a folder the owner
            # trusts; the companion's own folder is on its list, and the
            # owner's entry stays.
            trust = json.loads(me("cat ~/.gemini/trustedFolders.json"))
            assert trust.get("/home/matus/work") == "TRUST_FOLDER", trust
            assert "/home/matus/.local/share/losos/companion/gemini" in trust, trust
            for program, says in [
                ("claude mcp list", "Connected"),
                ("gemini mcp list", "Connected"),
                ("qwen mcp list", "Connected"),
                ("opencode mcp list", "connected"),
            ]:
                status, out = me(f"cd ~/work && timeout 180 {program} 2>&1", check=False)
                print(f"{program}:\n{out}")
                line = next((l for l in out.splitlines() if "losos" in l), "")
                assert says in line, f"{program}: {out}"
            print(me("timeout 180 codex mcp list 2>&1 || true"))

        def entries():
            """The command each tool was given, from the file it reads."""
            home = "/home/matus"
            plugin = f"{home}/.local/share/losos/companion"
            def read(path):
                return json.loads(me(f"cat {path}"))
            zed_entry = json.loads(me(
                "python3 /etc/losos-test/jsonc.py ~/.config/zed/settings.json context_servers losos"
            ))
            out = {
                "Claude Code": read(f"{plugin}/claude-code/plugins/losos/.mcp.json")["mcpServers"]["losos"],
                "Codex": read(f"{plugin}/codex/plugins/losos/.mcp.json")["mcpServers"]["losos"],
                "Gemini CLI": read(f"{home}/.gemini/extensions/losos/gemini-extension.json")["mcpServers"]["losos"],
                "Qwen Code": read(f"{home}/.qwen/extensions/losos/qwen-extension.json")["mcpServers"]["losos"],
                "Cursor": read(f"{home}/.cursor/mcp.json")["mcpServers"]["losos"],
                "VS Code": read(f"{home}/.config/Code/User/mcp.json")["servers"]["losos"],
                "Zed": zed_entry,
            }
            commands = {name: [e["command"], *e["args"]] for name, e in out.items()}
            commands["opencode"] = read(f"{home}/.config/opencode/opencode.json")["mcp"]["losos"]["command"]
            return commands

        def probe(command, calls, env=""):
            out = me(
                f"{env} python3 /etc/losos-test/mcp-probe.py "
                f"{shlex.quote(json.dumps(command))} {shlex.quote(json.dumps(calls))}"
            )
            return json.loads(out)

        with subtest("the command every tool was given reads the box as the owner"):
            commands = entries()
            for name, command in commands.items():
                assert command[1:] == ["mcp", "--host", URL], (name, command)
                got = probe(command, [["whoami", {}]])
                result = got["calls"][0]["result"]
                assert result["isError"] is False, (name, got)
                assert result["structuredContent"]["name"] == "matus", (name, result)
            names = [t["name"] for t in got["tools"]["result"]["tools"]]
            assert "cloud_search_text" in names and "git_search_code" in names, names
            # Codex starts its servers with HOME, PATH and little else: the
            # companion finds the session bus where systemd keeps it.
            got = probe(commands["Codex"], [["box_status", {}]], env="env -i HOME=/home/matus PATH=$PATH")
            assert got["calls"][0]["result"]["isError"] is False, got
            # LosOS Cloud is not on this box: an answer the agent can explain.
            got = probe(commands["Zed"], [["cloud_list_folder", {"path": "/"}]])
            print(got["calls"][0])

        with subtest("the companion's key reaches only the AI agents routes"):
            for path in ["/api/me", "/api/status", "/api/git/repos"]:
                status, out = me(f"losos agents call GET {path} 2>&1", check=False)
                assert "403" not in out.splitlines()[0], (path, out)
            for method, path in [("GET", "/api/state"), ("GET", "/api/settings"), ("POST", "/api/cloud/files"), ("GET", "/api/access")]:
                status, out = me(f"losos agents call {method} {path} 2>&1", check=False)
                assert status != 0, (method, path, out)
                assert out.splitlines()[0].strip() in ("403", "401"), (method, path, out)

        with subtest("a revoked token turns the companion away"):
            code, body = api("/api/access/tokens/revoke", cookie, "POST", {"id": agent_id})
            assert code == 200, (code, body)
            got = probe(entries()["Cursor"], [["whoami", {}]])
            result = got["calls"][0]["result"]
            assert result["isError"] is True, got
            print(result["content"][0]["text"])

        with subtest("losos agents remove puts everything back"):
            out = me("losos agents remove --yes 2>&1")
            print(out)
            assert "Revoke the AI agents token" in out, out
            me("cmp ~/.config/zed/settings.json /etc/losos-test/zed-settings.json")
            me("cmp ~/.gemini/trustedFolders.json /etc/losos-test/gemini-trust.json")
            me("test ! -e ~/.gemini/extensions/losos && test ! -e ~/.qwen/extensions/losos")
            assert "losos" not in me("cat ~/.cursor/mcp.json"), me("cat ~/.cursor/mcp.json")
            assert "companion in" not in me("losos agents")
            status, _ = me("secret-tool search service losos kind companion 2>&1 | grep -q companion", check=False)
            assert status != 0, "the companion is still in the keychain"
            status, out = me("timeout 180 claude mcp list 2>&1", check=False)
            assert "losos" not in out, out
  '';
}
