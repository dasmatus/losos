# nixos-test-vms config for LosOS Calendar (backend/src/cloud/calendar.rs,
# backend/src/cloud/webcal.rs, backend/src/ical.rs, admin-ui/app/src/calendar/).
#
# The box runs LosOS Login, lososd, nginx, LosOS Cloud, Synapse and
# Stalwart as they ship. A laptop on the LAN carries the browser's traffic,
# as in nix/tests/contacts.nix, and serves two calendar feeds over plain
# HTTP the way a tax office or EduPage publishes one. What only a booted box
# can show:
#
#   * a webcal:// address is read through the sandboxed fetcher
#     (losos-webcal@, its own unit with no access to the box), first as
#     https and then as http, and lands as a read-only calendar that a phone
#     syncs over CalDAV and cannot write to;
#   * a subscription follows its feed: on "Read now", and on its own once its
#     time is up, with what was added, changed and taken out;
#   * an address on the box itself is refused by the fetcher;
#   * an .ics file comes in, and a second import of it updates rather than
#     doubles;
#   * a published calendar is a feed anyone with the address reads, and
#     stops being one when it is unpublished;
#   * an event with a guest, a reminder and a video call, made on the page,
#     goes out as an iCalendar invitation in LosOS Mail; the guest answers in
#     Mail, the answer comes back by mail, and the organizer's copy shows it;
#   * the pages: the week, month and list, the event, the editor, the
#     subscribe and publish dialogs, the invitation in Mail, a reminder, and
#     the phone layout.
#
# The browser is Chromium under Playwright in the test driver
# (nix/tests/calendar-browser.mjs). Screenshots land in result/shots/pr/,
# the launch film's pictures in result/shots/film/.
#
# With `beforeSrc` (admin-ui/app of the branch this one is based on) the
# same box serves the admin pages from before LosOS Calendar, and the run
# takes only the "before" pictures: the sidebar, /calendar and an
# invitation in Mail as they were.
{
  pkgs,
  beforeSrc ? null,
}:

let
  inherit (pkgs) lib;
  lososPkgs = import ../packages.nix { inherit pkgs; };
  screens = import ./screens { inherit pkgs; };
  before = beforeSrc != null;

  # The admin UI as it ships, without LosOS Diagrams' three WebAssembly
  # packages (stand-ins, as nix/tests/contacts.nix), so the bundle builds
  # without Bevy; only /diagrams, never opened here, would fail.
  ui = lososPkgs.losos-admin-ui.overrideAttrs (
    _:
    {
      pname = "losos-admin-ui-without-lab";
      preBuild = ''
        for f in core-pkg/losos_lab_core virt-pkg/losos_lab_virt \
                 render-pkg/webgpu/losos_lab_render render-pkg/webgl2/losos_lab_render; do
          mkdir -p "src/lab/$(dirname "$f")"
          echo 'export default function init() { throw new Error("not in this build"); } export class Lab {}' > "src/lab/$f.js"
          : > "src/lab/''${f}_bg.wasm"
        done
        cat > vite.calendar-test.config.ts <<'EOF'
        import base from "./vite.config";
        const root = import.meta.dirname;
        const config = base as Record<string, any>;
        config.build.rolldownOptions.input = {
          main: `''${root}/index.html`,
          chat: `''${root}/chat/index.html`,
          manage: `''${root}/manage/index.html`,
        };
        export default config;
        EOF
      '';
      buildPhase = ''
        runHook preBuild
        bun --bun vite build --config vite.calendar-test.config.ts
        runHook postBuild
      '';
    }
    // lib.optionalAttrs before { src = beforeSrc; }
  );

  playwright = "${lososPkgs.losos-admin-ui.nodeModules}/node_modules/playwright";
  browsers = pkgs.playwright-driver.browsers.override {
    withFirefox = false;
    withWebkit = false;
  };
  fonts = pkgs.makeFontsConf {
    fontDirectories = [
      pkgs.dejavu_fonts
      pkgs.noto-fonts-color-emoji
    ];
  };

  # Two public holidays, as a calendar app exports them.
  holidays = pkgs.writeText "holidays.ics" (
    lib.concatStringsSep "\r\n" [
      "BEGIN:VCALENDAR"
      "VERSION:2.0"
      "PRODID:-//Example//Holidays//EN"
      "BEGIN:VEVENT"
      "UID:sk-2026-11-01@holidays.example"
      "DTSTAMP:20260101T000000Z"
      "DTSTART;VALUE=DATE:20261101"
      "DTEND;VALUE=DATE:20261102"
      "SUMMARY:All Saints' Day"
      "END:VEVENT"
      "BEGIN:VEVENT"
      "UID:sk-2026-11-17@holidays.example"
      "DTSTAMP:20260101T000000Z"
      "DTSTART;VALUE=DATE:20261117"
      "DTEND;VALUE=DATE:20261118"
      "SUMMARY:Struggle for Freedom and Democracy Day"
      "END:VEVENT"
      "END:VCALENDAR"
      ""
    ]
  );

  # What a phone writes into a subscription: refused.
  phoneEvent = pkgs.writeText "phone.ics" (
    lib.concatStringsSep "\r\n" [
      "BEGIN:VCALENDAR"
      "VERSION:2.0"
      "PRODID:-//Apple Inc.//iPhone OS 26.0//EN"
      "BEGIN:VEVENT"
      "UID:phone-1@example"
      "DTSTAMP:20260101T000000Z"
      "DTSTART:20261101T100000Z"
      "DTEND:20261101T110000Z"
      "SUMMARY:Written by a phone"
      "END:VEVENT"
      "END:VCALENDAR"
      ""
    ]
  );
