# nixos-test-vms config for LosOS Office's conversions
# (modules/office-convert.nix, backend/plugins/office,
# backend/mdx/src/bin/losos-convert.rs): a document to Word, OpenDocument,
# PDF and a web page, a presentation to PowerPoint, and a Word file back to
# a document, on a box with LosOS Cloud, the real front vhost and LosOS
# Login.
#
# Checked on the box and from the owner's laptop on the LAN:
#
#   * the image is loaded into losos-convert's own Podman at boot, and the
#     socket is root's alone;
#   * through nginx and lososd (the spare key), a document with an equation,
#     a footnote, a callout, a table and a picture comes back as Word with
#     the equation as Word maths and the picture inside, as OpenDocument,
#     as a PDF and as a web page with MathML and no script; a presentation
#     of two slides as PowerPoint with two slides and its speaker notes;
#   * that Word file comes back as a document, the picture beside it;
#   * a document full of script is as inert in every format, a body over
#     64 MiB is refused, an unknown format is 404 and someone not signed in
#     gets nothing;
#   * Chromium (nix/tests/office-convert-shots.mjs) downloads both from the
#     menu above the page and opens a Word file from LosOS Cloud as a
#     document, for the pictures;
#   * with `beforeUi` given, the box switches to the admin UI as it was
#     before (specialisation `before`) for the pictures' "before" column.
{
  pkgs,
  beforeUi ? null,
}:

let
  inherit (pkgs) lib;
  lososPkgs = import ../packages.nix { inherit pkgs; };

  noLab = import ./admin-ui-without-lab.nix;

  picture = pkgs.runCommand "tomato.png" { } ''
    ${pkgs.librsvg}/bin/rsvg-convert -w 320 ${../../admin-ui/app/tests/fixtures/vault/attachments/tomato.svg} -o $out
  '';

  garden = pkgs.writeText "Garden day.paper" ''
    ---
    page:
      size: A4
    ---
    # Garden day

    The bed is $A = \pi r^{2}$ square metres, with **everyone** there.[^1]

    <Callout type="tip" title="Water">
    Water in the morning.
    </Callout>

    | Bed | Crop |
    | --- | --- |
    | 1 | Tomato |
    | 2 | Bean |

    <Figure src="tomato.png" alt="A tomato" caption="The first tomato" />

    [^1]: Bring a hat if it is sunny.
  '';

  talk = pkgs.writeText "Talk.slides" ''
    <Slide layout="title">
    # The allotment

    Saturday
    </Slide>

    <Slide layout="content">
    ## This year

    - Tomatoes
    - Beans

    <Notes>
    Thank the neighbours.
    </Notes>
    </Slide>
  '';

  hostile = pkgs.writeText "Hostile.paper" ''
    # Hello

    <script>window.__ran = 1</script>

    <img src="x" onerror="window.__ran = 2" />

    {window.__ran = 3}

    <Callout type="note" onClick="window.__ran = 4">Boo</Callout>

    <a href="javascript:window.__ran = 5">click</a>

    [link](javascript:alert(6))

    export const x = (window.__ran = 7)
  '';

  # The bodies the page would send: the text and its pictures in base64.
  request =
    name: source: pictures:
    pkgs.runCommand name { nativeBuildInputs = [ pkgs.jq ]; } ''
      jq -n --rawfile source ${source} ${
        lib.concatMapStringsSep " " (p: "--rawfile ${p.key} <(base64 -w0 ${p.file})") pictures
      } '{source: $source, lang: "en-GB", pictures: {${
        lib.concatMapStringsSep ", " (p: builtins.toJSON p.name + ": $" + p.key) pictures
      }}}' > $out
    '';
  gardenRequest = request "garden.json" garden [
    {
      key = "tomato";
      name = "tomato.png";
      file = picture;
    }
  ];
  talkRequest = request "talk.json" talk [ ];
  hostileRequest = request "hostile.json" hostile [ ];

  shots = pkgs.writeShellScriptBin "office-convert-shots" ''
    export PLAYWRIGHT_CORE=${pkgs.playwright-driver}/index.mjs
    export PLAYWRIGHT_BROWSERS_PATH=${
      pkgs.playwright-driver.browsers.override {
        withFirefox = false;
        withWebkit = false;
      }
    }
    export PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS=true
    exec ${pkgs.bun}/bin/bun ${./office-convert-shots.mjs} "$@"
  '';
