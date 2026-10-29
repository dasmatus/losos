# nixos-test-vms config for LosOS Contacts (backend/src/cloud/contacts.rs,
# backend/src/contacts.rs, backend/src/vcard.rs, admin-ui/app/src/contacts/).
#
# The box runs LosOS Login, lososd, nginx, LosOS Cloud, Synapse and
# Stalwart as they ship; a laptop on the LAN carries the browser's traffic,
# as in nix/tests/chat.nix. What only a booted box can show:
#
#   * the book is the person's LosOS Cloud address books: what the Contacts
#     page writes, a phone reads over CardDAV, and what a phone writes, the
#     page shows, every book of it; an edit on the page keeps the lines the
#     page does not read; nobody else reads the book, and the spare key has
#     none;
#   * .vcf files in vCard 3.0 (with a photo), 4.0 and 2.1 come in, a second
#     import of the same file updates rather than doubles, and an export
#     reads back into another person's book with the same contacts;
#   * the people pickers (GET /api/contacts/suggest) offer the person's
#     contacts and the people on this box;
#   * Contacts is there wherever LosOS Cloud is: with sharing off the book
#     and the page stay, and only the Mail, Chat, Call and Video buttons go;
#   * the pages: the book, a person's page, the editor with the picker, an
#     import, the right-click menu, Mail's To field and Chat's
#     "Message someone" offering contacts, and a contact's Chat button
#     opening a conversation with them.
#
# The browser is Chromium under Playwright in the test driver
# (nix/tests/contacts-browser.mjs). Screenshots land in result/shots/pr/.
#
# With `beforeSrc` (admin-ui/app of the branch this one is based on) the
# same box serves the admin pages from before Contacts, and the run takes
# only the "before" pictures: the sidebar, Mail's To field and Chat's
# picker as they were.
{
  pkgs,
  beforeSrc ? null,
}:

