# nixos-test-vms config for LosOS Git between two boxes on one network
# (backend/src/forge/matrix.rs, backend/src/git_rooms.rs, modules/chat.nix,
# admin-ui/app/src/git/, admin-ui/app/src/chat/vouch.tsx).
#
# mattbox (Matus) and kitchenbox (Eva) each share their storage in a mesh,
# so each runs LosOS Chat, and their chat servers federate over the LAN:
# .local names, self-signed certificates, nothing outside the network.
# mattbox runs LosOS Git with a public repository, notshared/site. What only
# two booted boxes can show:
#
#   * Eva has no account on mattbox. Her own box hands her a Matrix OpenID
#     token (POST /api/chat/openid), and mattbox's LosOS Git checks it with
#     kitchenbox's chat server over the LAN and makes her
#     eva@kitchenbox.local, with a token of her own. A token kitchenbox does
#     not know, or one for mattbox's own server, gets nothing;
#   * she clones the public repository, may not push to main or to a branch
#     of anyone else's name, pushes eva@kitchenbox.local/hours, opens a pull
#     request and comments; she cannot merge;
#   * Matus turns the repository's room on from its settings: lososd makes
#     the room as him, with LosOS Git's account in it, and every push, pull
#     request, comment and check lands there as a notice;
#   * Eva joins that room from kitchenbox's chat, over federation, reads the
#     notices there, and sees the merge arrive when Matus merges.
#
# A laptop on the LAN carries git and Chromium (nix/tests/git-federation-shots.mjs),
# which pictures both people's pages. With LOSOS_GIT_OLD_UI (a
# losos-admin-ui from before), the same pages before this change first.
{ pkgs }:

let
  inherit (pkgs) lib;
  lososPkgs = import ../packages.nix { inherit pkgs; };

  # The admin UI as it ships, without LosOS Diagrams' WebAssembly, which
  # takes longer to build than the test to run: stand-ins take the place of
  # the three packages, so the bundle builds without Bevy, and only
  # /diagrams, which no step here opens, would fail.
  ui = lososPkgs.losos-admin-ui.overrideAttrs (_: {
    pname = "losos-admin-ui-without-lab";
    preBuild = ''
      for f in core-pkg/losos_lab_core virt-pkg/losos_lab_virt \
               render-pkg/webgpu/losos_lab_render render-pkg/webgl2/losos_lab_render; do
        mkdir -p "src/lab/$(dirname "$f")"
        echo 'export default function init() { throw new Error("not in this build"); } export class Lab {}' > "src/lab/$f.js"
        : > "src/lab/''${f}_bg.wasm"
      done
      cat > vite.federation-test.config.ts <<'EOF'
      import base from "./vite.config";
      const root = import.meta.dirname;
      const config = base as Record<string, any>;
      config.build.rolldownOptions.input = {
        main: `''${root}/index.html`,
        chat: `''${root}/chat/index.html`,
        git: `''${root}/git/index.html`,
      };
      export default config;
      EOF
    '';
    buildPhase = ''
      runHook preBuild
      bun --bun vite build --config vite.federation-test.config.ts
      runHook postBuild
    '';
  });

  shots = pkgs.writeShellScriptBin "federation-shots" ''
    export PLAYWRIGHT_CORE=${pkgs.playwright-driver}/index.mjs
    export PLAYWRIGHT_BROWSERS_PATH=${
      pkgs.playwright-driver.browsers.override {
        withFirefox = false;
        withWebkit = false;
      }
    }
    export PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS=true
    exec ${pkgs.bun}/bin/bun ${./git-federation-shots.mjs} "$@"
  '';

  # Each box knows the other two machines by name: the names a box has on
  # a LAN, without waiting on multicast.
  names =
    nodes:
    lib.concatMapStringsSep "\n" (n: "${nodes.${n}.networking.primaryIPAddress} ${n}.local") [
      "mattbox"
      "kitchenbox"
    ];

  box =
    name:
    { nodes, ... }:
    {
      imports = [
        ../../modules/options.nix
        ../../modules/containers.nix
        ../../modules/tls.nix
        ../../modules/daemon.nix
        ../../modules/config-repo.nix
        ../../modules/identity.nix
        ../../modules/chat.nix
        ../../modules/git.nix
      ];
      losos = {
        hostName = name;
        backend.package = lososPkgs.losos-ctl;
        admin.ui = ui;
        git.enable = name == "mattbox";
        # The shared user's side of the box: chat's gate.
        sharingMyStorage = true;
        cluster.enable = true;
      };
      networking.hostName = name;
      networking.extraHosts = names nodes;
      # configuration.nix's shared user, without the rest of it.
      users.users.shared = {
        isNormalUser = true;
        uid = 1001;
        group = "shared";
        homeMode = "700";
      };
      users.groups.shared.gid = 1001;
      environment.systemPackages = [
        pkgs.jq
        pkgs.curl
        pkgs.sqlite
        pkgs.gitMinimal
      ];
      virtualisation = {
        memorySize = 2048;
        cores = 2;
        diskSize = 4096;
        vlans = [ 1 ];
      };
    };
