# nixos-test-vms config for Office documents (modules/office.nix,
# backend/src/office/).
#
# The real Collabora Online from nixpkgs behind the box's real front vhost
# (modules/containers.nix), reading and saving through `losos-office` over a
# folder, and a laptop on the same LAN with Chromium. What only a VM shows:
#
#   * the editor session the host signs opens in coolwsd: a WebSocket through
#     nginx loads the document over WOPI, typed text is saved back into the
#     .docx in the folder, and "save as PDF" lands a new file beside it;
#   * two people in one document see each other and one save carries both
#     edits (co-editing over the WOPI lock);
#   * the editor page carries coolwsd's own frame-ancestors and none of the
#     admin pages' headers, and Documents frames it under the admin policy;
#   * LosOS Login names the person on Documents' calls, and nobody else
#     gets a session;
#   * the WOPI routes refuse a missing, foreign or other file's token, and
#     nginx routes neither /wopi/ nor coolwsd's discovery, admin, metrics
#     and convert-to endpoints;
#   * the whole thing in a browser, in the admin app, on Word, Excel and
#     PowerPoint files: the laptop opens Documents from the sidebar, a .docx
#     loads, letters typed land in the file on the box, a second person
#     shows up in the first one's editor, the file stays a .docx with its
#     headings, an amount changed in a .xlsx moves the total its formula
#     keeps, a .pptx opens, and the plain http address goes through sign-in
#     to the same document on https. nix/tests/office-shots.py takes a picture of each
#     step (for the pull request and the launch film), and the driver
#     copies them out to office-shots/;
#   * how much memory coolwsd takes idle and with documents open.
#
# Matus claims the box and adds Eva through lososd's own routes, and each
# signs in at /auth/. LosOS Cloud is a stub, as in nix/tests/identity.nix:
# the claim only asks it to take the owner's password.
{ pkgs }:

