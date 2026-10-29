# nixos-test-vms config for Photos in LosOS Cloud
# (backend/src/cloud/photos/, `losos-cloud-photos` in modules/cloud.nix).
#
# The owner uploads thirteen pictures over WebDAV, the way the phone app
# does: NASA's portraits of three astronauts and a group photo from the
# space station (public domain, real faces for the models to tell apart),
# and four landscapes drawn at build time (nix/tests/cloud-photos-draw.py)
# with the EXIF block a phone writes, places included. Half go up while the
# background pass is already reading the first half, and every upload still
# answers at once. Then the test checks that
#
#   * every photo was read once, and a second pass has nothing to do;
#   * each astronaut's photos are one person and no one else's, with the
#     two models running on the box's own CPU and nothing fetched;
#   * the camera's details came through: the date it was taken, the camera
#     and lens, the exposure, and the place with its coordinates;
#   * the owner can name a person, and giving two people one name makes
#     them one;
#   * a face's picture is a small JPEG, and another person on the box sees
#     none of it: no photo, no face, no name, and cannot rename anyone;
#   * with losos.cloud.photos.faces off, the next pass deletes every face
#     and person and keeps the camera's details.
#
# Last, the portraits go to the trash and the launch film's frames are taken
# of the landscapes alone.
#
# The pictures for the pull request come from a browser on the box
# (nix/tests/cloud-photos-shots.mjs). `beforeUi`, when given, is the admin
# UI as it was before Photos grouped anything: the box switches to it for
# the "before" pictures. The flake check runs without it.
{
  pkgs,
  beforeUi ? null,
}:

let
  inherit (pkgs) lib;
  lososPkgs = import ../packages.nix { inherit pkgs; };

  # The admin UI without LosOS Diagrams and the Lab, whose WebAssembly
  # (Bevy among it) takes longer to build than this test runs: their pages
  # are left out and their modules are empty stand-ins. Nothing here opens
  # either; the shipped package builds both.
  noLab =
    ui:
    ui.overrideAttrs {
      preBuild = ''
        for pkg in core-pkg/losos_lab_core virt-pkg/losos_lab_virt \
            render-pkg/webgpu/losos_lab_render render-pkg/webgl2/losos_lab_render; do
          mkdir -p "src/lab/$(dirname "$pkg")"
          printf 'export default async function init() {}\nexport class Lab {}\n' > "src/lab/$pkg.js"
          : > "src/lab/''${pkg}_bg.wasm"
        done
      '';
      buildPhase = ''
        runHook preBuild
        cat > vite.nolab.config.ts <<'EOF'
        import config from "./vite.config";
        for (const page of ["diagrams", "lab"]) delete (config.build!.rolldownOptions!.input as Record<string, string>)[page];
        export default config;
        EOF
        bun --bun vite build --config vite.nolab.config.ts
        runHook postBuild
      '';
    };

  nasa = id: size: sha256: {
    name = "${id}.jpg";
    path = pkgs.fetchurl {
      url = "https://images-assets.nasa.gov/image/${id}/${id}~${size}.jpg";
      inherit sha256;
    };
  };
  # Three of each astronaut, from different years, cameras and lights. The
  # originals keep their camera's EXIF block; NASA's smaller copies drop it.
  photos = pkgs.linkFarm "losos-test-photos" [
    # Victor Glover
    (nasa "KSC-20200814-PH-SPX01_0005" "medium"
      "1cb0a42a09c7201e0e6d2aec3cf23ed391b68b238bb1b5f39620fbeb899be008"
    )
    (nasa "NHQ202009150002" "medium" "3a36a61de3c9000c76c4c85cc5abca3b06468830834ae8b78f6ee349a49a6ed9")
    # The Crew-1 crew, Glover second from the left.
    (nasa "jsc2020e032979" "medium" "9dc7fb71452e35bea0faf20dbc8a9c003290cd85d2976e37f7f37fd00f474e2b")
    # Jessica Meir
    (nasa "NHQ202009150005" "medium" "c99c8f145e5083ecb0b5d47722a510ccfea94f47bb1a524c6b7a16fe4e2308e5")
    (nasa "jsc2025e078605_alt" "medium"
      "3177035d2ef10190bf921fc223cd2624293c75a6e74d9e32c071d31df1c08473"
    )
    (nasa "iss074e0365172" "orig" "4426bf3da293dadfacf17501bfc40c0ef96638e7c2f9af0b8c189f869a4bb979")
    # Christina Koch
    (nasa "NHQ202009160004" "medium" "6a497fe163d661c8f9504ca3075c6964b8ea7a07375b92769a9ca2f15327b4dd")
    (nasa "jsc2018e095073_alt" "orig"
      "c9d1054dd50010477099b093e73aaff95759ab19099c143f5c18cb0b298ae96a"
    )
    # Expedition 59, Koch among them.
    (nasa "iss059e098878" "orig" "774afc687eea73b90ae063589c56c13ce613b4dd2a7c8880da8a196ff290e818")
  ];
  landscapes = pkgs.runCommand "losos-test-landscapes" {
    nativeBuildInputs = [ (pkgs.python3.withPackages (ps: [ ps.pillow ])) ];
  } "python3 ${./cloud-photos-draw.py} $out";
  pictures = pkgs.symlinkJoin {
    name = "losos-test-pictures";
    paths = [
      photos
      landscapes
    ];
  };

  shots = pkgs.writeShellScriptBin "cloud-photos-shots" ''
    export PLAYWRIGHT_CORE=${pkgs.playwright-driver}/index.mjs
    export PLAYWRIGHT_BROWSERS_PATH=${
      pkgs.playwright-driver.browsers.override {
        withFirefox = false;
        withWebkit = false;
      }
    }
    export PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS=true
    exec ${pkgs.bun}/bin/bun ${./cloud-photos-shots.mjs} "$@"
  '';
