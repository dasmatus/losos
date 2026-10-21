# nixos-test-vms config for the AI agent companion (docs/terminal-access.md,
# "The AI agent companion"; handbook/docs/manual/ai-agents.md).
#
# A box with the real lososd, nginx, LosOS Login and the built admin UI,
# and a laptop on the LAN with the real `losos` client, a desktop keychain
# (gnome-keyring on the user's session bus), a browser, and real AI tools
# from nixpkgs: Claude Code, Codex, Gemini CLI, Qwen Code and opencode, plus
# the settings folders of Cursor, VS Code and Zed. It asserts:
#
#   * `losos agents` finds the tools by their programs and their folders;
#   * `losos agents install --yes --host <box>`, on a computer that never
#     ran `losos login`, shows the box's certificate, signs the companion
#     in to LosOS Login with a device code, and the browser on the laptop
#     signs in and approves it on LosOS Login's consent page;
#   * it then puts the companion in each tool, through the tool's own
#     plugin or extension command where it has one, and each tool starts
#     `losos mcp` and lists it as connected;
#   * the command line each tool was given, run as the tool runs it, answers
#     MCP and reads the box as the owner, with the LosOS Login token;
#   * the box holds that token to the companion's few read-only routes, for
#     an administrator too, and the companion sends no write at all;
#   * the companion refreshes its token when it runs out (an access token
#     lives 90 seconds here) and keeps the new one;
#   * Gemini CLI is told to trust the companion's folder and nothing else;
#   * `losos agents remove --yes` puts every settings file back as it was,
#     hands both tokens back to LosOS Login, which then refuses them, and
#     the keychain forgets the companion.
#
# The browser photographs LosOS Login's consent page and the page after it
# for the pull request (screens/).
{ pkgs }:

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
  name = "losos-agents";

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
          forgejo.enable = false;
        };
        systemd.services.lososd.path = lib.mkForce [
          pkgs.coreutils
        ];
        # An hour is long to wait for in a test: the companion's access
        # token lives 90 seconds here, so the test sees it refreshed.
        services.authelia.instances.losos.settings.identity_providers.oidc.lifespans.custom.companion.access_token =
          lib.mkForce "90s";
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
          pkgs.libsecret
          pkgs.python3
          lososPkgs.losos
          claudeCode
          pkgs.codex
          pkgs.gemini-cli
          pkgs.qwen-code
          pkgs.opencode
        ];
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
    import shlex

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

    def status(args):
        """The HTTP status of one request from the laptop."""
        return int(curl(f"-o /dev/null -w '%{{http_code}}' {args}"))

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

    def kept():
        """The companion's sign-in, as the keychain holds it."""
        return json.loads(me(f"secret-tool lookup service losos kind companion url {URL}"))

    with subtest("claim the box"):
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
        # A keychain on matus's session bus, unlocked, as a desktop has.
        me("printf hunter2 | gnome-keyring-daemon --unlock --replace --daemonize --components=secrets")
        me("printf v | secret-tool store --label=probe service losos-probe kind probe && secret-tool lookup service losos-probe kind probe")

    with subtest("losos agents finds the tools by their programs and their folders"):
        me("mkdir -p ~/.cursor ~/.config/Code ~/.config/zed ~/.config/opencode")
        me("cp /etc/losos-test/zed-settings.json ~/.config/zed/settings.json && chmod 644 ~/.config/zed/settings.json")
        me("mkdir -p ~/work ~/.gemini && cp /etc/losos-test/gemini-trust.json ~/.gemini/trustedFolders.json && chmod 644 ~/.gemini/trustedFolders.json")
        listed = me("losos agents")
        print(listed)
        for name in ["Claude Code", "Codex", "Gemini CLI", "Qwen Code", "opencode", "Cursor", "VS Code", "Zed"]:
            assert name in listed, f"{name} not found:\n{listed}"
        assert "Windsurf" not in listed and "LM Studio" not in listed, listed

    with subtest("the companion signs in to LosOS Login, approved in the browser"):
        # This computer never ran losos login: no key, no access token.
        me(f"setsid nohup losos agents install --yes --host {URL} >/tmp/agents-install.log 2>&1 &")
        laptop.wait_until_succeeds("grep -q 'user_code=' /tmp/agents-install.log", timeout=300)
        log = laptop.succeed("cat /tmp/agents-install.log")
        print(log)
        assert "SHA-256 fingerprint is" in log, log
        link = next(w for w in log.split() if "user_code=" in w)
        assert link.startswith(f"{URL}/auth/consent/openid/device-authorization?user_code="), link
        code = link.rsplit("=", 1)[1]
        assert f"shows the code {code}" in log, log
        page = link[len(URL):]
        plan = {
            "width": 1280,
            "height": 800,
            "shots": [
                {
                    "name": "companion-consent",
                    "path": page,
                    "waitFor": {"text": "LosOS companion"},
                    "settle": 3000,
                },
                {
                    "name": "companion-approved",
                    "path": page,
                    "waitFor": {"text": "LosOS companion"},
                    "steps": [
                        {"click": {"selector": "button:has-text('Accept')"}},
                        {"waitText": "Consent has been accepted and processed"},
                    ],
                },
            ],
        }
        laptop.succeed(f"echo {shlex.quote(json.dumps(plan))} > /tmp/plan.json")
        laptop.succeed(
            f"losos-capture --base {URL} --plan /tmp/plan.json "
            f"--out /tmp/screens --user matus --password '{PASSWORD}'",
            timeout=1800,
        )
        laptop.copy_from_machine("/tmp/screens", "")
        laptop.wait_until_succeeds("grep -q 'takes it out again' /tmp/agents-install.log", timeout=900)
        out = laptop.succeed("cat /tmp/agents-install.log")
        print(out)
        assert "Signed in." in out, out

    with subtest("losos agents install puts the companion in every tool"):
        for name in ["Claude Code", "Codex", "Gemini CLI", "Qwen Code", "opencode", "Cursor", "VS Code", "Zed"]:
            assert f"The LosOS companion is in {name}." in out, f"{name}:\n{out}"
        zed = me("cat ~/.config/zed/settings.json")
        for kept_line in ["// Zed settings", "// the dark one", "/* the owner's own */", '"github"']:
            assert kept_line in zed, zed
        me("ls ~/.local/share/losos/companion/backups/zed/")
        assert "companion in" in me("losos agents")
        signed = kept()
        assert signed["accessToken"].startswith("authelia_at_"), signed
        assert signed["refreshToken"].startswith("authelia_rt_"), signed
        assert signed.get("tlsPin"), signed
        # No AI tool's settings hold a token.
        assert me("grep -rl authelia_ ~/.config ~/.cursor ~/.gemini ~/.qwen ~/.local/share/losos || true").strip() == ""

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
            code, listed = me(f"cd ~/work && timeout 180 {program} 2>&1", check=False)
            print(f"{program}:\n{listed}")
            line = next((l for l in listed.splitlines() if "losos" in l), "")
            assert says in line, f"{program}: {listed}"
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
        found = {
            "Claude Code": read(f"{plugin}/claude-code/plugins/losos/.mcp.json")["mcpServers"]["losos"],
            "Codex": read(f"{plugin}/codex/plugins/losos/.mcp.json")["mcpServers"]["losos"],
            "Gemini CLI": read(f"{home}/.gemini/extensions/losos/gemini-extension.json")["mcpServers"]["losos"],
            "Qwen Code": read(f"{home}/.qwen/extensions/losos/qwen-extension.json")["mcpServers"]["losos"],
            "Cursor": read(f"{home}/.cursor/mcp.json")["mcpServers"]["losos"],
            "VS Code": read(f"{home}/.config/Code/User/mcp.json")["servers"]["losos"],
            "Zed": zed_entry,
        }
        commands = {name: [e["command"], *e["args"]] for name, e in found.items()}
        commands["opencode"] = read(f"{home}/.config/opencode/opencode.json")["mcp"]["losos"]["command"]
        return commands

    def probe(command, calls, env=""):
        got = me(
            f"{env} python3 /etc/losos-test/mcp-probe.py "
            f"{shlex.quote(json.dumps(command))} {shlex.quote(json.dumps(calls))}"
        )
        return json.loads(got)

    with subtest("the command every tool was given reads the box as the owner"):
        commands = entries()
        for name, command in commands.items():
            assert command[1:] == ["mcp", "--host", URL], (name, command)
            got = probe(command, [["whoami", {}]])
            result = got["calls"][0]["result"]
            assert result["isError"] is False, (name, got)
            assert result["structuredContent"]["name"] == "matus", (name, result)
            assert result["structuredContent"]["companion"] is True, (name, result)
            assert result["structuredContent"]["admin"] is False, (name, result)
        names = [t["name"] for t in got["tools"]["result"]["tools"]]
        assert "cloud_search_text" in names and "git_search_code" in names, names
        # Codex starts its servers with HOME, PATH and little else: the
        # companion finds the session bus where systemd keeps it.
        got = probe(commands["Codex"], [["box_status", {}]], env="env -i HOME=/home/matus PATH=$PATH")
        assert got["calls"][0]["result"]["isError"] is False, got
        # LosOS Cloud is not on this box: an answer the agent can explain.
        got = probe(commands["Zed"], [["cloud_list_folder", {"path": "/"}]])
        print(got["calls"][0])

    with subtest("the box holds the companion's token to its read-only routes"):
        for path in ["/api/me", "/api/status", "/api/git/repos"]:
            code, said = me(f"losos agents call GET {path} 2>&1", check=False)
            assert "403" not in said.splitlines()[0], (path, said)
        for path in ["/api/state", "/api/settings", "/api/access", "/api/people"]:
            code, said = me(f"losos agents call GET {path} 2>&1", check=False)
            assert code != 0 and said.splitlines()[0].strip() == "403", (path, said)
        code, said = me("losos agents call POST /api/cloud/files 2>&1", check=False)
        assert code != 0 and "only reads" in said, said
        # The box itself, asked with the token and no companion in between:
        # the owner is an administrator, the companion is not.
        bearer = f"-H 'Authorization: Bearer {kept()['accessToken']}'"
        assert status(f"{bearer} {URL}/api/me") == 200
        for method, path in [("POST", "/api/apply"), ("POST", "/api/factory-reset"), ("GET", "/api/settings"), ("POST", "/api/people")]:
            got = status(f"-X {method} {bearer} -H 'Content-Type: application/json' -d '{{}}' {URL}{path}")
            assert got == 403, (method, path, got)
        assert status(f"-H 'Authorization: Bearer authelia_at_made.up' {URL}/api/me") == 401

    with subtest("the companion refreshes its token when it runs out"):
        before = kept()
        # 90 seconds of life, refreshed in its last minute: wait past that.
        laptop.sleep(45)
        got = probe(entries()["Cursor"], [["whoami", {}]])
        assert got["calls"][0]["result"]["isError"] is False, got
        after = kept()
        assert after["accessToken"] != before["accessToken"], (before, after)
        assert after["refreshToken"] != before["refreshToken"], (before, after)

    with subtest("losos agents remove puts everything back and signs the companion out"):
        signed = kept()
        out = me("losos agents remove --yes 2>&1")
        print(out)
        assert f"signed out of {URL}" in out, out
        me("cmp ~/.config/zed/settings.json /etc/losos-test/zed-settings.json")
        me("cmp ~/.gemini/trustedFolders.json /etc/losos-test/gemini-trust.json")
        me("test ! -e ~/.gemini/extensions/losos && test ! -e ~/.qwen/extensions/losos")
        assert "losos" not in me("cat ~/.cursor/mcp.json"), me("cat ~/.cursor/mcp.json")
        assert "companion in" not in me("losos agents")
        code, _ = me("secret-tool search service losos kind companion 2>&1 | grep -q companion", check=False)
        assert code != 0, "the companion is still in the keychain"
        code, listed = me("timeout 180 claude mcp list 2>&1", check=False)
        assert "losos" not in listed, listed
        # LosOS Login refuses the refresh token at once, and lososd the
        # access token once its minute of trust is over.
        refresh = curl(
            f"-X POST {URL}/auth/api/oidc/token -d grant_type=refresh_token "
            f"-d client_id=losos-companion -d refresh_token={signed['refreshToken']}"
        )
        assert "invalid_grant" in refresh, refresh
        laptop.wait_until_succeeds(
            f"test $(curl -sk -o /dev/null -w '%{{http_code}}' -H 'Authorization: Bearer {signed['accessToken']}' {URL}/api/me) = 401",
            timeout=180,
        )
  '';
}
