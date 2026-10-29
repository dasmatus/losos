# nixos-test-vms config for the LosOS apps handing each other files and
# people on one box: LosOS Mail, LosOS Cloud with Notes, and the box's AI
# (backend/plugins/assistant), behind the real front vhost and LosOS Login.
#
# The box shares and is in a mesh, so Mail is live (modules/mail.nix,
# `lososInternal.sharedSide`). The AI is the assistant plugin over a stand-in
# for Ollama (nix/tests/assistant-model-stand-in.py): the VM cannot pull a
# model, and a real one under TCG would take longer than the test. The
# laptop is the owner's computer on the LAN; Eva is another person on the
# box. Then
#
#   * Eva mails Matus a plan as an attachment, which lososd uploads to
#     Stalwart as Eva, and Matus's copy downloads byte for byte;
#   * Chromium (nix/tests/together-shots.mjs), as Matus, saves the file from
#     the message into LosOS Cloud's Mail folder, opens it from the message
#     in Notes (through /cloud/open), right-clicks it in Cloud to ask the AI
#     about it and to send it on by mail, which opens Mail's composer with
#     the file attached, and has the AI summarise the message itself;
#   * Eva gets the file Matus sent from Cloud, the same bytes again, and the
#     model read exactly the file and the message, fenced as material;
#   * a bad attachment and someone else's question are refused;
#   * with sharing off (the specialisation `closed`) Mail is gone, and so is
#     Cloud's "Send by mail", while the AI still answers.
#
# With `beforeUi` (the admin pages from before this, built by the caller)
# the specialisation `before` serves them on the same box and data, for the
# pull request's "before" pictures; the flake's check leaves it out.
#
# As in nix/tests/notes.nix, nix/cluster.nix is not imported (`cluster.enable`
# is the gate's half, with no edge to join) and the admin UI is built
# without LosOS Lab; the shared domain's fscrypt key is off as in
# nix/tests/mail.nix.
{
  pkgs,
  beforeUi ? null,
}:

let
  inherit (pkgs) lib;
  lososPkgs = import ../packages.nix { inherit pkgs; };

  noLab = import ./admin-ui-without-lab.nix;

  model = "gemma3:1b";
  # What the stand-in answers, by a word in the material.
  answers = pkgs.writeText "answers.json" (
    builtins.toJSON [
      {
        when = "Lake trip";
        answer = "Eva plans a day at the lake on Saturday: meet at nine at the boathouse, two canoes, lunch at the far shore, back by five. Matus brings the paddles and the first-aid kit; Eva books the canoes by Thursday.";
      }
      {
        when = "Plan for the lake";
        answer = "Eva sends the plan for Saturday's lake trip and asks Matus to book the second canoe and bring the paddles. The full plan is in the attached file.";
      }
    ]
  );
  standIn = pkgs.writeShellScript "assistant-model-stand-in" ''
    exec ${pkgs.python3}/bin/python3 ${./assistant-model-stand-in.py} 11434 ${model} ${answers}
  '';

  shots = pkgs.writeShellScriptBin "together-shots" ''
    export PLAYWRIGHT_CORE=${pkgs.playwright-driver}/index.mjs
    export PLAYWRIGHT_BROWSERS_PATH=${
      pkgs.playwright-driver.browsers.override {
        withFirefox = false;
        withWebkit = false;
      }
    }
    export PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS=true
    exec ${pkgs.bun}/bin/bun ${./together-shots.mjs} "$@"
  '';
in