in

pkgs.testers.nixosTest {
  name = "losos-git-federation";

  nodes = {
    mattbox = box "mattbox";
    kitchenbox = box "kitchenbox";

    laptop =
      { nodes, ... }:
      {
        networking.extraHosts = names nodes;
        environment.systemPackages = [
          pkgs.curl
          pkgs.jq
          pkgs.gitMinimal
          shots
        ];
        # A refused push fails instead of asking for a password.
        environment.variables.GIT_TERMINAL_PROMPT = "0";
        fonts.packages = [ pkgs.dejavu_fonts ];
        virtualisation = {
          memorySize = 3072;
          cores = 2;
          vlans = [ 1 ];
        };
      };
  };

  testScript = ''
    import json
    import os
    import re
    import time
    import urllib.parse

    MATT = "https://mattbox.local"
    KITCHEN = "https://kitchenbox.local"
    MATUS_PASSWORD = "Correct-Horse-Battery-9"
    EVA_PASSWORD = "Eva-Long-Password-3"
    REPO = "notshared/site"
    EVA = "eva@kitchenbox.local"
    EVA_ID = "@eva:kitchenbox.local"
    OUT = os.environ.get("out", os.getcwd())

    start_all()
    for b in (mattbox, kitchenbox):
        b.wait_for_unit("lososd.service")
        b.wait_for_unit("nginx.service")
        b.wait_for_unit("losos-chat.service")
        b.wait_for_open_port(443)
    mattbox.wait_for_unit("losos-git.service")
    mattbox.wait_for_open_port(3000)

    for b, name in ((mattbox, "mattbox"), (kitchenbox, "kitchenbox")):
        cert = b.succeed("cat /var/lib/losos-tls/cert.pem")
        laptop.succeed(f"cat > /etc/losos-{name}.pem <<'EOF'\n{cert}EOF")
    laptop.succeed(
        "cat /etc/losos-mattbox.pem /etc/losos-kitchenbox.pem > /etc/losos-boxes.pem",
        "git config --system http.sslCAInfo /etc/losos-boxes.pem",
    )
    git_admin = mattbox.succeed("cat /var/lib/losos-git/admin-token").strip()

    def curl(args):
        return laptop.succeed(f"curl -s --cacert /etc/losos-boxes.pem {args}")

    def call(url, cookie=None, method="GET", data=None, token=None, auth=None):
        """(status, JSON or text) of one request from the laptop."""
        args = f"-o /tmp/answer -w '%{{http_code}}' -X {method} '{url}'"
        if cookie:
            args += f" -H 'Cookie: {cookie}'"
        if token:
            args += f" -H 'Authorization: Bearer {token}'"
        if auth:
            args += f" -H 'Authorization: {auth}'"
        if data is not None:
            args += f" -H 'Content-Type: application/json' -d '{json.dumps(data)}'"
        code = int(curl(args))
        out = laptop.succeed("cat /tmp/answer")
        try:
            return code, json.loads(out)
        except ValueError:
            return code, out

    def forge(method, path, body=None, token=None):
        """LosOS Git's API on mattbox's loopback port."""
        args = f"-s -o /tmp/forge -w '%{{http_code}}' -X {method}"
        args += f" -H 'Authorization: token {token or git_admin}'"
        if body is not None:
            args += f" -H 'Content-Type: application/json' -d '{json.dumps(body)}'"
        code = int(mattbox.succeed(f"curl {args} http://127.0.0.1:3000/api/v1{path}"))
        out = mattbox.succeed("cat /tmp/forge")
        try:
            return code, json.loads(out)
        except ValueError:
            return code, out

    def ok(method, path, body=None, token=None):
        code, doc = forge(method, path, body, token)
        assert code in (200, 201, 204), (method, path, code, doc)
        return doc

    def claim(base, b, name, password):
        laptop.wait_until_succeeds(
            f"curl -sf --cacert /etc/losos-boxes.pem {base}/api/setup/claim | jq -e .ready", timeout=600
        )
        code, doc = call(f"{base}/api/setup/claim", method="POST", data={"name": name, "password": password})
        assert code == 200, (code, doc)
        b.wait_for_unit("authelia-losos.service")
        b.wait_for_open_port(9091)
        host = base.removeprefix("https://")
        b.wait_until_succeeds(f"grep -q \"domain: '{host}'\" /run/losos-identity/session.yml")

    def sign_in(base, name, password):
        body = json.dumps({"username": name, "password": password})
        head = curl(
            f"-D - -o /dev/null {base}/auth/api/firstfactor "
            f"-H 'Content-Type: application/json' -d '{body}'"
        )
        m = re.search(r"(?im)^set-cookie: (losos_session=[^;]+)", head)
        assert m, f"no session cookie for {name}: {head}"
        return m.group(1)

    def chat_token(base, cookie):
        code, doc = call(f"{base}/api/chat/session", cookie, method="POST", data={})
        assert code == 200, (code, doc)
        return doc["accessToken"]

    def notices(base, cookie, token, room):
        """The bodies of the notices in a room, oldest first."""
        q = urllib.parse.quote(room, safe="")
        code, doc = call(f"{base}/_matrix/client/v3/rooms/{q}/messages?dir=b&limit=100", cookie, token=token)
        assert code == 200, (code, doc)
        return [
            e["content"]["body"] for e in reversed(doc["chunk"])
            if e.get("type") == "m.room.message" and e["content"].get("msgtype") == "m.notice"
        ]

    def shots(phase, folder, *flags):
        try:
            laptop.succeed(f"federation-shots /tmp/shots/{folder} {phase} {' '.join(flags)} >&2")
        finally:
            laptop.copy_from_machine(f"/tmp/shots/{folder}", "shots")

    with subtest("each box is claimed: Matus on mattbox, Eva on kitchenbox"):
        claim(MATT, mattbox, "matus", MATUS_PASSWORD)
        claim(KITCHEN, kitchenbox, "eva", EVA_PASSWORD)
        matus = sign_in(MATT, "matus", MATUS_PASSWORD)
        eva = sign_in(KITCHEN, "eva", EVA_PASSWORD)
        code, doc = call(f"{MATT}/api/account", matus, method="POST",
                         data={"displayName": "Matus", "email": "matus@mattbox.local"})
        assert code == 200, (code, doc)
        code, doc = call(f"{KITCHEN}/api/account", eva, method="POST",
                         data={"displayName": "Eva", "email": "eva@kitchenbox.local"})
        assert code == 200, (code, doc)

    with subtest("the two chat servers federate over the LAN, and only there"):
        # Each finds the other's server by its .well-known address and
        # takes its signing key from it, not from matrix.org.
        for b, other in ((mattbox, "kitchenbox.local"), (kitchenbox, "mattbox.local")):
            b.succeed(f"curl -sfk https://{other}/.well-known/matrix/server | jq -e '.\"m.server\" == \"{other}:443\"'")
            b.succeed(f"curl -sfk https://{other}/_matrix/key/v2/server | jq -e '.server_name == \"{other}\"'")
            # ...or, as a Matrix server does when the well-known's certificate is a
            # box's own, at <name>:8448, which carries federation and keys only.
            b.succeed(f"curl -sfk https://{other}:8448/_matrix/key/v2/server | jq -e '.server_name == \"{other}\"'")
            b.fail(f"curl -sfk https://{other}:8448/git/")
        # The box itself is no other server: loopback is refused by nginx.
        mattbox.fail("curl -sfk --resolve mattbox.local:443:127.0.0.1 https://mattbox.local/_matrix/key/v2/server")
        mattbox.fail("curl -sfk --resolve mattbox.local:8448:127.0.0.1 https://mattbox.local:8448/_matrix/key/v2/server")

    with subtest("Matus has a public repository with its main branch"):
        ok("PUT", "/admin/users/notshared", {"full_name": "Matus", "email": "matus@mattbox.local"})
        owner = ok("POST", "/admin/users/notshared/tokens",
                   {"name": "laptop", "scopes": ["write:repository", "write:issue", "read:user"], "replace": True})["token"]
        ok("POST", "/user/repos", {
            "name": "site",
            "description": "The family website",
            "visibility": "public",
        }, token=owner)
        laptop.succeed(
            "mkdir -p /tmp/main && cd /tmp/main && git init -q -b main",
            "printf '# Our website\\n\\nOpening hours, the menu and how to find us.\\n' > /tmp/main/README.md",
            "printf '<h1>Hours</h1>\\n<p>Mon to Fri, 8 to 18.</p>\\n' > /tmp/main/hours.html",
            "cd /tmp/main && git add -A && git -c user.name=Matus -c user.email=matus@mattbox.local commit -qm 'The first pages'",
            f"cd /tmp/main && git push -q https://x:{owner}@mattbox.local/git/{REPO}.git main",
        )

    if os.environ.get("LOSOS_GIT_OLD_UI", ""):
        with subtest("the pictures' before column: the pages as they were"):
            ui = "${ui}"
            old = os.environ["LOSOS_GIT_OLD_UI"]
            for b in (mattbox, kitchenbox):
                b.succeed(f"mount --bind {old} {ui}", "systemctl restart nginx")
                b.wait_for_open_port(443)
            shots("before", "before", "--old", "--test", "losos-git-federation")
            for b in (mattbox, kitchenbox):
                b.succeed(f"umount {ui}", "systemctl restart nginx")
                b.wait_for_open_port(443)

    with subtest("Matus gives the repository a room in LosOS Chat"):
        shots("room", "room", "--test", "losos-git-federation")
        repo = ok("GET", f"/repos/{REPO}")
        room = repo["chat_room"]
        assert room and room.startswith("!") and room.endswith(":mattbox.local"), repo
        kept = json.loads(mattbox.succeed("cat /var/lib/losos/git-rooms.json"))
        assert kept == {REPO: room}, kept
        # Asked again, the same room.
        code, doc = call(f"{MATT}/api/chat/git-room", matus, method="POST", data={"owner": "notshared", "repo": "site"})
        assert (code, doc) == (200, {"room": room}), (code, doc)
        # Nobody but an administrator of the repository.
        code, doc = call(f"{MATT}/api/chat/git-room", method="POST", data={"owner": "notshared", "repo": "site"})
        assert code == 401, (code, doc)
        matus_chat = chat_token(MATT, matus)
        q = urllib.parse.quote(room, safe="")
        code, doc = call(f"{MATT}/_matrix/client/v3/rooms/{q}/joined_members", matus, token=matus_chat)
        assert code == 200, (code, doc)
        assert set(doc["joined"]) == {"@matus:mattbox.local", "@losos-git:mattbox.local"}, doc
        code, doc = call(f"{MATT}/_matrix/client/v3/rooms/{q}/state/m.room.join_rules/", matus, token=matus_chat)
        assert doc.get("join_rule") == "public", doc

    with subtest("Eva signs in to mattbox's LosOS Git with her chat address"):
        code, vouch = call(f"{KITCHEN}/api/chat/openid", eva, method="POST")
        assert code == 200, (code, vouch)
        assert vouch["matrix_server_name"] == "kitchenbox.local" and vouch["user_id"] == EVA_ID, vouch
        # Without LosOS Login on kitchenbox, nothing.
        code, doc = call(f"{KITCHEN}/api/chat/openid", method="POST")
        assert code == 401, (code, doc)
        # A token kitchenbox never gave, and mattbox's own server, get nothing.
        code, doc = call(f"{MATT}/git/api/v1/matrix/session", method="POST",
                         data={"access_token": "made-up", "matrix_server_name": "kitchenbox.local"})
        assert code == 401, (code, doc)
        code, doc = call(f"{MATT}/git/api/v1/matrix/session", method="POST",
                         data={"access_token": vouch["access_token"], "matrix_server_name": "mattbox.local"})
        assert code == 422, (code, doc)
        code, doc = call(f"{MATT}/git/api/v1/matrix/session", method="POST",
                         data={"access_token": vouch["access_token"], "matrix_server_name": "localhost"})
        assert code == 502, (code, doc)
        code, session = call(f"{MATT}/git/api/v1/matrix/session", method="POST",
                             data={"access_token": vouch["access_token"], "matrix_server_name": "kitchenbox.local",
                                   "display_name": "Eva"})
        assert code == 201, (code, session)
        assert session["login"] == EVA and session["matrix_id"] == EVA_ID, session
        eva_git = session["token"]
        code, me = call(f"{MATT}/git/api/v1/user", auth=f"token {eva_git}")
        assert code == 200 and me["matrix_id"] == EVA_ID and me["full_name"] == "Eva", (code, me)

    with subtest("she proposes a change from a branch under her own name"):
        quoted = urllib.parse.quote(EVA, safe="")
        remote = f"https://{quoted}:{eva_git}@mattbox.local/git/{REPO}.git"
        laptop.succeed(
            f"git clone -q {remote} /tmp/eva",
            "cd /tmp/eva && sed -i 's/8 to 18/8 to 18, Sat 9 to 12/' hours.html",
            "cd /tmp/eva && git -c user.name=Eva -c user.email=eva@kitchenbox.local commit -qam 'Open on Saturday mornings'",
        )
        out = laptop.fail("cd /tmp/eva && git push -q origin HEAD:main 2>&1")
        assert "eva@kitchenbox.local/" in out, out
        laptop.fail("cd /tmp/eva && git push -q origin HEAD:hours 2>&1")
        laptop.succeed(f"cd /tmp/eva && git push -q origin HEAD:{EVA}/hours")
        code, pull = call(f"{MATT}/git/api/v1/repos/{REPO}/pulls", method="POST", auth=f"token {eva_git}", data={
            "title": "Open on Saturday mornings",
            "body": "We start on Saturdays from next week.",
            "head": f"{EVA}/hours",
            "base": "main",
        })
        assert code == 201 and pull["number"] == 1, (code, pull)
        code, doc = call(f"{MATT}/git/api/v1/repos/{REPO}/issues/1/comments", method="POST",
                         auth=f"token {eva_git}", data={"body": "The menu page can wait for another one."})
        assert code == 201, (code, doc)
        ok("POST", f"/repos/{REPO}/issues/1/comments", {"body": "Thanks Eva, looks right."}, token=owner)
        # She cannot merge.
        code, doc = call(f"{MATT}/git/api/v1/repos/{REPO}/pulls/1/merge", method="POST",
                         auth=f"token {eva_git}", data={"Do": "merge"})
        assert code == 403, (code, doc)

    with subtest("the room on mattbox says what happened"):
        def said():
            got = notices(MATT, matus, matus_chat, room)
            return (
                any("opened pull request #1: Open on Saturday mornings" in n for n in got)
                and any(n.startswith(f"{EVA_ID} commented on pull request #1") for n in got)
                and any(n.startswith("@matus:mattbox.local commented on pull request #1") for n in got)
            )
        for _ in range(60):
            if said():
                break
            time.sleep(2)
        got = notices(MATT, matus, matus_chat, room)
        assert said(), got
        assert any(f"pushed to {EVA}/hours" in n for n in got), got

    with subtest("Eva joins the room from her own box, over federation"):
        eva_chat = chat_token(KITCHEN, eva)
        q = urllib.parse.quote(room, safe="")
        code, doc = call(f"{KITCHEN}/_matrix/client/v3/join/{q}?server_name=mattbox.local", eva,
                         method="POST", data={}, token=eva_chat)
        assert code == 200 and doc.get("room_id") == room, (code, doc)
        for _ in range(60):
            if any("opened pull request #1" in n for n in notices(KITCHEN, eva, eva_chat, room)):
                break
            time.sleep(2)
        got = notices(KITCHEN, eva, eva_chat, room)
        assert any("opened pull request #1" in n for n in got), got

    with subtest("the pictures: Eva's way in, and the room on both boxes"):
        shots("eva", "eva", "--test", "losos-git-federation")
        shots("chat", "chat", "--test", "losos-git-federation")

    with subtest("Matus merges, and the merge reaches Eva's box"):
        ok("POST", f"/repos/{REPO}/pulls/1/merge", {"Do": "merge"}, token=owner)
        for _ in range(90):
            if any("merged pull request #1" in n for n in notices(KITCHEN, eva, eva_chat, room)):
                break
            time.sleep(2)
        got = notices(KITCHEN, eva, eva_chat, room)
        assert any(n.startswith("@matus:mattbox.local merged pull request #1") for n in got), got
        laptop.succeed("cd /tmp/eva && git pull -q origin main && grep -q 'Sat 9 to 12' hours.html")

    with subtest("a repository without its room gets no notices"):
        ok("PATCH", f"/repos/{REPO}", {"chat_room": ""}, token=owner)
        before = len(notices(MATT, matus, matus_chat, room))
        laptop.succeed(
            "cd /tmp/main && git pull -q origin main 2>/dev/null || true",
            f"cd /tmp/main && git pull -q https://x:{owner}@mattbox.local/git/{REPO}.git main",
            "cd /tmp/main && echo '<p>Closed on holidays.</p>' >> hours.html",
            "cd /tmp/main && git -c user.name=Matus -c user.email=matus@mattbox.local commit -qam 'Holidays'",
            f"cd /tmp/main && git push -q https://x:{owner}@mattbox.local/git/{REPO}.git main",
        )
        time.sleep(10)
        assert len(notices(MATT, matus, matus_chat, room)) == before
        # The room ID set by hand through the API, with no room lososd made
        # for it, gets nothing either.
        ok("PATCH", f"/repos/{REPO}", {"chat_room": "!elsewhere:mattbox.local"}, token=owner)
  '';
}