in

pkgs.testers.nixosTest {
  name = if before then "losos-calendar-before" else "losos-calendar";

  nodes = {
    box =
      { lib, nodes, ... }:
      {
        imports = [
          ../../modules/options.nix
          ../../modules/containers.nix
          ../../modules/tls.nix
          ../../modules/daemon.nix
          ../../modules/identity.nix
          ../../modules/chat.nix
          ../../modules/mail.nix
          ../../modules/cloud.nix
        ];
        losos = {
          hostName = "mattbox";
          backend.package = lososPkgs.losos-ctl;
          admin.ui = ui;
          backend.plugins = lososPkgs.losos-plugins;
          git.enable = false;
          # LosOS Cloud keeps the calendars.
          cloud.enable = true;
          # The shared user's side of the box: chat's gate, and mail's.
          sharingMyStorage = true;
          cluster.enable = true;
          shared.fscrypt.enable = false;
        };

        networking.hostName = "mattbox";
        # The feeds' server is on the LAN, by name.
        networking.extraHosts = "${nodes.laptop.networking.primaryIPAddress} feeds.example";
        users.users.shared = {
          isNormalUser = true;
          uid = 1001;
          group = "shared";
          homeMode = "700";
        };
        users.groups.shared.gid = 1001;
        systemd.tmpfiles.rules = [ "d /home/shared/data 0700 shared shared -" ];

        systemd.services.lososd.path = lib.mkForce [ pkgs.coreutils ];
        environment.systemPackages = [
          pkgs.jq
          pkgs.curl
          pkgs.sqlite
        ];
        virtualisation = {
          memorySize = 3072;
          cores = 2;
          vlans = [ 1 ];
        };
      };

    laptop =
      { nodes, ... }:
      {
        networking.extraHosts = "${nodes.box.networking.primaryIPAddress} mattbox.local";
        systemd.services.socks = {
          wantedBy = [ "multi-user.target" ];
          after = [ "network-online.target" ];
          wants = [ "network-online.target" ];
          serviceConfig = {
            ExecStart = "${pkgs.microsocks}/bin/microsocks -i 0.0.0.0 -p 1080";
            DynamicUser = true;
          };
        };
        # The feeds, over plain HTTP. Nothing answers on 443, so a
        # webcal:// address is refused there at once and read over http.
        services.nginx = {
          enable = true;
          virtualHosts."feeds.example" = {
            default = true;
            root = "/srv/feeds";
            extraConfig = ''
              types { text/calendar ics; }
            '';
          };
        };
        systemd.tmpfiles.rules = [ "d /srv/feeds 0755 root root -" ];
        networking.firewall.allowedTCPPorts = [
          80
          443
          1080
        ];
        virtualisation.forwardPorts = [
          {
            from = "host";
            host.port = 1080;
            guest.port = 1080;
          }
        ];
        environment.etc = {
          "calendar/holidays.ics".source = holidays;
          "calendar/phone.ics".source = phoneEvent;
        };
        environment.systemPackages = [
          pkgs.curl
          pkgs.jq
        ];
        virtualisation.vlans = [ 1 ];
      };
  };

  testScript = ''
    import base64
    import json
    import os
    import re
    import subprocess
    import time
    from datetime import date, datetime, timedelta
    from zoneinfo import ZoneInfo

    BEFORE = ${if before then "True" else "False"}
    HOST = "mattbox.local"
    DOMAIN = "mattbox.home.arpa"
    PASSWORD = "Correct-Horse-Battery-9"
    EVA_PASSWORD = "Eva-Long-Password-3"

    ${screens.handbookScript}
    PEOPLE = "/var/lib/losos-identity/users.yml"
    DB = "/var/lib/losos-cloud/cloud.db"
    OUT = os.environ.get("out", os.getcwd())
    SHOTS = f"{OUT}/shots"
    os.makedirs(SHOTS, exist_ok=True)
    ZONE = ZoneInfo("Europe/Bratislava")
    TODAY = datetime.now(ZONE).date()
    MONDAY = TODAY - timedelta(days=TODAY.weekday())

    # ── The feeds, made for this week ─────────────────────────────────────
    VTIMEZONE = [
        "BEGIN:VTIMEZONE", "TZID:Europe/Bratislava",
        "BEGIN:DAYLIGHT", "TZOFFSETFROM:+0100", "TZOFFSETTO:+0200", "TZNAME:CEST",
        "DTSTART:19700329T020000", "RRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=-1SU", "END:DAYLIGHT",
        "BEGIN:STANDARD", "TZOFFSETFROM:+0200", "TZOFFSETTO:+0100", "TZNAME:CET",
        "DTSTART:19701025T030000", "RRULE:FREQ=YEARLY;BYMONTH=10;BYDAY=-1SU", "END:STANDARD",
        "END:VTIMEZONE",
    ]

    def ics(prodid, name, events):
        lines = ["BEGIN:VCALENDAR", "VERSION:2.0", f"PRODID:{prodid}", f"X-WR-CALNAME:{name}",
                 "X-WR-TIMEZONE:Europe/Bratislava", *VTIMEZONE]
        for e in events:
            lines += ["BEGIN:VEVENT", *e, "DTSTAMP:20260101T000000Z", "END:VEVENT"]
        lines.append("END:VCALENDAR")
        return "\r\n".join(lines) + "\r\n"

    def d(day):
        return day.strftime("%Y%m%d")

    def lesson(uid, summary, weekday, start, end, room, extra=()):
        day = MONDAY + timedelta(days=weekday)
        until = d(MONDAY + timedelta(weeks=12))
        return [f"UID:{uid}@edupage.example", f"SUMMARY:{summary}", f"LOCATION:{room}",
                f"DTSTART;TZID=Europe/Bratislava:{d(day)}T{start}00",
                f"DTEND;TZID=Europe/Bratislava:{d(day)}T{end}00",
                f"RRULE:FREQ=WEEKLY;UNTIL={until}T000000Z", *extra]

    def timetable(version):
        lessons = [
            lesson("mo1", "Mathematics", 0, "0800", "0845", "Room 12"),
            lesson("mo2", "English", 0, "0855", "0940", "Room 7" if version < 3 else "Room 9"),
            lesson("tu1", "Programming", 1, "1000", "1130", "Lab 2"),
            lesson("we1", "Mathematics", 2, "0800", "0845", "Room 12"),
            lesson("fr1", "English", 4, "0855", "0940", "Room 7"),
        ]
        if version == 1:
            lessons.append(lesson("th1", "Computer networks", 3, "1200", "1330", "Lab 1"))
        if version >= 2:
            lessons.append(lesson("fr2", "Physics", 4, "1000", "1045", "Room 3"))
        return ics("-//EduPage//Timetable//EN", "Timetable", lessons)

    def all_day(uid, summary, day, extra=()):
        return [f"UID:{uid}@tax.example", f"SUMMARY:{summary}", f"DTSTART;VALUE=DATE:{d(day)}",
                f"DTEND;VALUE=DATE:{d(day + timedelta(days=1))}", *extra]

    vat_day = TODAY.replace(day=25)
    tax_feed = ics("-//Tax office//Deadlines//EN", "Tax deadlines", [
        all_day("advance", "Income tax advance", TODAY + timedelta(days=2)),
        all_day("vat", "VAT return", vat_day, ["RRULE:FREQ=MONTHLY;BYMONTHDAY=25;COUNT=12"]),
        all_day("motor", "Motor vehicle tax", date(TODAY.year + 1, 1, 31)),
        all_day("income", "Income tax return", date(TODAY.year + 1, 3, 31)),
    ])

    def feed(name, text):
        laptop.succeed(f"cat > /srv/feeds/{name}.tmp <<'EOF'\n{text}EOF\nmv /srv/feeds/{name}.tmp /srv/feeds/{name}")

    start_all()
    box.wait_for_unit("lososd.service")
    # lososd's unit is up before its API: under emulation loading the
    # plugins takes it a minute, and the claim would meet nginx's 502.
    box.wait_for_open_port(8082)
    box.wait_for_unit("nginx.service")
    box.wait_for_unit("losos-chat.service")
    box.wait_for_unit("losos-mail.service")
    box.wait_for_unit("losos-cloud.service")
    box.wait_for_unit("losos-webcal.socket")
    box.wait_for_open_port(443)
    laptop.wait_for_unit("socks.service")
    laptop.wait_for_unit("nginx.service")
    feed("tax.ics", tax_feed)
    feed("timetable.ics", timetable(1))
    box.succeed("curl -sf http://feeds.example/tax.ics | grep -q 'Income tax advance'")

    def curl(args):
        return laptop.succeed(f"curl -sk {args}")

    def call(path, cookie=None, method="GET", data=None, extra=""):
        args = f"-X {method} 'https://{HOST}{path}' {extra}"
        if cookie:
            args += f" -H 'Cookie: {cookie}'"
        if data is not None:
            body = json.dumps(data).replace("'", "'\\'''")
            args += f" -H 'Content-Type: application/json' -d '{body}'"
        out = curl(f"-w '\\n%{{http_code}}' {args}")
        body, code = out.rsplit("\n", 1)
        return int(code), body

    def ok(path, cookie, **kw):
        code, body = call(path, cookie, **kw)
        assert code == 200, (path, code, body)
        return json.loads(body)

    def sign_in(name, password):
        body = json.dumps({"username": name, "password": password})
        head = curl(
            f"-D - -o /dev/null https://{HOST}/auth/api/firstfactor "
            f"-H 'Content-Type: application/json' -d '{body}'"
        )
        m = re.search(r"(?im)^set-cookie: (losos_session=[^;]+)", head)
        assert m, f"no session cookie for {name}: {head}"
        return m.group(1)

    with subtest("the claim makes Matus, and an administrator adds Eva"):
        code, body = call("/api/setup/claim", method="POST",
                          data={"name": "matus", "password": PASSWORD})
        assert code == 200, (code, body)
        box.wait_for_unit("authelia-losos.service")
        box.wait_for_open_port(9091)
        box.wait_until_succeeds(f"grep -q \"domain: '{HOST}'\" /run/losos-identity/session.yml")
        matus = sign_in("matus", PASSWORD)
        ok("/api/account", matus, method="POST",
           data={"displayName": "Matus", "email": "matus@mattbox.local"})
        ok("/api/people", matus, method="POST",
           data={"name": "eva", "displayName": "Eva", "email": "eva@mattbox.local",
                 "password": EVA_PASSWORD, "groups": []})
        box.wait_until_succeeds(f"grep -q eva {PEOPLE}")
        time.sleep(5)
        eva = sign_in("eva", EVA_PASSWORD)
        assert ok("/api/mail", matus)["address"] == f"matus@{DOMAIN}"
        assert ok("/api/mail", eva)["address"] == f"eva@{DOMAIN}"

    browser_env = {
        **os.environ,
        "HOME": os.environ.get("TMPDIR", "/tmp"),
        "CALENDAR_PLAYWRIGHT": "${playwright}",
        "PLAYWRIGHT_BROWSERS_PATH": "${browsers}",
        "PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS": "true",
        "FONTCONFIG_FILE": "${fonts}",
        "CALENDAR_HOST": HOST,
        "CALENDAR_MATUS_PASSWORD": PASSWORD,
        "CALENDAR_EVA_PASSWORD": EVA_PASSWORD,
        "CALENDAR_FFMPEG": "${pkgs.ffmpeg-headless}/bin/ffmpeg",
    }

    def browser(phase):
        run = subprocess.run(
            ["${pkgs.bun}/bin/bun", "${./calendar-browser.mjs}", phase, SHOTS],
            env=browser_env, capture_output=True, text=True, timeout=3600,
        )
        print(run.stdout)
        print(run.stderr)
        if run.returncode != 0:
            print(box.execute("journalctl -b --no-pager -o cat -n 800 -u lososd -u losos-cloud -u nginx -u 'losos-webcal@*'")[1])
        assert run.returncode == 0, f"the calendar pages, {phase}: {run.stderr[-2000:]}"

    A = "/cloud/api/calendar"

    def window(cookie, start, days):
        return ok(f"{A}/events?from={start.isoformat()}&to={(start + timedelta(days=days)).isoformat()}", cookie)["events"]

    def summaries(events, calendar=None):
        return sorted(e["summary"] for e in events if calendar is None or e["calendar"] == calendar)

    if BEFORE:
        with subtest("an invitation in Mail, before the Calendar"):
            tomorrow = TODAY + timedelta(days=1)
            made = ok(f"{A}/events", matus, method="POST", data={
                "calendar": "personal", "summary": "Review the box", "location": "", "description": "",
                "start": f"{tomorrow.isoformat()}T14:00", "end": f"{tomorrow.isoformat()}T15:00",
                "zone": "Europe/Bratislava", "alarms": [15], "call": "video",
                "organizer": {"email": f"matus@{DOMAIN}", "name": "Matus"},
                "attendees": [{"email": f"eva@{DOMAIN}", "name": "Eva"}],
            })
            invite = curl(f"-f -H 'Cookie: {matus}' 'https://{HOST}{A}/events/{made['id']}/invite'")
            ok("/api/mail/send", matus, method="POST", data={
                "to": [f"eva@{DOMAIN}"], "cc": [], "subject": "Invitation: Review the box",
                "text": "Matus invites you to Review the box.",
                "attachments": [{"name": "invite.ics", "type": "text/calendar",
                                 "data": base64.b64encode(invite.encode()).decode()}],
            })
            browser("before")
    else:
        DAV = f"https://{HOST}/remote.php/dav/calendars"

        def dav(who, method, path, extra=""):
            out = curl(f"-u '{who}' -X {method} -w '\\n%{{http_code}}' {extra} '{DAV}/{path}'")
            body, code = out.rsplit("\n", 1)
            return int(code), body

        OWNER = f"matus:{PASSWORD}"

        with subtest("a person starts with one calendar of their own, and only theirs"):
            cals = ok(f"{A}/calendars", matus)["calendars"]
            assert [(c["uri"], c["readOnly"]) for c in cals] == [("personal", False)], cals
            code, _ = call(f"{A}/calendars")
            assert code == 401, code
            code, _ = call(f"{A}/calendars", extra="-H 'X-Losos-Cloud-User: notshared' -H 'Remote-User: matus'")
            assert code == 401, code
            assert "calendar" in ok("/api/me", matus)["shared"]

        with subtest("a webcal:// address is read through the fetcher, https first, then http"):
            sub = ok(f"{A}/subscriptions", matus, method="POST",
                     data={"url": "webcal://feeds.example/tax.ics", "refresh": 86400, "color": "#dc2626"})
            tax = sub["calendar"]
            assert sub["report"]["added"] == 4, sub
            assert tax["readOnly"] and tax["name"] == "Tax deadlines", tax
            assert tax["feed"]["url"] == "webcal://feeds.example/tax.ics" and tax["feed"]["error"] == "", tax
            # The read ran in its own unit, not in LosOS Cloud.
            accepted = int(box.succeed("systemctl show -P NAccepted losos-webcal.socket").strip())
            assert accepted >= 1, accepted
            events = window(matus, TODAY, 3)
            assert "Income tax advance" in summaries(events, tax["uri"]), events
            year = window(matus, MONDAY, 200)
            vat = [e for e in year if e["summary"] == "VAT return"]
            assert len(vat) >= 6 and all(e["readOnly"] and e["allDay"] for e in vat), vat
            # The same address twice is the same subscription.
            again = ok(f"{A}/subscriptions", matus, method="POST", data={"url": "webcal://feeds.example/tax.ics"})
            assert again["calendar"]["uri"] == tax["uri"], again
            assert len(ok(f"{A}/calendars", matus)["calendars"]) == 2

        with subtest("a subscription is read-only, on the page and to a phone"):
            some = [e for e in window(matus, TODAY, 3) if e["calendar"] == tax["uri"]][0]
            code, body = call(f"{A}/events/{some['id']}", matus, method="DELETE")
            assert code in (403, 409), (code, body)
            code, body = dav(OWNER, "PROPFIND", "notshared/", "-H 'Depth: 1'")
            assert code == 207 and tax["uri"] in body, (code, body[:600])
            code, body = dav(OWNER, "PUT", f"notshared/{tax['uri']}/phone.ics",
                             "-H 'Content-Type: text/calendar' --data-binary @/etc/calendar/phone.ics")
            assert code == 403, (code, body)
            # The phone's own calendar takes it.
            code, body = dav(OWNER, "PUT", "notshared/personal/phone.ics",
                             "-H 'Content-Type: text/calendar' --data-binary @/etc/calendar/phone.ics")
            assert code == 201, (code, body)
            assert "Written by a phone" in summaries(window(matus, date(2026, 11, 1), 1))
            code, _ = dav(OWNER, "DELETE", "notshared/personal/phone.ics")
            assert code == 204, code

        with subtest("the fetcher refuses the box itself, and a feed that is not a calendar"):
            for url in ("http://127.0.0.1:8082/api/health", "http://localhost/", "ftp://feeds.example/tax.ics"):
                code, body = call(f"{A}/subscriptions", matus, method="POST", data={"url": url})
                assert code in (400, 422, 502), (url, code, body)
                assert json.loads(body).get("error"), body
            feed("notes.txt", "just some text\n")
            code, body = call(f"{A}/subscriptions", matus, method="POST",
                              data={"url": "http://feeds.example/notes.txt"})
            assert code in (400, 422, 502), (code, body)
            assert len(ok(f"{A}/calendars", matus)["calendars"]) == 2

        with subtest("an .ics file comes in, and a second import updates it"):
            put = lambda q: ok(f"{A}/import{q}", matus, method="POST",
                               extra="-H 'Content-Type: text/calendar' --data-binary @/etc/calendar/holidays.ics")
            first = put("")
            assert first["report"]["added"] == 2, first
            holidays = first["calendar"]
            assert not holidays["readOnly"], holidays
            second = put(f"?calendar={holidays['uri']}")
            assert second["report"]["added"] == 0 and second["report"]["updated"] == 2, second
            nov = window(matus, date(2026, 11, 1), 30)
            assert summaries(nov, holidays["uri"]) == ["All Saints' Day", "Struggle for Freedom and Democracy Day"], nov
            ok(f"{A}/calendars/{holidays['uri']}", matus, method="DELETE")

        with subtest("a published calendar is a feed for anyone with the address"):
            made = ok(f"{A}/events", matus, method="POST", data={
                "calendar": "personal", "summary": "Board games", "location": "Kitchen", "description": "",
                "start": f"{(TODAY + timedelta(days=3)).isoformat()}T19:00",
                "end": f"{(TODAY + timedelta(days=3)).isoformat()}T21:00",
                "zone": "Europe/Bratislava", "attendees": [], "alarms": [], "call": "",
                "repeat": {"freq": "weekly", "interval": 1},
            })
            assert made["repeats"] and made["repeat"]["freq"] == "weekly", made
            pub = ok(f"{A}/calendars/personal/publish", matus, method="POST")
            address = pub["published"]
            assert re.fullmatch(r"/cloud/feed/[A-Za-z0-9_-]{20,}\.ics", address), pub
            text = curl(f"-f 'https://{HOST}{address}'")
            assert "BEGIN:VCALENDAR" in text and "SUMMARY:Board games" in text and "RRULE:FREQ=WEEKLY" in text, text
            head = curl(f"-o /dev/null -w '%{{content_type}}' 'https://{HOST}{address}'")
            assert head.startswith("text/calendar"), head
            # Read-only, even to someone who guesses the method.
            code = curl(f"-o /dev/null -w '%{{http_code}}' -X PUT -d x 'https://{HOST}{address}'")
            assert code in ("403", "405"), code
            ok(f"{A}/calendars/personal/publish", matus, method="DELETE")
            code = curl(f"-o /dev/null -w '%{{http_code}}' 'https://{HOST}{address}'")
            assert code == "404", code
            ok(f"{A}/events/{made['id']}", matus, method="DELETE")

        with subtest("Eva reads none of Matus's calendars"):
            assert [c["uri"] for c in ok(f"{A}/calendars", eva)["calendars"]] == ["personal"]
            assert window(eva, TODAY, 3) == []
            code, _ = dav(f"eva:{EVA_PASSWORD}", "PROPFIND", f"notshared/{tax['uri']}/", "-H 'Depth: 1'")
            assert code in (403, 404), code

        with subtest("the pages: a subscription, an invitation by Mail and its answer"):
            browser("on")

        with subtest("the invitation went by Mail, and Eva's answer is on Matus's copy"):
            mine = [e for e in window(matus, TODAY, 8) if e["summary"] == "Review the box"]
            assert len(mine) == 1, mine
            review = mine[0]
            assert review["call"] == "video" and review["alarms"] == [15], review
            assert review["organizer"]["email"] == f"matus@{DOMAIN}", review
            assert [(a["email"], a.get("status")) for a in review["attendees"]] == [(f"eva@{DOMAIN}", "ACCEPTED")], review
            hers = [e for e in window(eva, TODAY, 8) if e["summary"] == "Review the box"]
            assert len(hers) == 1 and hers[0]["uid"] == review["uid"], hers
            # A phone of Matus's reads the answer over CalDAV too.
            code, body = dav(OWNER, "REPORT", "notshared/personal/",
                             "-H 'Depth: 1' -H 'Content-Type: application/xml' --data-binary "
                             "'<c:calendar-query xmlns:d=\"DAV:\" xmlns:c=\"urn:ietf:params:xml:ns:caldav\">"
                             "<d:prop><c:calendar-data/></d:prop><c:filter><c:comp-filter name=\"VCALENDAR\"/>"
                             "</c:filter></c:calendar-query>'")
            assert code == 207 and "PARTSTAT=ACCEPTED" in body.replace("\r\n ", ""), (code, body[:1200])

        with subtest("a subscription follows its feed, on Read now and on its own"):
            subs = [c for c in ok(f"{A}/calendars", matus)["calendars"] if c["name"] == "Timetable (EduPage)"]
            assert len(subs) == 1 and subs[0]["readOnly"], subs
            tt = subs[0]
            assert tt["feed"]["refresh"] == 21600, tt
            week = window(matus, MONDAY, 7)
            assert "Computer networks" in summaries(week, tt["uri"]), week
            # The feed changes; Read now brings it.
            feed("timetable.ics", timetable(2))
            r = ok(f"{A}/calendars/{tt['uri']}/refresh", matus, method="POST")["report"]
            assert (r["added"], r["removed"]) == (1, 1), r
            week = summaries(window(matus, MONDAY, 7), tt["uri"])
            assert "Physics" in week and "Computer networks" not in week, week
            # Unchanged, it is not read again in full (the server says 304).
            r = ok(f"{A}/calendars/{tt['uri']}/refresh", matus, method="POST")["report"]
            assert r == {"added": 0, "updated": 0, "removed": 0, "skipped": 0, "leftOut": 0}, r
            # Once its time is up, LosOS Cloud reads it by itself.
            feed("timetable.ics", timetable(3))
            box.succeed(f"sqlite3 {DB} \"UPDATE calendar_feed SET fetched = 0 WHERE url LIKE '%timetable.ics'\"")
            # From the laptop: the box's own loopback is refused the admin side.
            retry(lambda _: any(e["summary"] == "English" and e["location"] == "Room 9"
                                for e in window(matus, MONDAY, 1)), timeout=timedelta(seconds=180))

        with subtest("the launch film's pictures"):
            browser("film")

        with subtest("the handbook's pictures"):
            review = {"selector": 'button:has-text("Review the box")'}
            handbook_pictures(box, f"https://{HOST}", "matus", PASSWORD, [
                {"name": "calendar-week", "path": "/calendar", "waitFor": review,
                 "steps": [{"waitFor": {"selector": 'button:has-text("Mathematics")'}}],
                 "settle": 1500},
                {"name": "calendar-event", "path": "/calendar", "waitFor": review,
                 "steps": [{"click": review},
                           {"waitFor": {"selector": '#calendar-event-title:has-text("Review the box")'}}],
                 "settle": 1500},
            ], proxy="socks5://127.0.0.1:1080")
  '';
}