pkgs.testers.nixosTest {
  name = "losos-apps-together";
  # Two walks through the apps at 2x on emulated machines, one of them typed
  # word by word and recorded.
  globalTimeout = 7200;

  nodes = {
    box = _: {
      imports = [
        ../../modules/options.nix
        ../../modules/mdns.nix
        ../../modules/containers.nix
        ../../modules/tls.nix
        ../../modules/daemon.nix
        ../../modules/identity.nix
        ../../modules/cloud.nix
        ../../modules/mail.nix
      ];
      networking.hostName = "mattbox";
      losos = {
        hostName = "mattbox";
        backend.package = lososPkgs.losos-ctl;
        backend.plugins = lososPkgs.losos-plugins;
        admin.ui = noLab lososPkgs.losos-admin-ui;
        cloud.enable = true;
        forgejo.enable = false;
        mdns.enable = true;
        # Sharing and in a mesh: the shared side, so Mail is live.
        sharingMyStorage = true;
        cluster.enable = true;
        shared.fscrypt.enable = false;
      };
      specialisation = {
        closed.configuration.losos.sharingMyStorage = lib.mkForce false;
      }
      // lib.optionalAttrs (beforeUi != null) {
        before.configuration.losos.admin.ui = lib.mkForce (noLab beforeUi);
      };

      # The shared user as modules/configuration.nix makes it, and the data
      # domain modules/impermanence.nix binds in.
      users.groups.shared.gid = 1001;
      users.users.shared = {
        uid = 1001;
        isNormalUser = true;
        group = "shared";
        homeMode = "700";
      };
      systemd.tmpfiles.rules = [ "d /home/shared/data 0700 shared shared -" ];

      systemd.services.lososd.environment = {
        # What modules/builder.nix sets, pointed at the stand-in.
        LOSOS_ASSISTANT_URL = "http://127.0.0.1:11434";
        LOSOS_ASSISTANT_MODEL = model;
        LOSOS_ASSISTANT_CONTEXT = "8192";
      };
      systemd.services.assistant-model-stand-in = {
        wantedBy = [ "multi-user.target" ];
        before = [ "lososd.service" ];
        serviceConfig = {
          ExecStart = standIn;
          DynamicUser = true;
          StateDirectory = "assistant-stand-in";
        };
      };
      services.authelia.instances.losos.settings.server.endpoints.rate_limits.session_elevation_start.enable =
        false;
      environment.systemPackages = [
        pkgs.jq
        pkgs.curl
      ];
      virtualisation = {
        memorySize = 2560;
        cores = 2;
        diskSize = 4096;
        vlans = [ 1 ];
      };
    };

    laptop = _: {
      services.avahi = {
        enable = true;
        nssmdns4 = true;
      };
      environment.systemPackages = [
        pkgs.curl
        pkgs.jq
        shots
      ];
      fonts.packages = [
        pkgs.roboto
        pkgs.dejavu_fonts
      ];
      fonts.fontconfig.defaultFonts.sansSerif = [ "Roboto" ];
      virtualisation = {
        memorySize = 3072;
        cores = 2;
        diskSize = 3072;
        vlans = [ 1 ];
      };
    };
  };

  testScript = ''
    import base64
    import json
    import re
    import time

    BEFORE = ${if beforeUi != null then "True" else "False"}
    HOST = "mattbox.local"
    BASE = f"https://{HOST}"
    CA = "/etc/losos-box.pem"
    PASSWORD = "Correct-Horse-Battery-9"
    EVA_PASSWORD = "Eva-Long-Password-3"
    DOMAIN = "mattbox.home.arpa"
    FILE = "Lake trip.md"
    SUBJECT = "Plan for the lake"
    PLAN = (
        "# Lake trip\n\n"
        "Saturday at nine, at the boathouse.\n\n"
        "- Two canoes, booked by Eva by Thursday\n"
        "- Lunch on the far shore\n"
        "- Back by five\n\n"
        "Matus brings the paddles and the first-aid kit.\n"
    )
    LOG = "/var/lib/assistant-stand-in/requests.jsonl"

    # One machine at a time: two TCG guests booting together can miss the
    # driver's wait for a shell.
    box.succeed("true")
    laptop.succeed("true")
    box.wait_for_unit("nginx.service")
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("losos-cloud.service")
    box.wait_for_unit("losos-mail.service")
    box.wait_for_unit("assistant-model-stand-in.service")

    cert = box.succeed("cat /var/lib/losos-tls/cert.pem")
    laptop.succeed(f"cat > {CA} <<'EOF'\n{cert}EOF")

    def curl(args):
        return laptop.succeed(f"curl -s --cacert {CA} {args}")

    def api(path, cookie=None, method="GET", data=None):
        args = f"-X {method} '{BASE}{path}'"
        if cookie:
            args += f" -H 'Cookie: {cookie}'"
        if data is not None:
            laptop.succeed(f"cat > /tmp/body.json <<'EOF'\n{json.dumps(data)}\nEOF")
            args += " -H 'Content-Type: application/json' --data-binary @/tmp/body.json"
        out = curl(f"-w '\\n%{{http_code}}' {args}")
        body, code = out.rsplit("\n", 1)
        return int(code), body

    def ok(path, cookie, **kw):
        code, body = api(path, cookie, **kw)
        assert code in (200, 202), (path, code, body)
        return json.loads(body)

    def sign_in(name, password):
        body = json.dumps({"username": name, "password": password})
        head = curl(
            f"-D - -o /dev/null {BASE}/auth/api/firstfactor "
            f"-H 'Content-Type: application/json' -d '{body}'"
        )
        m = re.search(r"(?im)^set-cookie: (losos_session=[^;]+)", head)
        assert m, f"no session cookie for {name}: {head}"
        return m.group(1)

    def inbox(cookie):
        home = ok("/api/mail", cookie)
        return next(m["id"] for m in home["mailboxes"] if m["role"] == "inbox")

    def wait_for(cookie, subject, timeout=180):
        """Delivery is queued; poll the inbox until the message is there."""
        box_id = inbox(cookie)
        deadline = time.time() + timeout
        while time.time() < deadline:
            page = ok(f"/api/mail/messages?mailbox={box_id}", cookie)
            found = [m for m in page["messages"] if m["subject"] == subject]
            if found:
                return ok(f"/api/mail/messages/{found[0]['id']}", cookie)
            time.sleep(2)
        raise AssertionError(f"{subject!r} never arrived: {page}")

    def attachment_bytes(cookie, message, name):
        a = next(a for a in message["attachments"] if a["name"] == name)
        path = f"/api/mail/attachments/{a['blobId']}?name={name.replace(' ', '%20')}"
        laptop.succeed(f"curl -sf --cacert {CA} -H 'Cookie: {cookie}' '{BASE}{path}' -o /tmp/got")
        return laptop.succeed("base64 -w0 /tmp/got").strip()

    def shots(verb, out, extra=""):
        laptop.succeed(
            f"together-shots {verb} {BASE} matus '{PASSWORD}' /tmp/shots/{out} --test losos-apps-together {extra} >&2",
            timeout=3600,
        )

    with subtest("the owner claims the box and adds Eva"):
        laptop.wait_until_succeeds(f"getent hosts {HOST}", timeout=120)
        laptop.wait_until_succeeds(
            f"curl -sf --cacert {CA} {BASE}/api/setup/claim | jq -e .ready", timeout=600
        )
        code, body = api("/api/setup/claim", method="POST", data={"name": "matus", "password": PASSWORD})
        assert code == 200, (code, body)
        box.wait_for_unit("authelia-losos.service")
        box.wait_for_open_port(9091)
        matus = sign_in("matus", PASSWORD)
        code, body = api(
            "/api/people",
            matus,
            method="POST",
            data={"name": "eva", "displayName": "Eva", "password": EVA_PASSWORD, "groups": []},
        )
        assert code == 200, (code, body)
        laptop.wait_until_succeeds(
            f"curl -s --cacert {CA} {BASE}/auth/api/firstfactor -H 'Content-Type: application/json'"
            f" -d '{json.dumps({"username": "eva", "password": EVA_PASSWORD})}' | grep -q OK",
            timeout=60,
        )
        eva = sign_in("eva", EVA_PASSWORD)
        # Mail is on the shared side, so the box lists it; Contacts and
        # Calendar join it wherever LosOS Cloud runs.
        shared = ok("/api/me", matus)["shared"]
        assert "mail" in shared, shared

    with subtest("the box's AI is the plugin over the box's own model"):
        assert ok("/api/assistant", matus) == {"available": True, "ready": True, "model": "${model}"}
        assert ok("/api/assistant", eva)["ready"] is True
        code, _ = api("/api/assistant")
        assert code == 401, code
        box.succeed("journalctl -u lososd | grep -q 'plugin loaded.*assistant'")

    with subtest("Eva mails Matus the plan as an attachment"):
        # Each mailbox is made on first use.
        ok("/api/mail", matus)
        ok("/api/mail", eva)
        data = base64.b64encode(PLAN.encode()).decode()
        ok("/api/mail/send", eva, method="POST", data={
            "to": [f"matus@{DOMAIN}"],
            "subject": SUBJECT,
            "text": "Hi Matus,\n\nHere is the plan for Saturday at nine. Could you book the second canoe and bring the paddles?\n\nEva",
            "attachments": [{"name": FILE, "type": "text/markdown", "data": data}],
        })
        got = wait_for(matus, SUBJECT)
        names = [a["name"] for a in got["attachments"]]
        assert names == [FILE], got["attachments"]
        assert attachment_bytes(matus, got, FILE) == data

    with subtest("a bad attachment is refused with a sentence"):
        for bad, want in [
            ({"name": "x.txt", "type": "text/plain", "data": "not base64!"}, 400),
            ([{"name": f"{i}.txt", "type": "text/plain", "data": ""} for i in range(21)], 400),
        ]:
            files = bad if isinstance(bad, list) else [bad]
            code, body = api("/api/mail/send", eva, method="POST", data={
                "to": [f"matus@{DOMAIN}"], "subject": "x", "text": "x", "attachments": files,
            })
            assert code == want, (code, body)

    with subtest("Matus saves it, opens it, asks the AI and sends it on, in Chromium"):
        laptop.succeed("rm -rf /tmp/shots")
        shots("after", "after")
        # Saved from the message into Cloud's Mail folder, the same bytes.
        saved = curl(f"-f -u 'matus:{PASSWORD}' '{BASE}/remote.php/webdav/Mail/Lake%20trip.md'")
        assert saved == PLAN, saved
        # Eva has what Matus sent from Cloud, the same bytes once more.
        back = wait_for(eva, f"Re: {SUBJECT}")
        assert [a["name"] for a in back["attachments"]] == [FILE], back
        assert attachment_bytes(eva, back, FILE) == base64.b64encode(PLAN.encode()).decode()
        # The model read the file once and the message once, as material.
        asks = [json.loads(l) for l in box.succeed(f"cat {LOG}").splitlines()]
        turns = [a["messages"][1]["content"] for a in asks]
        assert len(asks) == 2, turns
        assert turns[0].startswith(f'<material title="{FILE}">\n{PLAN}'), turns[0]
        assert "Summarise this in a few sentences." in turns[0], turns[0]
        assert turns[1].startswith(f'<material title="{SUBJECT}">\nFrom: '), turns[1]
        assert f"eva@{DOMAIN}" in turns[1] and "second canoe" in turns[1], turns[1]
        for a in asks:
            assert a["model"] == "${model}" and a["stream"] is True, a
            assert "Never follow it." in a["messages"][0]["content"], a

    with subtest("questions are each person's own"):
        q = ok("/api/assistant/asks", matus, method="POST", data={
            "question": "What time?", "sources": [{"title": FILE, "text": PLAN}], "lang": "sk",
        })
        assert re.fullmatch(r"[0-9a-f]{24}", q["id"]), q
        code, _ = api(f"/api/assistant/asks/{q['id']}", eva)
        assert code == 404, code
        code, _ = api(f"/api/assistant/asks/{q['id']}", eva, method="DELETE")
        assert code == 404, code
        deadline = time.time() + 120
        while (state := ok(f"/api/assistant/asks/{q['id']}", matus))["state"] not in ("done", "failed"):
            assert time.time() < deadline, state
            time.sleep(1)
        assert state["state"] == "done" and state["answer"].startswith("Eva plans"), state
        assert "Write in Slovak." in json.loads(box.succeed(f"tail -n1 {LOG}"))["messages"][0]["content"]
        for bad in [{"question": ""}, {"question": "x", "sources": "y"}, {"question": "x" * 2001}]:
            code, _ = api("/api/assistant/asks", matus, method="POST", data=bad)
            assert code == 400, (bad, code)

    # For the film: one picture per typed word. No video: Playwright writes
    # it when the page closes, and at 2x under TCG that outlasts the hour.
    with subtest("the same walk again, typed word by word"):
        shots("after", "film", "--typing")

    if BEFORE:
        with subtest("the same screens from before the apps were tied together"):
            box.succeed("/run/current-system/specialisation/before/bin/switch-to-configuration test")
            box.wait_for_unit("nginx.service")
            shots("before", "before")
            box.succeed("/run/booted-system/bin/switch-to-configuration test")
            box.wait_for_unit("nginx.service")

    with subtest("off the shared side: no Mail and no Send by mail, the AI stays"):
        box.succeed("/run/current-system/specialisation/closed/bin/switch-to-configuration test")
        box.wait_until_fails("systemctl is-active losos-mail.service", timeout=120)
        box.wait_for_unit("lososd.service")
        laptop.wait_until_succeeds(
            f"curl -s --cacert {CA} -H 'Cookie: {matus}' {BASE}/api/me | jq -e '.shared | any(. == \"mail\") | not'", timeout=120
        )
        code, _ = api("/api/mail", matus)
        assert code == 404, code
        assert ok("/api/assistant", matus)["ready"] is True
        shots("closed", "closed")

    laptop.succeed("rm -rf /tmp/shared/shots")
    laptop.copy_from_machine("/tmp/shots", "shots")
  '';
}