in

pkgs.testers.nixosTest {
  name = "losos-office-convert";

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
        ../../modules/office-convert.nix
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
        office.convert = {
          package = lososPkgs.losos-convert;
          image = lososPkgs.losos-image-convert;
        };
      };
      specialisation = lib.mkIf (beforeUi != null) {
        before.configuration.losos.admin.ui = lib.mkForce (noLab beforeUi);
      };
      environment.systemPackages = [
        pkgs.jq
        pkgs.curl
      ];
      virtualisation = {
        memorySize = 3072;
        cores = 2;
        diskSize = 6144;
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
        pkgs.unzip
        pkgs.poppler-utils
        shots
      ];
      fonts.packages = [ pkgs.dejavu_fonts ];
      virtualisation = {
        memorySize = 3072;
        cores = 2;
        diskSize = 3072;
        vlans = [ 1 ];
      };
    };
  };

  testScript = ''
    import json
    import urllib.parse

    HOST = "mattbox.local"
    BASE = f"https://{HOST}"
    CA = "/etc/losos-box.pem"
    PASSWORD = "Correct-Horse-Battery-9"
    BEFORE = ${if beforeUi == null then "False" else "True"}
    Q = urllib.parse.quote

    box.succeed("true")
    laptop.succeed("true")
    box.wait_for_unit("nginx.service")
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("losos-cloud.service")
    box.fail("systemctl cat coolwsd.service")

    with subtest("the converter's image is in its own Podman, and its socket is root's"):
        box.wait_for_unit("losos-convert-image.service", timeout=900)
        images = box.succeed(
            "runuser -u losos-convert -- env HOME=/var/lib/losos-convert XDG_RUNTIME_DIR=/run/losos-convert"
            " CONTAINERS_STORAGE_CONF=$(systemctl show -P Environment losos-convert-image.service"
            " | tr ' ' '\\n' | sed -n 's/^CONTAINERS_STORAGE_CONF=//p')"
            " podman images --format '{{.Repository}}:{{.Tag}}'"
        )
        assert "localhost/losos-convert:" in images, images
        assert box.succeed("stat -c '%U %a' /run/losos-convert.sock").strip() == "root 600"
        box.succeed("systemctl is-active losos-convert.socket")

    cert = box.succeed("cat /var/lib/losos-tls/cert.pem")
    laptop.succeed(f"cat > {CA} <<'EOF'\n{cert}EOF")

    def curl(args):
        return laptop.succeed(f"curl -s --cacert {CA} {args}")

    with subtest("the owner claims the box"):
        laptop.wait_until_succeeds(f"getent hosts {HOST}", timeout=120)
        laptop.wait_until_succeeds(
            f"curl -sf --cacert {CA} {BASE}/api/setup/claim | jq -e .ready", timeout=600
        )
        out = curl(
            f"-w '\\n%{{http_code}}' -X POST {BASE}/api/setup/claim"
            " -H 'Content-Type: application/json'"
            f" -d '{json.dumps({"name": "matus", "password": PASSWORD})}'"
        )
        body, code = out.rsplit("\n", 1)
        assert code == "200", out
        SPARE = json.loads(body)["token"]
        box.wait_for_unit("authelia-losos.service")
        box.wait_for_open_port(9091)

    def office(path, body, out, ctype="application/json"):
        """POSTs a file at /api/office/PATH with the spare key; the code."""
        return laptop.succeed(
            f"curl -s --cacert {CA} -o {out} -w '%{{http_code}}' -X POST {BASE}/api/office/{path}"
            f" -H 'Authorization: Bearer {SPARE}' -H 'Content-Type: {ctype}' --data-binary @{body}"
        ).strip()

    laptop.succeed("mkdir -p /tmp/c && cp ${gardenRequest} /tmp/c/garden.json && cp ${talkRequest} /tmp/c/talk.json && cp ${hostileRequest} /tmp/c/hostile.json")

    with subtest("the page learns what the box converts"):
        formats = json.loads(curl(f"-f -H 'Authorization: Bearer {SPARE}' {BASE}/api/office/formats"))
        assert formats == {"exports": ["docx", "odt", "pdf", "html", "pptx"], "imports": ["docx", "odt"]}, formats

    with subtest("a document becomes Word, with its equation as Word maths and its picture inside"):
        assert office("export/docx", "/tmp/c/garden.json", "/tmp/c/garden.docx") == "200"
        laptop.succeed("unzip -t /tmp/c/garden.docx")
        doc = laptop.succeed("unzip -p /tmp/c/garden.docx word/document.xml")
        for want in ["Garden day", "<m:oMath", "Tomato", "everyone", "Water in the morning"]:
            assert want in doc, f"{want!r} not in the Word file"
        notes = laptop.succeed("unzip -p /tmp/c/garden.docx word/footnotes.xml")
        assert "Bring a hat" in notes, notes
        media = laptop.succeed("unzip -l /tmp/c/garden.docx")
        assert "word/media/" in media, media
        assert "Callout" in laptop.succeed("unzip -p /tmp/c/garden.docx word/document.xml word/styles.xml")

    with subtest("as OpenDocument, as a PDF and as a web page with MathML and no script"):
        assert office("export/odt", "/tmp/c/garden.json", "/tmp/c/garden.odt") == "200"
        assert "Garden day" in laptop.succeed("unzip -p /tmp/c/garden.odt content.xml")
        assert office("export/pdf", "/tmp/c/garden.json", "/tmp/c/garden.pdf") == "200"
        assert laptop.succeed("head -c 5 /tmp/c/garden.pdf") == "%PDF-"
        text = laptop.succeed("pdftotext /tmp/c/garden.pdf -")
        assert "Garden day" in text and "Tomato" in text, text
        info = laptop.succeed("pdfinfo /tmp/c/garden.pdf")
        assert "(A4)" in info, info
        assert office("export/html", "/tmp/c/garden.json", "/tmp/c/garden.html") == "200"
        page = laptop.succeed("cat /tmp/c/garden.html")
        assert "<math" in page and "Garden day" in page, page[:400]
        assert "<script" not in page.lower(), "the web page carries script"
        assert "data:image/png;base64," in page, "the picture is inside the page"

    with subtest("a presentation becomes PowerPoint, two slides and the speaker's notes"):
        assert office("export/pptx", "/tmp/c/talk.json", "/tmp/c/talk.pptx") == "200"
        slides = laptop.succeed("unzip -l /tmp/c/talk.pptx | grep -c 'ppt/slides/slide[0-9]*.xml$'").strip()
        assert slides == "2", slides
        assert "Tomatoes" in laptop.succeed("unzip -p /tmp/c/talk.pptx 'ppt/slides/slide2.xml'")
        assert "Thank the neighbours" in laptop.succeed("unzip -p /tmp/c/talk.pptx 'ppt/notesSlides/*.xml'")

    with subtest("the Word file comes back as a document, its picture beside it"):
        assert office("import/docx?folder=Garden-files", "/tmp/c/garden.docx", "/tmp/c/back.json", "application/octet-stream") == "200"
        back = json.loads(laptop.succeed("cat /tmp/c/back.json"))
        source = back["source"]
        for want in ["# Garden day", "$A = \\pi r^{2}$", "**everyone**", "| Bed | Crop", "| 2   | Bean", "Bring a hat"]:
            assert want in source, f"{want!r} not in {source!r}"
        assert any(k.startswith("Garden-files/media/") for k in back["pictures"]), back["pictures"].keys()
        assert office("import/odt?folder=Garden-files", "/tmp/c/garden.odt", "/tmp/c/back-odt.json", "application/octet-stream") == "200"
        assert "Garden day" in json.loads(laptop.succeed("cat /tmp/c/back-odt.json"))["source"]

    with subtest("a document full of script is inert in every format"):
        for fmt in ["html", "docx", "odt"]:
            assert office(f"export/{fmt}", "/tmp/c/hostile.json", f"/tmp/c/hostile.{fmt}") == "200"
        page = laptop.succeed("cat /tmp/c/hostile.html").lower()
        for bad in ["<script", "onerror", "onclick", "javascript:"]:
            assert bad not in page, f"{bad} in the web page"
        for fmt, part in [("docx", "word/document.xml"), ("odt", "content.xml")]:
            inner = laptop.succeed(f"unzip -p /tmp/c/hostile.{fmt} {part}").lower()
            assert "javascript:" not in inner, f"javascript: in the {fmt}"

    with subtest("what is not a conversion is refused"):
        assert office("export/exe", "/tmp/c/garden.json", "/dev/null") == "404"
        assert office("import/pdf", "/tmp/c/garden.pdf", "/dev/null", "application/octet-stream") == "404"
        assert office("import/docx?folder=..%2Fetc", "/tmp/c/garden.docx", "/dev/null", "application/octet-stream") == "400"
        laptop.succeed("head -c 70000000 /dev/zero > /tmp/c/big")
        assert office("export/docx", "/tmp/c/big", "/dev/null") == "413"
        code = laptop.succeed(
            f"curl -s --cacert {CA} -o /dev/null -w '%{{http_code}}' -X POST {BASE}/api/office/export/docx"
            " -H 'Content-Type: application/json' --data-binary @/tmp/c/garden.json"
        ).strip()
        assert code in ("302", "401", "403"), code

    with subtest("Chromium downloads both from the menu above the page and opens a Word file as a document"):
        def put(local, name):
            laptop.succeed(f"curl -sf --cacert {CA} -u 'matus:{PASSWORD}' -T {local} '{BASE}/remote.php/webdav/{Q(name)}'")

        laptop.wait_until_succeeds(
            f"curl -sf --cacert {CA} -u 'matus:{PASSWORD}' -X PROPFIND -H 'Depth: 0' {BASE}/remote.php/webdav/ -o /dev/null",
            timeout=120,
        )
        laptop.succeed("cp ${picture} /tmp/c/tomato.png")
        put("${garden}", "Garden day.paper")
        put("/tmp/c/tomato.png", "tomato.png")
        put("${talk}", "Talk.slides")
        put("/tmp/c/garden.docx", "Old report.docx")
        laptop.succeed("rm -rf /tmp/shots")
        try:
            laptop.succeed(
                f"office-convert-shots {BASE} matus '{PASSWORD}' /tmp/shots/after --test losos-office-convert >&2"
            )
        finally:
            laptop.copy_from_machine("/tmp/shots/after", "shots")
        got = laptop.succeed("unzip -p '/tmp/shots/after/downloads/Garden day.docx' word/document.xml")
        assert "<m:oMath" in got and "Garden day" in got
        assert laptop.succeed("head -c 5 '/tmp/shots/after/downloads/Garden day.pdf'") == "%PDF-"
        assert "<script" not in laptop.succeed("cat '/tmp/shots/after/downloads/Garden day.html'").lower()
        assert laptop.succeed("unzip -l /tmp/shots/after/downloads/Talk.pptx | grep -c 'ppt/slides/slide[0-9]*.xml$'").strip() == "2"

    with subtest("the document made from the Word file is beside it, and the Word file is as it was"):
        made = curl(f"-f -u 'matus:{PASSWORD}' '{BASE}/remote.php/webdav/Old%20report.paper'")
        assert "# Garden day" in made and "Old-report-files/media/" in made, made
        curl(f"-f -u 'matus:{PASSWORD}' -o /tmp/c/again.docx '{BASE}/remote.php/webdav/Old%20report.docx'")
        laptop.succeed("cmp /tmp/c/again.docx /tmp/c/garden.docx")
        listing = curl(f"-f -u 'matus:{PASSWORD}' -X PROPFIND -H 'Depth: infinity' {BASE}/remote.php/webdav/Old-report-files/")
        assert "image" in listing, listing

    if BEFORE:
        with subtest("the pictures: the same places before"):
            # The folder as it was before the import, so both runs start alike.
            curl(f"-f -u 'matus:{PASSWORD}' -X DELETE '{BASE}/remote.php/webdav/Old%20report.paper'")
            curl(f"-f -u 'matus:{PASSWORD}' -X DELETE '{BASE}/remote.php/webdav/Old-report-files/'")
            box.succeed("/run/current-system/specialisation/before/bin/switch-to-configuration test")
            laptop.succeed(f"office-convert-shots {BASE} matus '{PASSWORD}' /tmp/shots/before --before >&2")
            laptop.copy_from_machine("/tmp/shots/before", "shots")
  '';
}