let
  inherit (pkgs) lib;
  lososPkgs = import ../packages.nix { inherit pkgs; };
  screens = import ./screens { inherit pkgs; };
  before = beforeSrc != null;

  # The admin UI as it ships, without the Lab page and LosOS Cloud's page,
  # which this test never opens. Stand-ins take the place of LosOS
  # Diagrams' three WebAssembly packages (as nix/tests/calls.nix), so the
  # bundle builds without Bevy; only /diagrams, never opened here, would fail.
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
        cat > vite.contacts-test.config.ts <<'EOF'
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
        bun --bun vite build --config vite.contacts-test.config.ts
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
  # makeFontsConf carries no conf.d, so nothing maps the page's generic
  # families (system-ui, sans-serif, monospace) to a face, and Chromium
  # drew every page in DejaVu Serif. The aliases name the faces outright.
  fonts = pkgs.writeText "contacts-fonts.conf" ''
    <?xml version="1.0"?>
    <!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">
    <fontconfig>
      <include>${
        pkgs.makeFontsConf {
          fontDirectories = [
            pkgs.dejavu_fonts
            pkgs.noto-fonts-color-emoji
          ];
        }
      }</include>
      <alias binding="same"><family>system-ui</family><prefer><family>DejaVu Sans</family></prefer></alias>
      <alias binding="same"><family>ui-sans-serif</family><prefer><family>DejaVu Sans</family></prefer></alias>
      <alias><family>sans-serif</family><prefer><family>DejaVu Sans</family><family>Noto Color Emoji</family></prefer></alias>
      <alias><family>serif</family><prefer><family>DejaVu Serif</family></prefer></alias>
      <alias binding="same"><family>ui-monospace</family><prefer><family>DejaVu Sans Mono</family></prefer></alias>
      <alias><family>monospace</family><prefer><family>DejaVu Sans Mono</family></prefer></alias>
    </fontconfig>
  '';

  # The files people bring from their phones and other address books.
  #
  # vCard 3.0, as iOS and Google write it: three people, one with a photo
  # (base64, folded at 75 octets), Apple's item1. groups with X-ABLabel,
  # a Matrix ID as IMPP and a mesh address.
  threeCards = pkgs.runCommand "contacts-3.0.vcf" { nativeBuildInputs = [ pkgs.coreutils ]; } ''
    photo=$(base64 -w0 ${../../admin-ui/themes/brand/salmon-128.png} | fold -w 74 | sed '2,$s/^/ /')
    {
      printf 'BEGIN:VCARD\r\nVERSION:3.0\r\n'
      printf 'N:Nováková;Eva;;;\r\nFN:Eva Nováková\r\n'
      printf 'EMAIL;TYPE=INTERNET;TYPE=HOME:eva@mattbox.home.arpa\r\n'
      printf 'IMPP;X-SERVICE-TYPE=Matrix:matrix:u/eva:mattbox.local\r\n'
      printf 'TEL;TYPE=CELL:+421 905 123 456\r\n'
      printf 'X-LOSOS-MESH:eva@mattbox.local\r\n'
      printf 'PHOTO;ENCODING=b;TYPE=PNG:'
      printf '%s' "$photo" | sed 's/$/\r/'
      printf '\nEND:VCARD\r\n'
      printf 'BEGIN:VCARD\r\nVERSION:3.0\r\n'
      printf 'N:Kováčová;Jana;;;\r\nFN:Jana Kováčová\r\n'
      printf 'ORG:Pekáreň u Jany\r\nTITLE:Majiteľka\r\n'
      printf 'item1.EMAIL;TYPE=INTERNET:jana@pekaren.example\r\nitem1.X-ABLabel:_$!<Work>!$_\r\n'
      printf 'TEL;TYPE=WORK;TYPE=VOICE:+421 2 555 0101\r\n'
      printf 'ADR;TYPE=WORK:;;Hlavná 12;Košice;;040 01;Slovensko\r\n'
      printf 'BDAY:1984-03-09\r\n'
      printf 'NOTE:Bread on Fridays\\, rye only.\r\n'
      printf 'URL:https://pekaren.example\r\n'
      printf 'X-LOSOS-MESH:jana@kitchenbox.local\r\n'
      printf 'END:VCARD\r\n'
      printf 'BEGIN:VCARD\r\nVERSION:3.0\r\n'
      printf 'N:Bielik;Tomáš;;;\r\nFN:Tomáš Bielik\r\n'
      printf 'EMAIL:tomas@bielik.example\r\n'
      printf 'URL:https://matrix.to/#/@tomas:kitchenbox.local\r\n'
      printf 'END:VCARD\r\n'
    } > $out
  '';

  # vCard 4.0, as Thunderbird and Nextcloud write it, with a UID.
  fourCards = pkgs.writeText "contacts-4.0.vcf" (
    lib.concatStringsSep "\r\n" [
      "BEGIN:VCARD"
      "VERSION:4.0"
      "UID:urn:uuid:4fbe8971-0bc3-424c-9c26-36c3e1eff6b1"
      "FN:Zuzana Horváthová"
      "N:Horváthová;Zuzana;;;"
      "EMAIL;TYPE=work;PREF=1:zuzana@horvath.example"
      "TEL;VALUE=uri;TYPE=\"voice,cell\":tel:+421-911-222-333"
      "IMPP;PREF=1:matrix:u/zuzana:kitchenbox.local"
      "END:VCARD"
      ""
    ]
  );

  # vCard 2.1, as an old phone exports it: bare parameters and
  # quoted-printable.
  oldCard = pkgs.writeText "contacts-2.1.vcf" (
    lib.concatStringsSep "\r\n" [
      "BEGIN:VCARD"
      "VERSION:2.1"
      "N;CHARSET=UTF-8;ENCODING=QUOTED-PRINTABLE:Rak;Ondrej;;;"
      "FN;CHARSET=UTF-8;ENCODING=QUOTED-PRINTABLE:Ondrej Rak"
      "TEL;CELL;PREF:+421903777888"
      "EMAIL;INTERNET:ondrej@rak.example"
      "NOTE;CHARSET=UTF-8;ENCODING=QUOTED-PRINTABLE:Sused zo =C5=A1tvrt=C3=A9ho poschodia"
      "END:VCARD"
      ""
    ]
  );

  # A card a phone writes over CardDAV, with lines the Contacts page does
  # not read: they stay when the page edits the card.
  phoneCard = pkgs.writeText "marek.vcf" (
    lib.concatStringsSep "\r\n" [
      "BEGIN:VCARD"
      "VERSION:3.0"
      "PRODID:-//Apple Inc.//iPhone OS 26.0//EN"
      "UID:9c1d7e52-phone-marek"
      "N:Horný;Marek;;;"
      "FN:Marek Horný"
      "EMAIL;TYPE=INTERNET;TYPE=HOME:marek@horny.example"
      "CATEGORIES:Family"
      "X-PHONE-RINGTONE:marimba"
      "item2.X-ABRELATEDNAMES:Mama"
      "item2.X-ABLabel:_$!<Mother>!$_"
      "END:VCARD"
      ""
    ]
  );

  # A second address book, as a phone or Thunderbird makes one.
  workBook = pkgs.writeText "mkcol.xml" ''
    <?xml version="1.0" encoding="utf-8"?>
    <d:mkcol xmlns:d="DAV:" xmlns:card="urn:ietf:params:xml:ns:carddav">
      <d:set><d:prop>
        <d:resourcetype><d:collection/><card:addressbook/></d:resourcetype>
        <d:displayname>Work</d:displayname>
      </d:prop></d:set>
    </d:mkcol>
  '';
  workCard = pkgs.writeText "silvia.vcf" (
    lib.concatStringsSep "\r\n" [
      "BEGIN:VCARD"
      "VERSION:4.0"
      "UID:urn:uuid:5a0e3c1f-work-silvia"
      "FN:Silvia Malinová"
      "N:Malinová;Silvia;;;"
      "EMAIL;TYPE=work:silvia@firma.example"
      "ORG:Firma s.r.o."
      "END:VCARD"
      ""
    ]
  );