let
  inherit (pkgs) lib;
  lososPkgs = import ../packages.nix { inherit pkgs; };
  screens = import ./screens { inherit pkgs; };
  folder = "/var/lib/losos-office/files";
  python = pkgs.python3.withPackages (p: [ p.websocket-client ]);
  client = pkgs.writeShellScript "office-client" ''
    exec ${python}/bin/python3 ${./office-client.py} "$@"
  '';
  shots = pkgs.writeShellScript "office-shots" ''
    exec ${python}/bin/python3 ${./office-shots.py} "$@"
  '';
  # The document the browser part edits, as flat OpenDocument with its
  # heading styles, so it opens as a page and not as a web page.
  tripPlan = pkgs.writeText "trip-plan.fodt" ''
    <?xml version="1.0" encoding="UTF-8"?>
    <office:document xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" office:version="1.3" office:mimetype="application/vnd.oasis.opendocument.text">
     <office:styles>
      <style:style style:name="Text_20_body" style:display-name="Body Text" style:family="paragraph" style:class="text">
       <style:paragraph-properties fo:margin-top="0cm" fo:margin-bottom="0.25cm" fo:line-height="115%"/>
       <style:text-properties fo:font-size="12pt"/>
      </style:style>
      <style:style style:name="Heading_20_1" style:display-name="Heading 1" style:family="paragraph" style:next-style-name="Text_20_body" style:default-outline-level="1" style:class="text">
       <style:paragraph-properties fo:margin-top="0cm" fo:margin-bottom="0.3cm"/>
       <style:text-properties fo:font-size="24pt" fo:font-weight="bold"/>
      </style:style>
      <style:style style:name="Heading_20_2" style:display-name="Heading 2" style:family="paragraph" style:next-style-name="Text_20_body" style:default-outline-level="2" style:class="text">
       <style:paragraph-properties fo:margin-top="0.5cm" fo:margin-bottom="0.2cm"/>
       <style:text-properties fo:font-size="16pt" fo:font-weight="bold"/>
      </style:style>
     </office:styles>
     <office:body>
      <office:text>
       <text:h text:style-name="Heading_20_1" text:outline-level="1">Trip plan</text:h>
       <text:p text:style-name="Text_20_body">Tatras, 14 to 17 August. Four of us, one car.</text:p>
       <text:h text:style-name="Heading_20_2" text:outline-level="2">Before we go</text:h>
       <text:p text:style-name="Text_20_body">Book the cabin in Ždiar.</text:p>
       <text:p text:style-name="Text_20_body">Check the weather on Lomnický štít.</text:p>
      </office:text>
     </office:body>
    </office:document>
  '';
  # The presentation, made into a .pptx by coolwsd like the other two.
  kickoff = pkgs.writeText "kickoff.fodp" ''
    <?xml version="1.0" encoding="UTF-8"?>
    <office:document xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" xmlns:presentation="urn:oasis:names:tc:opendocument:xmlns:presentation:1.0" office:version="1.3" office:mimetype="application/vnd.oasis.opendocument.presentation">
     <office:automatic-styles>
      <style:page-layout style:name="PM1">
       <style:page-layout-properties fo:page-width="28cm" fo:page-height="15.75cm" style:print-orientation="landscape"/>
      </style:page-layout>
      <style:style style:name="dp1" style:family="drawing-page">
       <style:drawing-page-properties draw:fill="solid" draw:fill-color="#f4f7f8"/>
      </style:style>
      <style:style style:name="gr1" style:family="graphic">
       <style:graphic-properties draw:stroke="none" draw:fill="none"/>
      </style:style>
      <style:style style:name="P1" style:family="paragraph">
       <style:text-properties fo:font-size="40pt" fo:font-weight="bold" fo:color="#0f3d44"/>
      </style:style>
      <style:style style:name="P2" style:family="paragraph">
       <style:paragraph-properties fo:margin-bottom="0.3cm"/>
       <style:text-properties fo:font-size="24pt" fo:color="#1c2830"/>
      </style:style>
     </office:automatic-styles>
     <office:master-styles>
      <style:master-page style:name="Default" style:page-layout-name="PM1" draw:style-name="dp1"/>
     </office:master-styles>
     <office:body>
      <office:presentation>
       <draw:page draw:name="Kick-off" draw:master-page-name="Default" draw:style-name="dp1">
        <draw:frame draw:style-name="gr1" svg:x="2cm" svg:y="1.8cm" svg:width="24cm" svg:height="2.6cm">
         <draw:text-box><text:p text:style-name="P1">Tatras trip</text:p></draw:text-box>
        </draw:frame>
        <draw:frame draw:style-name="gr1" svg:x="2cm" svg:y="5.2cm" svg:width="24cm" svg:height="8cm">
         <draw:text-box>
          <text:p text:style-name="P2">14 to 17 August, from Ždiar</text:p>
          <text:p text:style-name="P2">Four of us, one car</text:p>
          <text:p text:style-name="P2">Budget and plan in Documents</text:p>
         </draw:text-box>
        </draw:frame>
       </draw:page>
      </office:presentation>
     </office:body>
    </office:document>
  '';
