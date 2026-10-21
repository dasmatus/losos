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
#     admin pages' headers, and the Office page frames it under the admin
#     policy;
#   * the WOPI routes refuse a missing, foreign or other file's token, and
#     nginx routes neither /wopi/ nor coolwsd's discovery, admin, metrics
#     and convert-to endpoints;
#   * the whole thing in a browser: the laptop opens /office/, the document
#     loads, letters typed land in the file on the box, and a second person
#     shows up in the first one's editor. nix/tests/office-shots.py takes a
#     picture of each step (for the pull request and the launch film), and
#     the driver copies them out to office-shots/;
#   * how much memory coolwsd takes idle and with documents open.
#
# LosOS Login is off here. Who is asking is what nginx passes in
# X-Losos-Cloud-User/-Name; the test sets those from two cookies the browser
# sends, where Authelia's Remote-User would on a box.
{ pkgs }:

let
  inherit (pkgs) lib;
  lososPkgs = import ../packages.nix { inherit pkgs; };
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
in
pkgs.testers.nixosTest {
  name = "losos-office";

  nodes.mattbox =
    { ... }:
    {
      imports = [
        ../../modules/options.nix
        ../../modules/containers.nix
        ../../modules/tls.nix
        ../../modules/office.nix
      ];
      virtualisation.memorySize = 4096;
      virtualisation.cores = 2;
      virtualisation.diskSize = 4096;

      losos = {
        hostName = "mattbox";
        backend.package = lososPkgs.losos-ctl;
        admin.enable = true;
        admin.ui = lososPkgs.losos-admin-ui;
        identity.enable = false;
        office = {
          enable = true;
          inherit folder;
        };
      };

      # LosOS Login's part: name the person on the editor-session calls.
      services.nginx.virtualHosts."losos-front".locations."/office/api/".extraConfig = lib.mkForce ''
        proxy_set_header Host $host;
        proxy_set_header X-Losos-Cloud-User $cookie_losos_person;
        proxy_set_header X-Losos-Cloud-Name $cookie_losos_person_name;
      '';

      # A TCG run is slow enough for a tab to count as idle or away; a box
      # keeps coolwsd's defaults.
      services.collabora-online.settings.per_view = {
        out_of_focus_timeout_secs = 3600;
        idle_timeout_secs = 3600;
      };

      networking.firewall.allowedTCPPorts = [ 80 ];
      environment.systemPackages = [
        pkgs.curl
        pkgs.jq
        pkgs.unzip
      ];
    };

  nodes.laptop =
    { nodes, ... }:
    {
      virtualisation.memorySize = 2048;
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

      environment.systemPackages = [ pkgs.curl ];
    };

  testScript = ''
    import json
    import re
    import shlex

    folder = "${folder}"
    client = "${client}"
    shots = "SHOTS=/tmp/shots ${shots}"
    trip_plan = "${tripPlan}"
    box = "https://mattbox.local"

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
        mattbox.fail("ss -ltn | grep ':8093' | grep -v 127.0.0.1")
        exts = json.loads(
            mattbox.succeed(
                "curl -sf --unix-socket /run/losos-office/http.sock"
                " -H 'X-Losos-Cloud-User: alice' http://box/office/api/extensions"
            )
        )["extensions"]
        by_ext = {e["extension"]: e["edit"] for e in exts}
        for ext in ["docx", "xlsx", "pptx", "odt", "ods", "odp"]:
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
        open_session("alice", "Plan.docx", "/tmp/alice.json")
        mattbox.succeed(f"OFFICE_WS=ws://localhost {client} edit --session /tmp/alice.json --saveas Plan.pdf")
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

    with subtest("in a browser: open, type, save, and a second person joins"):
        # A folder that looks like someone's: the earlier subtests' files go.
        mattbox.succeed(
            f"rm -rf {folder}/* {folder}/.trash",
            f"mkdir -p {folder}/Work",
            "printf 'Meeting notes\\n\\nNew office from 1 November.\\n' > /tmp/notes.txt",
            "printf 'Item,Amount\\nCabin,240\\nTrain,86\\nFood,120\\nTotal,=SUM(B2:B4)\\n' > /tmp/budget.csv",
        )
        convert(trip_plan, "Trip plan.odt", "odt")
        convert("/tmp/budget.csv", "Trip budget.ods", "ods")
        convert("/tmp/notes.txt", "Work/Meeting notes.odt", "odt")
        mattbox.succeed(f"chown -R losos-office:losos-office {folder}")
        laptop.wait_for_unit("chromium.service")
        laptop.wait_for_open_port(9222)

        def browse(*args):
            return laptop.succeed(" ".join([shots, *(shlex.quote(a) for a in args)]))

        def has(name, text):
            mattbox.wait_until_succeeds(
                f"unzip -p '{folder}/{name}' content.xml | grep -q {shlex.quote(text)}", timeout=300
            )

        trip = f"{box}/office/#file={hexid('Trip plan.odt')}"
        trip_row = f"a[href='#file={hexid('Trip plan.odt')}']"
        budget_row = f"a[href='#file={hexid('Trip budget.ods')}']"
        browse("open", "matus", "--person", "matus", "--name", "Matus", "--url", f"{box}/office/")
        browse("listed", "matus")
        browse(
            "shot", "matus", "01-documents", "--title", "The documents on the box",
            "--action", "click:trip", "--target", f"trip={trip_row}", "--target", f"budget={budget_row}",
        )
        browse("click", "matus", trip_row)
        browse("loaded", "matus")
        browse(
            "shot", "matus", "02-trip-plan", "--title", "Trip plan open in the editor",
            "--action", "click:page", "--target", "back", "--target", "name", "--target", "document",
        )
        # Into the document, then to its end and a new line.
        browse("click", "matus", "main iframe")
        browse("key", "matus", "ctrl+End", "Enter")
        browse(
            "type", "matus", "Pack the tent.", "--shot", "03-typing", "--title", "Matus types a line",
            "--action", "click:save",
        )
        browse("saved", "matus")
        browse("shot", "matus", "04-saved", "--title", "Saved to the box", "--target", "save")
        has("Trip plan.odt", "Pack the tent")

        browse("open", "eva", "--person", "eva", "--name", "Eva", "--url", trip)
        browse("loaded", "eva")
        browse("click", "eva", "main iframe")
        # The end of the line under the heading.
        browse("key", "eva", "ctrl+Home", "ArrowDown", "End")
        browse("write", "eva", " We leave at 7.")
        browse("views", "matus", "2")
        browse("shot", "matus", "05-together", "--title", "Eva is in the document too", "--target", "page")
        browse("shot", "eva", "06-together-eva", "--title", "What Eva sees", "--target", "page")
        browse("saved", "eva")
        has("Trip plan.odt", "We leave at 7.")
        has("Trip plan.odt", "Pack the tent")

        browse("open", "budget", "--person", "matus", "--name", "Matus", "--url", f"{box}/office/#file={hexid('Trip budget.ods')}")
        browse("loaded", "budget")
        browse("shot", "budget", "07-budget", "--title", "A spreadsheet", "--target", "page")
        busy = memory("with three editors open")

        # The editor works on https only; the page moves itself there.
        plain = f"http://mattbox.local/office/#file={hexid('Trip plan.odt')}"
        browse("open", "plain", "--person", "matus", "--name", "Matus", "--url", plain)
        browse("loaded", "plain")
        where = browse("where", "plain").strip()
        assert where.startswith("https://mattbox.local/office/#file="), where
        laptop.copy_from_machine("/tmp/shots", "office-shots")

    print(f"coolwsd memory: idle {idle // 2**20} MiB, with three editors open {busy // 2**20} MiB")
  '';
}