in

pkgs.testers.nixosTest {
  name = "losos-cloud-photos";
  # The face pass runs at idle priority on one core; emulated, without KVM,
  # a camera original takes minutes.
  globalTimeout = 4 * 3600;

  nodes.box = _: {
    imports = [
      ../../modules/options.nix
      ../../modules/mdns.nix
      ../../modules/containers.nix
      ../../modules/tls.nix
      ../../modules/daemon.nix
      ../../modules/identity.nix
      ../../modules/cloud.nix
    ];
    networking.hostName = "mattbox";
    losos = {
      hostName = "mattbox";
      backend.package = lososPkgs.losos-ctl;
      admin.ui = noLab lososPkgs.losos-admin-ui;
      cloud.enable = true;
      forgejo.enable = false;
    };
    specialisation = {
      nofaces.configuration.losos.cloud.photos.faces = false;
    }
    // lib.optionalAttrs (beforeUi != null) {
      before.configuration.losos.admin.ui = lib.mkForce (noLab beforeUi);
    };
    environment.systemPackages = [
      pkgs.jq
      pkgs.curl
      shots
    ];
    environment.etc."losos-test-photos".source = pictures;
    fonts.packages = [ pkgs.dejavu_fonts ];
    virtualisation = {
      memorySize = 3072;
      cores = 2;
      diskSize = 4096;
    };
  };

  testScript = ''
    import json
    import time

    PASSWORD = "Correct-Horse-Battery-9"
    CLOUD = "${lososPkgs.losos-ctl}/bin/losos-cloud --root /var/lib/losos-cloud"
    BEFORE = ${if beforeUi == null then "False" else "True"}

    def api(method, path, who="notshared", body=None):
        data = "" if body is None else f" -H 'Content-Type: application/json' -d '{json.dumps(body)}'"
        return (
            f"curl -sS --unix-socket /run/losos-cloud/http.sock -X {method}"
            f" -H 'X-Losos-Cloud-User: {who}' -w '\\n%{{http_code}}' http://box/cloud/api{path}{data}"
        )

    def call(method, path, who="notshared", body=None, status=200):
        out = box.succeed(api(method, path, who, body))
        text, code = out.rsplit("\n", 1)
        assert int(code) == status, f"{method} {path} as {who}: {code} {text}"
        return json.loads(text) if text.strip().startswith(("{", "[")) else text

    def pass_state():
        return [
            box.succeed(f"systemctl show -P {p} losos-cloud-photos.service").strip()
            for p in ("ActiveState", "ExecMainExitTimestampMonotonic")
        ]

    def wait_for_pass(before=None):
        # A oneshot is "activating" while it runs. Emulated, a camera
        # original takes minutes, so this says which photo it is on.
        deadline = time.monotonic() + 3 * 3600
        while True:
            state, ended = pass_state()
            if state != "activating" and ended != before:
                break
            assert time.monotonic() < deadline, "the pass did not finish in three hours"
            print(box.execute("journalctl -u losos-cloud-photos.service -o cat | grep '^photos: ' | tail -n 1")[1].strip())
            time.sleep(30)
        box.succeed("systemctl show -P Result losos-cloud-photos.service | grep -qx success")

    def run_pass():
        before = pass_state()[1]
        box.succeed("systemctl start --no-block losos-cloud-photos.service")
        wait_for_pass(before)

    def upload(name):
        start = time.monotonic()
        box.succeed(
            f"curl -sSf -k -u notshared:{app_password} -T /etc/losos-test-photos/{name}"
            f" https://127.0.0.1/remote.php/dav/files/notshared/Pictures/{name}"
        )
        return time.monotonic() - start

    box.start()
    box.wait_for_unit("losos-cloud.service")
    box.wait_for_unit("nginx.service")
    # Switching to a specialisation moves /run/current-system; the others
    # are reached from the system the box booted.
    BOOTED = box.succeed("readlink -f /run/current-system").strip()

    with subtest("the owner's photos go up while the pass reads the first ones"):
        app_password = box.succeed(f"runuser -u losos-cloud -- {CLOUD} app-password --login notshared").strip()
        box.succeed(
            f"curl -sSf -k -u notshared:{app_password} -X MKCOL"
            " https://127.0.0.1/remote.php/dav/files/notshared/Pictures"
        )
        names = sorted(box.succeed("ls /etc/losos-test-photos").split())
        assert len(names) == 13, names
        first, rest = names[:6], names[6:]
        for n in first:
            upload(n)
        box.succeed("systemctl start --no-block losos-cloud-photos.service")
        box.wait_until_succeeds("systemctl show -P ActiveState losos-cloud-photos.service | grep -qx activating", timeout=60)
        took = [upload(n) for n in rest]
        print("uploads during the pass took", [round(t, 2) for t in took], "seconds")
        busy = box.succeed("systemctl is-active losos-cloud-photos.service || true").strip()
        print("the pass after the uploads:", busy)
        assert max(took) < 20, f"an upload waited on the pass: {took}"

    with subtest("every photo is read once"):
        # The pass that ran, then one more for what arrived during it.
        wait_for_pass()
        run_pass()
        print(box.succeed("journalctl -u losos-cloud-photos.service -o cat | grep 'photos read'"))
        run_pass()
        # The last pass found nothing new (systemd's own lines come after it).
        box.succeed("journalctl -u losos-cloud-photos.service -o cat | grep 'photos read' | tail -n 1 | grep -q '^0 photos read'")
        # The pass right after boot waits for the server instead of failing.
        box.fail("journalctl -u losos-cloud-photos.service -o cat | grep -q 'database is locked'")
        data = call("GET", "/photos")
        print(json.dumps(data["index"]))
        assert data["index"]["total"] == 13 and data["index"]["done"] == 13, data["index"]
        assert data["index"]["faces"] is True

    by_name = {p["name"]: p for p in data["photos"]}
    people = {p["id"]: p for p in data["people"]}

    def person_of(*names):
        sets = [set(by_name[n]["people"]) for n in names]
        common = set.intersection(*sets)
        assert len(common) == 1, f"{names} share {common}"
        return common.pop()

    def photos_of(pid):
        return {n for n, p in by_name.items() if pid in p["people"]}

    with subtest("each astronaut is one person, and no one else"):
        glover = person_of("KSC-20200814-PH-SPX01_0005.jpg", "NHQ202009150002.jpg", "jsc2020e032979.jpg")
        meir = person_of("NHQ202009150005.jpg", "jsc2025e078605_alt.jpg", "iss074e0365172.jpg")
        koch = person_of("NHQ202009160004.jpg", "jsc2018e095073_alt.jpg")
        assert len({glover, meir, koch}) == 3
        assert photos_of(glover) == {"KSC-20200814-PH-SPX01_0005.jpg", "NHQ202009150002.jpg", "jsc2020e032979.jpg"}, photos_of(glover)
        assert photos_of(meir) == {"NHQ202009150005.jpg", "jsc2025e078605_alt.jpg", "iss074e0365172.jpg"}, photos_of(meir)
        assert photos_of(koch) <= {"NHQ202009160004.jpg", "jsc2018e095073_alt.jpg", "iss059e098878.jpg"}, photos_of(koch)
        assert not any(by_name[n]["people"] for n in ("old-town.jpg", "danube.jpg", "vienna.jpg", "tatras.jpg"))
        print("Koch's photos:", sorted(photos_of(koch)))

    with subtest("the camera's details came through"):
        m = by_name["iss074e0365172.jpg"]
        assert m["camera"] == "NIKON Z 9", m
        assert m["taken"].startswith("2026-03-09"), m
        e = by_name["iss059e098878.jpg"]
        assert e["camera"] == "NIKON D5" and e["taken"].startswith("2019-06-08"), e
        b = by_name["old-town.jpg"]
        assert abs(b["lat"] - 48.1437) < 0.001 and abs(b["lon"] - 17.1080) < 0.001, b
        assert b["camera"] == "Google Pixel 8" and b["taken"] == "2025-05-17T18:42:10", b
        d = call("GET", f"/photos/{m['id']}")
        assert d["summary"]["lens"] and d["summary"]["exposure"] and d["summary"]["iso"], d["summary"]
        groups = {f["group"] for f in d["fields"]}
        assert {"picture", "camera"} <= groups, groups
        assert not any(f["name"] == "MakerNote" for f in d["fields"])
        assert any(f["person"] == meir for f in d["faces"]), d["faces"]
        place = call("GET", f"/photos/{b['id']}")
        assert any(f["group"] == "place" for f in place["fields"]), place["fields"]

    with subtest("a face's picture is a small JPEG"):
        face = people[meir]["face"]
        box.succeed(
            "curl -sSf --unix-socket /run/losos-cloud/http.sock -H 'X-Losos-Cloud-User: notshared'"
            f" -o /tmp/face.jpg http://box/cloud/api/photos/faces/{face}"
        )
        box.succeed("head -c 3 /tmp/face.jpg | od -An -tx1 | grep -q 'ff d8 ff'")

    with subtest("the owner names people; one name makes two people one"):
        assert call("POST", f"/photos/people/{glover}", body={"name": "Victor Glover"})["id"] == glover
        assert call("POST", f"/photos/people/{meir}", body={"name": "Jessica Meir"})["id"] == meir
        assert call("POST", f"/photos/people/{koch}", body={"name": "Christina Koch"})["id"] == koch
        crew = [p for p in by_name["jsc2020e032979.jpg"]["people"] if p != glover]
        assert len(crew) >= 2, crew
        assert call("POST", f"/photos/people/{crew[0]}", body={"name": "Crew-1"})["id"] == crew[0]
        assert call("POST", f"/photos/people/{crew[1]}", body={"name": "crew-1 "})["id"] == crew[0]
        after = call("GET", "/photos")
        named = {p["name"]: p for p in after["people"]}
        assert {"Victor Glover", "Jessica Meir", "Christina Koch", "Crew-1"} <= set(named), named
        assert crew[1] not in {p["id"] for p in after["people"]}
        call("POST", f"/photos/people/{glover}", body={"name": "x" * 81}, status=400)

    with subtest("another person on the box sees none of it"):
        other = call("GET", "/photos", who="eva")
        assert other["photos"] == [] and other["people"] == [], other
        call("GET", f"/photos/faces/{face}", who="eva", status=404)
        call("GET", f"/photos/{m['id']}", who="eva", status=404)
        call("POST", f"/photos/people/{glover}", who="eva", body={"name": "Mallory"}, status=404)
        assert call("GET", "/photos")["people"][0]["name"] != "Mallory"

    with subtest("the pictures"):
        box.wait_until_succeeds("curl -sf 127.0.0.1:8082/api/setup/claim | jq -e .ready", timeout=600)
        box.succeed(
            "curl -sf -X POST 127.0.0.1:8082/api/setup/claim"
            " -H 'Content-Type: application/json'"
            f" -d '{{\"name\": \"matus\", \"password\": \"{PASSWORD}\"}}'"
        )
        box.wait_for_unit("authelia-losos.service")
        box.wait_for_open_port(9091)
        status, out = box.execute(f"cloud-photos-shots after /tmp/shots/after matus '{PASSWORD}' 127.0.0.1 2>&1", timeout=3000)
        print(out)
        box.copy_from_machine("/tmp/shots/after", "shots")
        assert status == 0, "the pictures failed; shots/after/failed.png shows where"
        if BEFORE:
            box.succeed(f"{BOOTED}/specialisation/before/bin/switch-to-configuration test")
            box.wait_for_unit("nginx.service")
            status, out = box.execute(f"cloud-photos-shots before /tmp/shots/before matus '{PASSWORD}' 127.0.0.1 2>&1", timeout=3000)
            print(out)
            box.copy_from_machine("/tmp/shots/before", "shots")
            assert status == 0, "the before pictures failed; shots/before/failed.png shows where"

    with subtest("faces off: every face and person goes, the camera's details stay"):
        box.succeed(f"{BOOTED}/specialisation/nofaces/bin/switch-to-configuration test")
        run_pass()
        off = call("GET", "/photos")
        assert off["index"]["faces"] is False and off["people"] == [], off["index"]
        assert all(p["people"] == [] for p in off["photos"])
        assert {p["name"]: p for p in off["photos"]}["iss074e0365172.jpg"]["camera"] == "NIKON Z 9"
        assert box.succeed(
            "runuser -u losos-cloud -- ${pkgs.sqlite}/bin/sqlite3 /var/lib/losos-cloud/cloud.db"
            " 'SELECT COUNT(*) FROM photo_faces; SELECT COUNT(*) FROM photo_people;'"
        ).split() == ["0", "0"]

    with subtest("the film's frames: the owner's own trips, no one's face"):
        # The portraits go to the trash and the next pass forgets them, so
        # only the drawn landscapes are left on screen.
        for n in names:
            if n not in ("old-town.jpg", "danube.jpg", "vienna.jpg", "tatras.jpg"):
                box.succeed(
                    f"curl -sSf -k -u notshared:{app_password} -X DELETE"
                    f" https://127.0.0.1/remote.php/dav/files/notshared/Pictures/{n}"
                )
        run_pass()
        left = call("GET", "/photos")
        assert sorted(p["name"] for p in left["photos"]) == ["danube.jpg", "old-town.jpg", "tatras.jpg", "vienna.jpg"]
        status, out = box.execute(f"cloud-photos-shots film /tmp/shots/film matus '{PASSWORD}' 127.0.0.1 2>&1", timeout=3000)
        print(out)
        box.copy_from_machine("/tmp/shots/film", "shots")
        assert status == 0, "the film's frames failed; shots/film/failed.png shows where"
  '';
}