in
pkgs.testers.nixosTest {
  name = "losos-office";
  # The handbook's pictures in three languages take most of an hour on
  # their own under TCG.
  globalTimeout = 3 * 3600;

  nodes.mattbox =
    { ... }:
    {
      imports = [
        ../../modules/options.nix
        ../../modules/containers.nix
        ../../modules/tls.nix
        ../../modules/daemon.nix
        ../../modules/identity.nix
        ../../modules/office.nix
      ];
      virtualisation.memorySize = 4096;
      virtualisation.cores = 2;
      virtualisation.diskSize = 4096;

      losos = {
        hostName = "mattbox";
        backend.package = lososPkgs.losos-ctl;
        # The pins and the look are plugins; without them the page sees 404s.
        backend.plugins = lososPkgs.losos-plugins;
        admin.enable = true;
        admin.ui = lososPkgs.losos-admin-ui;
        office = {
          enable = true;
          inherit folder;
        };
      };

      systemd.services.lososd.path = lib.mkForce [ pkgs.coreutils ];

      # A TCG run is slow enough for a tab to count as idle or away; a box
      # keeps coolwsd's defaults.
      services.collabora-online.settings.per_view = {
        out_of_focus_timeout_secs = 3600;
        idle_timeout_secs = 3600;
      };

      networking.firewall.allowedTCPPorts = [ 80 ];
      environment.systemPackages = [
        pkgs.curl
        pkgs.file
        pkgs.jq
        pkgs.unzip
      ];
    };

  nodes.laptop =
    { nodes, ... }:
    {
      # The handbook's pictures start a second Chromium beside this one.
      virtualisation.memorySize = 3072;
      virtualisation.cores = 2;
      networking.hosts.${nodes.mattbox.networking.primaryIPAddress} = [ "mattbox.local" ];
      fonts.enableDefaultPackages = true;
      time.timeZone = "Europe/Bratislava";

      # One browser for the whole run, driven over the DevTools protocol by
      # office-shots.py: 1280 x 900 at 2x, dark, reduced motion, en-US. Tabs
      # left in the background keep running, so two people can be open at
      # once.
      systemd.services.chromium = {
        wantedBy = [ "multi-user.target" ];
        environment.TZ = "Europe/Bratislava";
        serviceConfig.ExecStart = lib.escapeShellArgs [
          "${pkgs.chromium}/bin/chromium"
          "--headless=new"
          "--no-sandbox"
          "--disable-gpu"
          "--remote-debugging-port=9222"
          "--user-data-dir=/tmp/chromium"
          "--window-size=1280,900"
          "--force-device-scale-factor=2"
          "--force-dark-mode"
          "--force-prefers-reduced-motion"
          "--lang=en-US"
          "--hide-scrollbars"
          # The box's own certificate, which a person trusts once in the
          # first-run wizard.
          "--ignore-certificate-errors"
          "--no-first-run"
          "--mute-audio"
          "--disable-background-timer-throttling"
          "--disable-renderer-backgrounding"
          "--disable-backgrounding-occluded-windows"
          "about:blank"
        ];
      };

      environment.systemPackages = [
        pkgs.curl
        screens.capture
      ];
    };

  testScript = ''
    import json
    import re
    import shlex
    import urllib.parse

    folder = "${folder}"
    client = "${client}"
    shots = "SHOTS=/tmp/shots ${shots}"
    trip_plan = "${tripPlan}"
    kickoff = "${kickoff}"
    box = "https://mattbox.local"
    PASSWORD = "Correct-Horse-Battery-9"
    EVA_PASSWORD = "Eva-Long-Password-3"

    ${screens.handbookScript}

    def hexid(name):
        return name.encode().hex()

    def open_session(user, name, out):
        body = json.dumps({"fileId": hexid(name)})
        return mattbox.succeed(
            "curl -sf --unix-socket /run/losos-office/http.sock"
            f" -H 'X-Losos-Cloud-User: {user}' -H 'X-Losos-Cloud-Name: {user.title()}'"
            " -H 'Origin: https://box.local' -H 'Content-Type: application/json'"
            f" -d '{body}' http://box/office/api/open | tee {out}"
        )

    def memory(label):
        current = int(mattbox.succeed("systemctl show coolwsd -p MemoryCurrent --value").strip())
        print(f"coolwsd memory {label}: {current // (1024 * 1024)} MiB")
        return current

    def sign_in(name, password):
        """Sign in at /auth/ the way the portal does; the session cookie's value."""
        body = json.dumps({"username": name, "password": password})
        head = laptop.succeed(
            f"curl -ks -D - -o /dev/null {box}/auth/api/firstfactor"
            f" -H 'Content-Type: application/json' -d '{body}'"
        )
        m = re.search(r"(?im)^set-cookie: losos_session=([^;]+)", head)
        assert m, f"no session cookie for {name}: {head}"
        return m.group(1)

    def api(path, cookie=None, data=None, extra=""):
        args = f"-ks -w '\\n%{{http_code}}' {box}{path} {extra}"
        if cookie:
            args += f" -H 'Cookie: losos_session={cookie}'"
        if data is not None:
            # A browser sends its origin with every POST; the hosts check it.
            args += f" -X POST -H 'Origin: {box}' -H 'Content-Type: application/json' -d '{json.dumps(data)}'"
        body, code = laptop.succeed(f"curl {args}").rsplit("\n", 1)
        return int(code), body

    def convert(src, name, fmt):
        mattbox.succeed(
            f"curl -sf -F 'data=@{src}' http://127.0.0.1:9980/cool/convert-to/{fmt} -o '{folder}/{name}'",
            f"chown losos-office:losos-office '{folder}/{name}'",
        )

    start_all()
    mattbox.wait_for_unit("nginx.service")
    mattbox.wait_for_unit("losos-office.service")
    mattbox.wait_for_unit("coolwsd.service")
    mattbox.wait_for_open_port(9980)
    mattbox.wait_for_open_port(8093)

    with subtest("coolwsd lists the Office types, and only on loopback"):
        mattbox.wait_until_succeeds(
            "curl -sf http://127.0.0.1:9980/hosting/discovery | grep -q 'ext=\"docx\"'",
            timeout=900,
        )
        mattbox.fail("ss -ltn | grep ':9980' | grep -v 127.0.0.1")
        # Macros in a document never run.
        mattbox.succeed(
            "grep -q '<enable_macros_execution[^>]*>false<'"
            " $(systemctl cat coolwsd | grep -o -- '--config-file=[^ \"]*' | cut -d= -f2)"
        )
        mattbox.fail("ss -ltn | grep ':8093' | grep -v 127.0.0.1")
        exts = json.loads(
            mattbox.succeed(
                "curl -sf --unix-socket /run/losos-office/http.sock"
                " -H 'X-Losos-Cloud-User: alice' http://box/office/api/extensions"
            )
        )["extensions"]
        by_ext = {e["extension"]: e["edit"] for e in exts}
        # Microsoft's formats, old, new and with macros, and OpenDocument.
        for ext in ["docx", "docm", "doc", "xlsx", "xlsm", "xlsb", "xls", "pptx", "pptm", "ppsx", "ppt", "odt", "ods", "odp"]:
            assert by_ext.get(ext) is True, (ext, by_ext)

    with subtest("documents to work on, made by coolwsd itself over loopback"):
        mattbox.succeed("printf 'Quarterly plan\\n' > /tmp/plan.txt")
        convert("/tmp/plan.txt", "Plan.docx", "docx")
        mattbox.succeed(f"unzip -p {folder}/Plan.docx word/document.xml | grep -q 'Quarterly plan'")
        idle = memory("idle, one document process warm")

    with subtest("nobody named, no session; a missing file, a 404"):
        mattbox.fail(
            "curl -sf --unix-socket /run/losos-office/http.sock -H 'Origin: https://box.local'"
            " -H 'Content-Type: application/json' -d '{\"fileId\":\"00\"}' http://box/office/api/open"
        )
        code = mattbox.succeed(
            "curl -s -o /dev/null -w '%{http_code}' --unix-socket /run/losos-office/http.sock"
            " -H 'X-Losos-Cloud-User: alice' -H 'Origin: https://box.local'"
            f" -H 'Content-Type: application/json' -d '{{\"fileId\":\"{hexid('Gone.docx')}\"}}'"
            " http://box/office/api/open"
        )
        assert code == "404", code

    session = json.loads(open_session("alice", "Plan.docx", "/tmp/alice.json"))
    assert session["writable"] is True, session
    assert session["url"].startswith("/browser/"), session
    assert session["https"] is True, session

    with subtest("the editor carries coolwsd's frame-ancestors, the Office page frames it"):
        laptop.wait_for_unit("multi-user.target")
        headers = laptop.succeed(f"curl -ksf -D - -o /dev/null '{box}{session['url']}'").lower()
        csp = [l for l in headers.splitlines() if l.startswith("content-security-policy:")]
        assert len(csp) == 1 and re.search(r"frame-ancestors\s+'self'", csp[0]), headers
        assert "wss://mattbox.local" in csp[0], csp[0]
        assert "x-frame-options" not in headers, headers
        page = laptop.succeed(f"curl -ksf -D - -o /dev/null {box}/office/").lower()
        assert "frame-src 'self'" in page and "x-frame-options: deny" in page, page

    with subtest("the editor page links branding.css, and the box answers it with the LosOS look"):
        editor = laptop.succeed(f"curl -ksf '{box}{session['url']}'")
        linked = re.search(r'href="([^"]*/branding\.css)"', editor)
        assert linked, "the editor page links no branding.css"
        css = laptop.succeed(f"curl -ksf -D - '{box}{linked.group(1)}'")
        assert re.search(r"(?im)^content-type: text/css", css), css[:400]
        # The palette, then the rules that hide Collabora's own bars.
        assert "--accent-rgb:" in css and "#toolbar-row" in css, css[-600:]

    with subtest("the WOPI routes take only a valid token for the file asked"):
        src = "http://127.0.0.1:8093/wopi/files/" + hexid("Plan.docx")
        token = session["accessToken"]
        info = json.loads(mattbox.succeed(f"curl -sf '{src}?access_token={token}'"))
        assert info["BaseFileName"] == "Plan.docx" and info["UserFriendlyName"] == "Alice", info
        for bad in ["", "access_token=nope", f"access_token={token[:-2]}00"]:
            code = mattbox.succeed(f"curl -s -o /dev/null -w '%{{http_code}}' '{src}?{bad}'")
            assert code == "401", (bad, code)
        other = "http://127.0.0.1:8093/wopi/files/" + hexid("Other.docx")
        code = mattbox.succeed(f"curl -s -o /dev/null -w '%{{http_code}}' '{other}?access_token={token}'")
        assert code == "401", code

    with subtest("nginx routes no WOPI and none of coolwsd's other endpoints"):
        for path in ["/cool/convert-to/pdf", "/cool/getMetrics", "/cool/adminws", "/browser/dist/admin/admin.html"]:
            code = laptop.succeed(f"curl -ks -o /dev/null -w '%{{http_code}}' {box}{path}")
            assert code == "404", (path, code)
        # These two fall through to the admin page, never to coolwsd or the host.
        for path, upstream in [
            ("/wopi/files/" + hexid("Plan.docx") + f"?access_token={token}", "BaseFileName"),
            ("/hosting/discovery", "wopi-discovery"),
        ]:
            body = laptop.succeed(f"curl -ks '{box}{path}'")
            assert upstream not in body, (path, body[:300])

    with subtest("open, type, save: the text is in the .docx in the folder"):
        out = json.loads(
            mattbox.succeed(f"OFFICE_WS=ws://localhost {client} edit --session /tmp/alice.json --text 'Written in LosOS' --save")
        )
        assert '"success":true' in out["save"].replace(" ", ""), out
        mattbox.succeed(f"unzip -p {folder}/Plan.docx word/document.xml | grep -q 'Written in LosOS'")
        mattbox.succeed(f"unzip -p {folder}/Plan.docx word/document.xml | grep -q 'Quarterly plan'")

    with subtest("save as PDF lands a new file beside the document"):
        # coolwsd can still hold the last session's lock on the file for a
        # moment; a session opened then gets lockfailed and is read-only.
        def save_as_pdf(_):
            open_session("alice", "Plan.docx", "/tmp/alice.json")
            rc, _ = mattbox.execute(f"OFFICE_WS=ws://localhost {client} edit --session /tmp/alice.json --saveas Plan.pdf")
            return rc == 0
        retry(save_as_pdf, timeout_seconds=600)
        mattbox.succeed(f"head -c 5 {folder}/Plan.pdf | grep -q '%PDF-'")

    with subtest("two people edit one document together and one save keeps both"):
        open_session("alice", "Plan.docx", "/tmp/alice.json")
        open_session("bob", "Plan.docx", "/tmp/bob.json")
        out = json.loads(
            mattbox.succeed(
                f"OFFICE_WS=ws://localhost {client} coedit --session /tmp/alice.json --session /tmp/bob.json"
                " --text 'Alice was here' --text 'Bob was here'"
            )
        )
        assert out["people"] == ["Alice", "Bob"], out
        doc = mattbox.succeed(f"unzip -p {folder}/Plan.docx word/document.xml")
        assert "Alice was here" in doc and "Bob was here" in doc, doc[-2000:]

    with subtest("a read-only file opens read-only"):
        mattbox.succeed(
            f"cp {folder}/Plan.docx {folder}/Locked.docx",
            f"chown losos-office:losos-office {folder}/Locked.docx",
            f"chmod 0444 {folder}/Locked.docx",
        )
        ro = json.loads(open_session("bob", "Locked.docx", "/tmp/ro.json"))
        assert ro["writable"] is False, ro

    with subtest("the folder's documents are listed, pictures and dot files not"):
        mattbox.succeed(
            f"mkdir -p {folder}/Work {folder}/.trash",
            f"cp {folder}/Plan.docx {folder}/Work/Notes.docx",
            f"cp {folder}/Plan.docx {folder}/.trash/Old.docx",
            f"printf 'x' > {folder}/photo.png",
            f"chown -R losos-office:losos-office {folder}",
        )
        files = json.loads(
            mattbox.succeed(
                "curl -sf --unix-socket /run/losos-office/http.sock"
                " -H 'X-Losos-Cloud-User: alice' http://box/office/api/files"
            )
        )["files"]
        seen = [(f["folder"], f["name"], f["writable"]) for f in files]
        assert seen == [
            ("", "Locked.docx", False),
            ("", "Plan.docx", True),
            ("", "Plan.pdf", False),
            ("Work", "Notes.docx", True),
        ], seen
        mattbox.fail(
            "curl -sf --unix-socket /run/losos-office/http.sock http://box/office/api/files"
        )

    with subtest("LosOS Login names the person on Documents' calls"):
        code, body = api("/api/setup/claim", data={"name": "matus", "password": PASSWORD})
        assert code == 200, (code, body)
        mattbox.wait_for_unit("authelia-losos.service")
        mattbox.wait_for_open_port(9091)
        mattbox.wait_until_succeeds("grep -q \"domain: 'mattbox.local'\" /run/losos-identity/session.yml")
        mattbox.wait_until_succeeds("curl -sf http://127.0.0.1:9091/auth/api/health")
        # LosOS Login reads the people file again on its own after each
        # change; signing Eva in before it has would count as failed attempts
        # against her.
        def changed(path, cookie, data):
            mark = mattbox.succeed(
                "journalctl -u authelia-losos -n 0 --show-cursor | sed -n 's/^-- cursor: //p'"
            ).strip()
            code, body = api(path, cookie, data)
            assert code == 200, (path, code, body)
            mattbox.wait_until_succeeds(
                f"journalctl -u authelia-losos --after-cursor='{mark}' | grep -q 'Reloaded successfully'",
                timeout=60,
            )

        matus = sign_in("matus", PASSWORD)
        changed("/api/account", matus, {"displayName": "Matus", "email": "matus@mattbox.local"})
        changed(
            "/api/people",
            matus,
            {"name": "eva", "displayName": "Eva", "email": "eva@mattbox.local", "password": EVA_PASSWORD, "groups": []},
        )
        # Signed out, Documents' calls go nowhere, and a name the browser
        # made up is dropped.
        code, _ = api("/office/api/files")
        assert code == 401, code
        code, _ = api("/office/api/files", extra="-H 'X-Losos-Cloud-User: matus' -H 'Remote-User: matus'")
        assert code == 401, code

        # Signed in, each session's token carries that person's own name.
        def named(person, password):
            cookie = sign_in(person, password)
            code, body = api("/office/api/open", cookie, {"fileId": hexid("Plan.docx")})
            assert code == 200, (person, code, body)
            token = json.loads(body)["accessToken"]
            src = "http://127.0.0.1:8093/wopi/files/" + hexid("Plan.docx")
            info = json.loads(mattbox.succeed(f"curl -sf '{src}?access_token={token}'"))
            return cookie, info["UserFriendlyName"]

        matus, shown = named("matus", PASSWORD)
        assert shown == "Matus", shown
        eva, shown = named("eva", EVA_PASSWORD)
        assert shown == "Eva", shown
        code, body = api("/office/api/files", eva)
        assert code == 200 and "Plan.docx" in body, (code, body)

    with subtest("in a browser: Word, Excel and PowerPoint files, and a second person joins"):
        # A folder that looks like someone's: the earlier subtests' files go.
        # Microsoft's formats, as people bring them, and one OpenDocument file.
        mattbox.succeed(
            f"rm -rf {folder}/* {folder}/.trash",
            f"mkdir -p {folder}/Work",
            "printf 'Meeting notes\\n\\nNew office from 1 November.\\n' > /tmp/notes.txt",
            "printf 'Item,Amount\\nCabin,240\\nTrain,86\\nFood,120\\nTotal,=SUM(B2:B4)\\n' > /tmp/budget.csv",
        )
        convert(trip_plan, "Trip plan.docx", "docx")
        convert("/tmp/budget.csv", "Trip budget.xlsx", "xlsx")
        convert(kickoff, "Kick-off.pptx", "pptx")
        convert("/tmp/notes.txt", "Work/Meeting notes.odt", "odt")
        mattbox.succeed(f"chown -R losos-office:losos-office {folder}")
        laptop.wait_for_unit("chromium.service")
        laptop.wait_for_open_port(9222)

        def browse(*args):
            # A session cookie may start with "-", which argparse would take
            # for an option of its own; give it as --cookie=VALUE.
            args = list(args)
            if "--cookie" in args:
                at = args.index("--cookie")
                args[at : at + 2] = [f"--cookie={args[at + 1]}"]
            return laptop.succeed(" ".join([shots, *(shlex.quote(a) for a in args)]))

        def has(name, part, text):
            mattbox.wait_until_succeeds(
                f"unzip -p '{folder}/{name}' {part} | grep -q {shlex.quote(text)}", timeout=300
            )

        def link(name):
            return f"{box}/office/#file={hexid(name)}"

        word = "word/document.xml"
        sheet = "xl/worksheets/sheet1.xml"
        trip_row = f"a[href='#file={hexid('Trip plan.docx')}']"
        budget_row = f"a[href='#file={hexid('Trip budget.xlsx')}']"
        documents = "nav a[href='/office/']"
        browse("open", "matus", "--name", "Matus", "--cookie", matus, "--url", f"{box}/apps")
        browse("click", "matus", documents)
        browse("listed", "matus")
        browse(
            "shot", "matus", "01-documents", "--title", "The documents on the box, beside the rest of LosOS",
            "--action", "click:trip", "--target", f"trip={trip_row}", "--target", f"budget={budget_row}",
            "--target", f"sidebar={documents}",
        )
        browse("click", "matus", trip_row)
        browse("loaded", "matus")
        browse(
            "shot", "matus", "02-trip-plan", "--title", "A Word document open in the editor",
            "--action", "click:page", "--target", "back", "--target", "name", "--target", "document",
        )
        # Into the document, then to its end and a new line.
        browse("click", "matus", "main iframe")
        browse("key", "matus", "ctrl+End", "Enter")
        browse(
            "type", "matus", "Pack the tent.", "--shot", "03-typing", "--title", "Matus types a line",
            "--action", "click:save",
        )
        # The line made bold with the LosOS formatting bar, not Collabora's.
        browse("key", "matus", "shift+Home")
        browse("click", "matus", '[data-command=".uno:Bold"]')
        browse(
            "shot", "matus", "04-bold", "--title", "Bold, from the LosOS bar",
            "--action", "click:save", "--target", "toolbar", "--target", "save",
        )
        browse("saved", "matus")
        browse("shot", "matus", "05-saved", "--title", "Saved to the box", "--target", "save")
        has("Trip plan.docx", word, "Pack the tent")
        # Bold in Word's own markup: a run with <w:b/> that holds the line.
        mattbox.wait_until_succeeds(
            f"unzip -p '{folder}/Trip plan.docx' {word}"
            " | grep -qP '<w:r>(?:(?!</w:r>).)*<w:b/>(?:(?!</w:r>).)*Pack the tent'",
            timeout=300,
        )

        browse("open", "eva", "--name", "Eva", "--cookie", eva, "--url", link("Trip plan.docx"))
        browse("loaded", "eva")
        browse("click", "eva", "main iframe")
        # The end of the line under the heading.
        browse("key", "eva", "ctrl+Home", "ArrowDown", "End")
        browse("write", "eva", " We leave at 7.")
        browse("views", "matus", "2")
        browse("shot", "matus", "06-together", "--title", "Eva is in the document too", "--target", "page")
        browse("shot", "eva", "07-together-eva", "--title", "What Eva sees", "--target", "page")
        browse("saved", "eva")
        has("Trip plan.docx", word, "We leave at 7.")
        has("Trip plan.docx", word, "Pack the tent")
        # Still Word's own file, headings and all, after two people saved it.
        mattbox.succeed(f"file -b '{folder}/Trip plan.docx' | grep -q 'Microsoft Word 2007+'")
        has("Trip plan.docx", word, 'w:pStyle w:val="Heading1"')
        has("Trip plan.docx", word, 'w:pStyle w:val="Heading2"')

        # A spreadsheet: change one amount, and the total follows.
        browse("open", "budget", "--name", "Matus", "--cookie", matus, "--url", link("Trip budget.xlsx"))
        browse("loaded", "budget")
        browse("shot", "budget", "08-budget", "--title", "An Excel workbook", "--target", "page")
        busy = memory("with three editors open")
        browse("click", "budget", "main iframe")
        browse("key", "budget", "ctrl+Home", "ArrowDown", "ArrowDown", "ArrowDown", "ArrowRight")
        browse("write", "budget", "150")
        browse("key", "budget", "Enter")
        browse("saved", "budget")
        browse("shot", "budget", "09-budget-total", "--title", "Food is 150 now, and the total follows", "--target", "page")
        mattbox.succeed(f"file -b '{folder}/Trip budget.xlsx' | grep -q 'Microsoft Excel 2007+'")
        has("Trip budget.xlsx", sheet, "<v>150</v>")
        has("Trip budget.xlsx", sheet, "SUM(B2:B4)</f><v>476</v>")
        browse("close", "eva")
        browse("close", "budget")

        # A presentation.
        browse("open", "slides", "--name", "Matus", "--cookie", matus, "--url", link("Kick-off.pptx"))
        browse("loaded", "slides")
        browse("shot", "slides", "10-kickoff", "--title", "A PowerPoint presentation", "--target", "page")
        mattbox.succeed(f"file -b '{folder}/Kick-off.pptx' | grep -q 'Microsoft PowerPoint 2007+'")
        browse("close", "slides")
        laptop.copy_from_machine("/tmp/shots", "office-shots")

    with subtest("the handbook's pictures"):
        handbook_pictures(laptop, box, "matus", PASSWORD, [
            {"name": "office", "path": "/office/",
             "waitFor": {"selector": 'main [aria-busy="false"] table'}, "settle": 1500},
            {"name": "office-document", "path": f"/office/#file={hexid('Trip plan.docx')}",
             # Collabora can take minutes to load a document under emulation.
             "waitFor": {"selector": "main iframe"},
             "steps": [{"waitFor": {"selector": 'main[aria-busy="false"] iframe'}, "timeout": 600000},
                       {"sleep": 15000}],
             "settle": 3000, "live": True},
        ])

    with subtest("the plain http address goes through sign-in to the same document on https"):
        # The session cookie is https only, so the plain address cannot tell
        # who is there and asks to sign in. Sign in moves to the same
        # document on https first: Matus, who has a session, is simply in,
        # and only someone without one goes on to LosOS Login.
        plain = f"http://mattbox.local/office/#file={hexid('Trip plan.docx')}"
        browse("open", "plain", "--name", "Matus", "--cookie", matus, "--url", plain)
        browse("signin", "plain")
        try:
            # Until the page has dropped the ?signin mark it came with.
            where = browse("where", "plain", link("Trip plan.docx")).strip()
            browse("open", "stranger", "--name", "Stranger", "--url", plain)
            browse("signin", "stranger")
            portal = browse("where", "stranger", "https://mattbox.local/auth/").strip()
        finally:
            laptop.copy_from_machine("/tmp/shots", "office-shots-plain")
        assert where == link("Trip plan.docx"), where
        browse("loaded", "plain")
        back = urllib.parse.parse_qs(urllib.parse.urlsplit(portal).query)["rd"][0]
        assert back == link("Trip plan.docx"), portal

    print(f"coolwsd memory: idle {idle // 2**20} MiB, with three editors open {busy // 2**20} MiB")
  '';
}
