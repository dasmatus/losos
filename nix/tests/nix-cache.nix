# The local copy of the binary caches (modules/cache.nix, losos.cache.local).
#
# Three machines on one LAN:
#
#   upstream   stands in for cache.nixos.org and the LosOS caches: nginx
#              serving a file:// binary cache, signed with a key made at the
#              start of the run, plus one path signed with a key nobody trusts.
#   box        a box with the local copy on and shared on the LAN.
#   neighbour  a second box that names the first as its peer.
#
# What it proves, in the order a box lives it:
#
#   * the first fetch comes from upstream and lands in /var/cache;
#   * after the paths leave the store (the 04:30 clean-up), the next fetch
#     costs upstream nothing, and still works with upstream switched off;
#   * the copy survives a restart;
#   * the neighbour gets a path from the box over the LAN with upstream off,
#     and a path neither has is downloaded from upstream once for both;
#   * a path signed by an untrusted key is refused even though the copy
#     serves it, and the narinfo the box serves carries upstream's signature
#     unchanged;
#   * a source outside the private ranges is refused;
#   * the copy stays under maxSize, least recently used out first.
#
# The payloads are incompressible and deterministic (AES-CTR over zeros), so
# the byte counts below are the bytes a real box would move.
{ pkgs }:

let
  mib = 8;
  payload =
    name:
    pkgs.runCommand "losos-cache-payload-${name}" { nativeBuildInputs = [ pkgs.openssl ]; } ''
      head -c ${toString (mib * 1024 * 1024)} /dev/zero \
        | openssl enc -aes-256-ctr -nosalt -pbkdf2 -pass pass:${name} > $out
    '';
  a = payload "a";
  b = payload "b";
  c = payload "c";
  d = payload "d";
  e = payload "e";
  rogue = payload "rogue";

  # The key is made at run time, so the boxes learn it from a file the
  # script writes. The module's own trustedPublicKeys stay as they ship.
  testKeys = ''
    !include /var/lib/losos-test/keys.conf
  '';

  boxBase = _: {
    imports = [
      ../../modules/options.nix
      ../../modules/cache.nix
    ];
    losos.cache = {
      # No GHCR helper in a VM without the internet.
      substituters = [ ];
      local.enable = true;
      local.upstreams = [ "http://upstream" ];
    };
    services.resolved.enable = true;
    nix.extraOptions = testKeys;
    environment.systemPackages = [ pkgs.curl ];
    virtualisation = {
      memorySize = 1024;
      cores = 2;
      diskSize = 2048;
    };
  };