in

pkgs.testers.nixosTest {
  name = if before then "losos-contacts-before" else "losos-contacts";

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
          # LosOS Cloud keeps the book.
          cloud.enable = true;
          # The shared user's side of the box: chat's gate, and mail's.
          sharingMyStorage = true;
          cluster.enable = true;
          shared.fscrypt.enable = false;
        };
        specialisation.unshared.configuration.losos.sharingMyStorage = lib.mkForce false;

        networking.hostName = "mattbox";
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
        networking.firewall.allowedTCPPorts = [ 1080 ];
        virtualisation.forwardPorts = [
          {
            from = "host";
            host.port = 1080;
            guest.port = 1080;
          }
        ];
        environment.etc = {
          "contacts/three.vcf".source = threeCards;
          "contacts/four.vcf".source = fourCards;
          "contacts/old.vcf".source = oldCard;
          "contacts/marek.vcf".source = phoneCard;
          "contacts/mkcol.xml".source = workBook;
          "contacts/silvia.vcf".source = workCard;
        };
        environment.systemPackages = [
          pkgs.curl
          pkgs.jq
        ];
        virtualisation.vlans = [ 1 ];
      };
  };

  testScript = ''
    import json
    import os
    import re
    import subprocess
    import time

    BEFORE = ${if before then "True" else "False"}
    HOST = "mattbox.local"
    PASSWORD = "Correct-Horse-Battery-9"
    EVA_PASSWORD = "Eva-Long-Password-3"

    ${screens.handbookScript}
    PEOPLE = "/var/lib/losos-identity/users.yml"
    OUT = os.environ.get("out", os.getcwd())
    SHOTS = f"{OUT}/shots"
    os.makedirs(SHOTS, exist_ok=True)

    start_all()
    box.wait_for_unit("lososd.service")
    # lososd's unit is up before its API: under emulation loading the
    # plugins takes it a minute, and the claim would meet nginx's 502.
    box.wait_for_open_port(8082)
    box.wait_for_unit("nginx.service")
    box.wait_for_unit("losos-chat.service")
    box.wait_for_unit("losos-mail.service")
    box.wait_for_unit("losos-cloud.service")
    box.wait_for_open_port(443)
    box.wait_for_open_port(8008)
    laptop.wait_for_unit("socks.service")

    def curl(args):
        return laptop.succeed(f"curl -sk {args}")

    def call(path, cookie=None, method="GET", data=None, token=None, extra=""):
        args = f"-X {method} 'https://{HOST}{path}' {extra}"
        if cookie:
            args += f" -H 'Cookie: {cookie}'"
        if token:
            args += f" -H 'Authorization: Bearer {token}'"
        if data is not None:
            args += f" -H 'Content-Type: application/json' -d '{json.dumps(data)}'"
        out = curl(f"-w '\\n%{{http_code}}' {args}")
        body, code = out.rsplit("\n", 1)
        return int(code), body

    def ok(path, cookie, **kw):
        code, body = call(path, cookie, **kw)
        assert code == 200, (path, code, body)
        return json.loads(body)

    def upload(path, cookie, file):
        return ok(path, cookie, method="POST",
                  extra=f"-H 'Content-Type: text/vcard' --data-binary @{file}")

    def sign_in(name, password):
        body = json.dumps({"username": name, "password": password})
        head = curl(
            f"-D - -o /dev/null https://{HOST}/auth/api/firstfactor "
            f"-H 'Content-Type: application/json' -d '{body}'"
        )
        m = re.search(r"(?im)^set-cookie: (losos_session=[^;]+)", head)
        assert m, f"no session cookie for {name}: {head}"
        return m.group(1)

    def switch(to):
        path = "/run/booted-system" if to is None else f"/run/booted-system/specialisation/{to}"
        box.succeed(f"{path}/bin/switch-to-configuration test >&2")
        box.wait_for_unit("lososd.service")
        box.wait_for_open_port(8082)
        box.wait_for_unit("nginx.service")

    def names(book):
        return sorted(c["name"] for c in book["contacts"])

    with subtest("the claim makes Matus, and an administrator adds Eva"):
        code, body = call("/api/setup/claim", method="POST",
                          data={"name": "matus", "password": PASSWORD})
        assert code == 200, (code, body)
        spare = json.loads(body)["token"]
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

    browser_env = {
        **os.environ,
        "HOME": os.environ.get("TMPDIR", "/tmp"),
        "CONTACTS_PLAYWRIGHT": "${playwright}",
        "PLAYWRIGHT_BROWSERS_PATH": "${browsers}",
        "PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS": "true",
        "FONTCONFIG_FILE": "${fonts}",
        "CONTACTS_HOST": HOST,
        "CONTACTS_MATUS_PASSWORD": PASSWORD,
        "CONTACTS_OLD_CARD": "${oldCard}",
        "CONTACTS_FFMPEG": "${pkgs.ffmpeg-headless}/bin/ffmpeg",
    }

    def browser(phase):
        run = subprocess.run(
            ["${pkgs.bun}/bin/bun", "${./contacts-browser.mjs}", phase, SHOTS],
            env=browser_env, capture_output=True, text=True, timeout=2400,
        )
        print(run.stdout)
        print(run.stderr)
        if run.returncode != 0:
            print(box.execute("journalctl -b --no-pager -o cat -n 800 -u lososd -u losos-chat -u nginx")[1])
        assert run.returncode == 0, f"the contacts pages, {phase}: {run.stderr[-2000:]}"

    if BEFORE:
        with subtest("the admin pages before Contacts"):
            browser("before")
    else:
        C = "/cloud/api/contacts"
        DAV = f"https://{HOST}/remote.php/dav/addressbooks/users"

        def dav(who, method, path, extra=""):
            out = curl(f"-u '{who}' -X {method} -w '\\n%{{http_code}}' {extra} '{DAV}/{path}'")
            body, code = out.rsplit("\n", 1)
            return int(code), body

        OWNER = f"matus:{PASSWORD}"

        with subtest("a book is empty until something is put in it, and is only the person's"):
            assert ok(C, matus) == {"contacts": []}
            assert "contacts" in ok("/api/me", matus)["shared"]
            # The spare key is not a person, and has no book.
            code, body = call(C, extra=f"-H 'Authorization: Bearer {spare}'")
            assert code in (401, 403), (code, body)
            code, body = call(C)
            assert code == 401, (code, body)
            # A made-up person header is not believed.
            code, body = call(C, extra="-H 'X-Losos-Cloud-User: notshared' -H 'Remote-User: matus'")
            assert code == 401, (code, body)

        with subtest("a vCard 3.0 file with a photo comes in, and a second import updates it"):
            report = upload(f"{C}/import", matus, "/etc/contacts/three.vcf")
            assert report == {"added": 3, "updated": 0, "skipped": 0, "photosDropped": 0, "leftOut": 0}, report
            book = ok(C, matus)
            assert names(book) == ["Eva Nováková", "Jana Kováčová", "Tomáš Bielik"], book
            by = {c["name"]: c for c in book["contacts"]}
            eva_card, jana, tomas = by["Eva Nováková"], by["Jana Kováčová"], by["Tomáš Bielik"]
            assert eva_card["book"] == "Contacts", eva_card
            assert eva_card["matrix"] == "@eva:mattbox.local", eva_card
            assert eva_card["mesh"] == "eva@mattbox.local", eva_card
            assert jana["org"] == "Pekáreň u Jany" and jana["title"] == "Majiteľka", jana
            assert jana["emails"] == [{"value": "jana@pekaren.example", "label": "work"}], jana
            assert jana["addresses"][0]["locality"] == "Košice", jana
            assert jana["birthday"] == "1984-03-09" and jana["note"] == "Bread on Fridays, rye only.", jana
            assert tomas["matrix"] == "@tomas:kitchenbox.local", tomas
            # The photo, scaled to a JPEG, from its own route.
            assert eva_card.get("photo"), eva_card
            assert "photo" not in jana, jana
            head = curl(f"-o /tmp/photo.jpg -w '%{{http_code}} %{{content_type}}' "
                        f"-H 'Cookie: {matus}' 'https://{HOST}{eva_card['photo']}'")
            assert head == "200 image/jpeg", head
            assert laptop.succeed("head -c 3 /tmp/photo.jpg | od -An -tx1").split() == ["ff", "d8", "ff"]
            # The same file again: nothing doubles.
            again = upload(f"{C}/import", matus, "/etc/contacts/three.vcf")
            assert again["added"] == 0 and again["updated"] == 3, again
            assert len(ok(C, matus)["contacts"]) == 3

        with subtest("vCard 4.0 and 2.1 come in too"):
            assert upload(f"{C}/import", matus, "/etc/contacts/four.vcf")["added"] == 1
            assert upload(f"{C}/import", matus, "/etc/contacts/old.vcf")["added"] == 1
            by = {c["name"]: c for c in ok(C, matus)["contacts"]}
            zuzana, ondrej = by["Zuzana Horváthová"], by["Ondrej Rak"]
            assert zuzana["uid"] == "urn:uuid:4fbe8971-0bc3-424c-9c26-36c3e1eff6b1", zuzana
            assert zuzana["matrix"] == "@zuzana:kitchenbox.local", zuzana
            assert zuzana["phones"][0]["value"] == "+421-911-222-333", zuzana
            assert ondrej["note"] == "Sused zo štvrtého poschodia", ondrej
            assert ondrej["phones"][0]["value"] == "+421903777888", ondrej
            # The 2.1 card is in the browser's import later; take it out again.
            assert call(f"{C}/{ondrej['id']}", matus, method="DELETE")[0] == 200
            assert "Ondrej Rak" not in names(ok(C, matus))

        with subtest("one contact is added, edited and given a photo by hand"):
            made = ok(C, matus, method="POST",
                      data={"name": "Peter Dlhý", "emails": [{"value": "peter@dlhy.example", "label": "home"}]})
            pid = made["id"]
            assert re.fullmatch(r"[0-9]+", pid), made
            made["phones"] = [{"value": "+421 948 000 111", "label": "mobile"}]
            edited = ok(f"{C}/{pid}", matus, method="PUT", data=made)
            assert edited["phones"][0]["value"] == "+421 948 000 111", edited
            pic = ok(f"{C}/{pid}/photo", matus, method="PUT",
                     extra="-H 'Content-Type: image/png' --data-binary @${../../admin-ui/themes/brand/salmon.png}")
            assert pic.get("photo"), pic
            assert ok(f"{C}/{pid}", matus)["phones"] == edited["phones"]
            assert call(f"{C}/{pid}", matus, method="DELETE")[0] == 200
            code, _ = call(f"{C}/{pid}", matus)
            assert code == 404, code

        with subtest("the book is LosOS Cloud's address books, the ones a phone syncs"):
            # What the page put in, a phone reads over CardDAV.
            code, body = dav(OWNER, "PROPFIND", "notshared/contacts/", "-H 'Depth: 1'")
            assert code == 207, (code, body[:400])
            assert body.count(".vcf</d:href>") == 4, body
            # What a phone writes, the page shows.
            code, body = dav(OWNER, "PUT", "notshared/contacts/marek.vcf",
                             "-H 'If-None-Match: *' -H 'Content-Type: text/vcard' --data-binary @/etc/contacts/marek.vcf")
            assert code == 201, (code, body)
            by = {c["name"]: c for c in ok(C, matus)["contacts"]}
            marek = by["Marek Horný"]
            assert marek["emails"][0]["value"] == "marek@horny.example", marek
            # An edit on the page keeps the lines only the phone reads.
            marek["phones"] = [{"value": "+421 907 555 444", "label": "mobile"}]
            assert ok(f"{C}/{marek['id']}", matus, method="PUT", data=marek)["phones"][0]["value"] == "+421 907 555 444"
            code, card = dav(OWNER, "GET", "notshared/contacts/marek.vcf")
            assert code == 200, (code, card)
            for line in ("UID:9c1d7e52-phone-marek", "CATEGORIES:Family", "X-PHONE-RINGTONE:marimba",
                         "item2.X-ABRELATEDNAMES:Mama", "item2.X-ABLabel:_$!<Mother>!$_",
                         "+421 907 555 444", "FN:Marek Horný"):
                assert line in card, (line, card)
            # A second book, made by the phone, is part of the same list.
            code, body = dav(OWNER, "MKCOL", "notshared/work/",
                             "-H 'Content-Type: application/xml' --data-binary @/etc/contacts/mkcol.xml")
            assert code == 201, (code, body)
            code, body = dav(OWNER, "PUT", "notshared/work/silvia.vcf",
                             "-H 'Content-Type: text/vcard' --data-binary @/etc/contacts/silvia.vcf")
            assert code == 201, (code, body)
            by = {c["name"]: c for c in ok(C, matus)["contacts"]}
            assert by["Silvia Malinová"]["book"] == "Work", by["Silvia Malinová"]
            assert by["Silvia Malinová"]["org"] == "Firma s.r.o.", by["Silvia Malinová"]
            # A delete on the page is gone from the phone's next sync.
            assert call(f"{C}/{marek['id']}", matus, method="DELETE")[0] == 200
            code, _ = dav(OWNER, "GET", "notshared/contacts/marek.vcf", "-o /dev/null")
            assert code == 404, code
            # Eva reads none of it, over CardDAV or the API.
            code, _ = dav(f"eva:{EVA_PASSWORD}", "PROPFIND", "notshared/contacts/", "-H 'Depth: 1'")
            assert code in (403, 404), code
            assert ok(C, eva)["contacts"] == []
            code, _ = call(f"{C}/{by['Silvia Malinová']['id']}", eva)
            assert code == 404, code

        with subtest("an export reads back into another person's book"):
            count = len(ok(C, matus)["contacts"])
            assert count == 5, count
            for version in ("3.0", "4.0"):
                laptop.succeed(f"curl -skf -H 'Cookie: {matus}' -o /tmp/out-{version}.vcf "
                               f"'https://{HOST}{C}/export?version={version}'")
                text = laptop.succeed(f"cat /tmp/out-{version}.vcf")
                assert text.count("BEGIN:VCARD") == count and f"VERSION:{version}" in text, text[:400]
                assert all(len(l.encode()) <= 75 for l in text.split("\r\n")), "a line is not folded"
            report = upload(f"{C}/import", eva, "/tmp/out-4.0.vcf")
            assert report["added"] == count, report
            assert names(ok(C, eva)) == names(ok(C, matus))
            mine = {c["name"]: c for c in ok(C, matus)["contacts"]}
            hers = {c["name"]: c for c in ok(C, eva)["contacts"]}
            for name in mine:
                for field in ("emails", "phones", "matrix", "mesh", "org", "birthday", "note"):
                    assert mine[name].get(field) == hers[name].get(field), (name, field, mine[name], hers[name])
            assert hers["Eva Nováková"].get("photo"), hers["Eva Nováková"]
            # Eva's land in her own book, over CardDAV too.
            code, body = dav(f"eva:{EVA_PASSWORD}", "PROPFIND", "eva/contacts/", "-H 'Depth: 1'")
            assert code == 207 and body.count(".vcf</d:href>") == count, (code, body[:400])
            # One contact on its own.
            one = laptop.succeed(f"curl -skf -H 'Cookie: {matus}' "
                                 f"'https://{HOST}{C}/export?ids={mine['Jana Kováčová']['id']}'")
            assert one.count("BEGIN:VCARD") == 1 and "FN:Jana Kováčová" in one, one

        with subtest("the pickers offer contacts and the people on this box"):
            # Eva on the box is a contact of Matus's (by Matrix ID): offered once, as the contact.
            found = ok("/api/contacts/suggest?q=eva&want=matrix", matus)["contacts"]
            assert [(s["source"], s["name"], s.get("matrixId")) for s in found] == [
                ("contact", "Eva Nováková", "@eva:mattbox.local")], found
            # The book's "On this box" view has her anyway.
            found = ok("/api/contacts/suggest?q=&want=box", matus)["contacts"]
            assert [(s["source"], s["name"]) for s in found] == [("box", "Eva")], found
            # Matus is not in Eva's book; he is on the box.
            found = ok("/api/contacts/suggest?q=matus&want=matrix", eva)["contacts"]
            assert ("box", "Matus", "@matus:mattbox.local") in [
                (s["source"], s["name"], s.get("matrixId")) for s in found], found
            # A word that starts with it first: Kováčová before Nováková.
            found = ok("/api/contacts/suggest?q=kov&want=email", matus)["contacts"]
            assert [s["name"] for s in found] == ["Jana Kováčová", "Eva Nováková"], found
            assert found[0]["emails"] == ["jana@pekaren.example"], found
            found = ok("/api/contacts/suggest?q=jana&want=mesh", matus)["contacts"]
            assert found[0]["meshAddress"] == "jana@kitchenbox.local", found
            # Accents are not needed to find someone, in any book.
            found = ok("/api/contacts/suggest?q=tomas&want=any", matus)["contacts"]
            assert found[0]["name"] == "Tomáš Bielik", found
            found = ok("/api/contacts/suggest?q=silvia&want=email", matus)["contacts"]
            assert found[0]["emails"] == ["silvia@firma.example"], found

        with subtest("with sharing off the book stays, without chat and mail"):
            switch("unshared")
            box.wait_until_fails("systemctl is-active losos-chat.service")
            assert len(ok(C, matus)["contacts"]) == 5
            assert "contacts" in ok("/api/me", matus)["shared"]
            assert "chat" not in ok("/api/me", matus)["shared"]
            # The pickers still offer the book and the box, with no Matrix IDs.
            found = ok("/api/contacts/suggest?q=&want=box", matus)["contacts"]
            assert [(s["source"], s["name"], s.get("matrixId")) for s in found] == [("box", "Eva", None)], found
            code = curl(f"-o /dev/null -w '%{{http_code}}' -H 'Cookie: {matus}' https://{HOST}/contacts")
            assert code == "200", code
            browser("off")

        with subtest("with sharing on again the pages use the book in Mail and Chat"):
            switch(None)
            box.wait_for_unit("losos-chat.service")
            box.wait_until_succeeds("curl -sf http://127.0.0.1:8008/_matrix/client/versions")
            browser("on")
            # The browser imported the 2.1 card and added one contact.
            assert "Ondrej Rak" in names(ok(C, matus))
            assert "Lucia Malá" in names(ok(C, matus))

        with subtest("the launch film's pictures"):
            browser("film")

        with subtest("the handbook's pictures"):
            handbook_pictures(box, f"https://{HOST}", "matus", PASSWORD, [
                {"name": "contacts", "path": "/contacts",
                 "waitFor": {"selector": 'button:has-text("Eva Nováková")'},
                 "steps": [{"click": {"selector": 'button:has-text("Eva Nováková")'}}],
                 "settle": 1500},
            ], proxy="socks5://127.0.0.1:1080")
  '';
}