in
pkgs.testers.nixosTest {
  name = "losos-nix-cache";

  nodes = {
    upstream = _: {
      system.extraDependencies = [
        a
        b
        c
        d
        e
        rogue
      ];
      nix.settings.experimental-features = [ "nix-command" ];
      services.nginx = {
        enable = true;
        commonHttpConfig = ''
          log_format bytes '$request_uri $body_bytes_sent';
        '';
        virtualHosts.upstream = {
          default = true;
          root = "/srv/cache";
          extraConfig = ''
            access_log /var/log/nginx/bytes.log bytes;
          '';
        };
      };
      systemd.tmpfiles.rules = [ "d /srv/cache 0755 root root -" ];
      networking.firewall.allowedTCPPorts = [ 80 ];
      virtualisation.memorySize = 1024;
    };

    box = _: {
      imports = [ boxBase ];
      losos.cache.local = {
        shareOnLan = true;
        maxSize = "20m";
        minFree = "16m";
      };
    };

    neighbour = _: {
      imports = [ boxBase ];
      losos.cache.local.peers = [ "http://box:8790" ];
    };
  };

  testScript = ''
    import time

    A, B, C, D, E, ROGUE = "${a}", "${b}", "${c}", "${d}", "${e}", "${rogue}"
    MIB = ${toString mib}

    def upstream_bytes():
        return int(upstream.succeed(
            "awk '{s += $2} END {print s + 0}' /var/log/nginx/bytes.log"
        ).strip())

    def trust(m):
        m.succeed(
            "mkdir -p /var/lib/losos-test && "
            f"echo 'extra-trusted-public-keys = {PUB}' > /var/lib/losos-test/keys.conf && "
            "systemctl restart nix-daemon.service"
        )

    def forget(m, *paths):
        # What the 04:30 clean-up does to a rebuild's build-time inputs, and
        # Nix's own memory of narinfos, so every lookup goes to the proxy.
        m.succeed("nix-store --delete " + " ".join(paths))
        m.succeed("rm -rf /root/.cache/nix")
        for p in paths:
            m.fail(f"nix-store --check-validity {p}")

    def fetch(m, *paths):
        t = time.monotonic()
        m.succeed("nix-store --realise " + " ".join(paths))
        return time.monotonic() - t

    def lines(m, status):
        return int(m.succeed(
            f"journalctl -t losos_nix_cache --no-pager -o cat | grep -c 'cache={status}' || true"
        ).strip())

    results = []

    def record(step, moved, t):
        results.append((step, moved, t))
        print(f"RESULT {step}: {moved} bytes from upstream, {t:.2f} s")

    with subtest("upstream publishes signed paths, and one signed by a stranger"):
        upstream.start()
        upstream.wait_for_unit("nginx.service")
        upstream.wait_for_open_port(80)
        upstream.succeed(
            "nix key generate-secret --key-name losos-test-cache-1 > /root/sk",
            "nix key convert-secret-to-public < /root/sk > /root/pub",
            "nix key generate-secret --key-name stranger-1 > /root/stranger-sk",
            f"nix copy --to 'file:///srv/cache?compression=none&secret-key=/root/sk' {A} {B} {C} {D} {E}",
            f"nix copy --to 'file:///srv/cache?compression=none&secret-key=/root/stranger-sk' {ROGUE}",
            "chmod -R a+rX /srv/cache",
        )
        PUB = upstream.succeed("cat /root/pub").strip()

    with subtest("the box substitutes through its local copy only"):
        box.start()
        box.wait_for_unit("losos-nix-cache.service")
        box.wait_for_open_port(8790)
        trust(box)
        conf = box.succeed("nix --extra-experimental-features nix-command config show substituters").strip()
        assert conf.split()[0] == "http://127.0.0.1:8790", conf
        box.succeed("curl -sf http://127.0.0.1:8790/nix-cache-info | grep -q 'Priority: 10'")
        box.succeed("test \"$(stat -c %U /var/cache/losos-nix-cache/store)\" = losos-nix-cache")
        for p in (A, B):
            box.fail(f"nix-store --check-validity {p}")

    with subtest("first fetch: from upstream, kept in /var/cache"):
        before = upstream_bytes()
        t = fetch(box, A, B)
        moved = upstream_bytes() - before
        assert moved >= 2 * MIB * 1024 * 1024, moved
        record("first fetch, cold", moved, t)
        assert lines(box, "MISS") >= 4

    with subtest("the narinfo the box serves carries upstream's signature unchanged"):
        h = A.removeprefix("/nix/store/")[:32]
        ours = box.succeed(f"curl -sf http://127.0.0.1:8790/{h}.narinfo | grep '^Sig:'")
        theirs = upstream.succeed(f"grep '^Sig:' /srv/cache/{h}.narinfo")
        assert ours == theirs, (ours, theirs)

    with subtest("after the clean-up, the next fetch costs upstream nothing"):
        forget(box, A, B)
        before = upstream_bytes()
        t = fetch(box, A, B)
        moved = upstream_bytes() - before
        assert moved == 0, moved
        record("after clean-up, upstream up", moved, t)
        assert lines(box, "HIT") >= 4

    with subtest("and works with upstream off"):
        upstream.succeed("systemctl stop nginx.service")
        forget(box, A, B)
        t = fetch(box, A, B)
        record("after clean-up, upstream off", 0, t)

    with subtest("the copy survives a restart"):
        box.shutdown()
        box.start()
        box.wait_for_unit("losos-nix-cache.service")
        box.wait_for_open_port(8790)
        forget(box, A, B)
        t = fetch(box, A, B)
        record("after a restart, upstream off", 0, t)

    with subtest("the neighbour gets a path from the box over the LAN"):
        neighbour.start()
        neighbour.wait_for_unit("losos-nix-cache.service")
        neighbour.wait_for_open_port(8790)
        trust(neighbour)
        t = fetch(neighbour, A)
        record("neighbour, from the box, upstream off", 0, t)
        box.succeed("journalctl -t losos_nix_cache --no-pager -o cat | grep -q '^192.168.1.2 .*cache=HIT'")

    with subtest("a path neither has is downloaded once for both"):
        upstream.succeed("systemctl start nginx.service")
        upstream.wait_for_open_port(80)
        before = upstream_bytes()
        t = fetch(neighbour, C)
        moved = upstream_bytes() - before
        assert MIB * 1024 * 1024 <= moved < 2 * MIB * 1024 * 1024, moved
        record("neighbour, new path via the box", moved, t)
        box.succeed(f"curl -sf -o /dev/null http://127.0.0.1:8790/{C.removeprefix('/nix/store/')[:32]}.narinfo")
        before = upstream_bytes()
        fetch(box, C)
        assert upstream_bytes() == before

    with subtest("a peer does not pass a peer's request to its own peers"):
        # The neighbour names the box; asked as a peer, it goes straight to
        # upstream and never back to the box.
        nar = "nar/0000000000000000000000000000000000000000000000000000.nar"
        out = neighbour.succeed(
            "curl -s -o /dev/null -w '%{http_code}' -H 'X-Losos-Nix-Cache-Hop: 1' "
            f"http://127.0.0.1:8790/{nar}"
        )
        assert out == "404", out
        line = neighbour.succeed(
            f"journalctl -t losos_nix_cache --no-pager -o cat | grep '{nar}' | tail -n1"
        )
        assert "192.168.1.1:8790" not in line and "192.168.1.3:80" in line, line

    with subtest("a path signed by an untrusted key is refused"):
        out = box.fail(f"nix-store --realise {ROGUE} 2>&1")
        assert "not signed by any of the keys" in out, out
        box.fail(f"nix-store --check-validity {ROGUE}")
        neighbour.fail(f"nix-store --realise {ROGUE}")

    with subtest("only loopback and private addresses are let in"):
        box.succeed("ip addr add 100.64.0.9/24 dev eth1")
        h = A.removeprefix("/nix/store/")[:32]
        for path in ("nix-cache-info", f"{h}.narinfo"):
            code = box.succeed(
                "curl -s -o /dev/null -w '%{http_code}' --interface 100.64.0.9 "
                f"http://192.168.1.1:8790/{path}"
            )
            assert code == "403", (path, code)
        neighbour.succeed("curl -sf http://box:8790/nix-cache-info")
        # The neighbour does not share: its proxy listens on loopback only.
        box.fail("curl -sf --max-time 5 http://neighbour:8790/nix-cache-info")

    with subtest("the copy stays under maxSize, least recently used out first"):
        fetch(box, D, E)
        box.wait_until_succeeds(
            "test $(du -sm /var/cache/losos-nix-cache/store | cut -f1) -le 21", timeout=120
        )
        used = int(box.succeed("du -sm /var/cache/losos-nix-cache/store | cut -f1").strip())
        print(f"cache size after {5 * MIB} MiB of fetches with maxSize 20m: {used} MiB")

    print("\nlocal copy of the binary caches, two 8 MiB paths unless noted:")
    print(f"{'step':<42} {'bytes from upstream':>20} {'seconds':>8}")
    for step, moved, t in results:
        print(f"{step:<42} {moved:>20} {t:>8.2f}")
  '';
}
