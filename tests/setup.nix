# nixos-test-vms config for the first-run setup routes (modules/setup.nix).
#
# Step 1 of the wizard is the owner trusting the appliance's self-signed
# certificate. That step is worth testing for real because every part of it
# fails silently:
#
#   * an nginx `alias` to a path modules/tls.nix later moves is a 404 in a
#     wizard, on a box with no shell to read a log from;
#   * a certificate served as text/plain is one the browser renders instead of
#     offering to install;
#   * a fingerprint computed from the wrong file, or mangled by a `tr` that
#     changed behaviour, is the one number the owner is asked to compare
#     against their browser — wrong is worse than absent;
#   * and a LAN guard is not a property of any expression. It only exists once
#     a request arrives carrying a source address, which is why this is a VM
#     test and not an eval assertion.
#
# One node, not two. tests/front-vhost.nix needs a second appliance because it
# is about what other machines see; here every interesting distinction is
# between two source addresses on the same box, and curl --interface makes both
# without booting a second VM (the same trick that file's "LAN source, same
# box" subtest uses).
#
# losos.admin.enable is off on purpose. The routes under test do not follow
# that flag — modules/setup.nix argues why — and leaving it off keeps the
# packaged admin SPA out of this test's closure, so a test about a certificate
# does not fail because of an unrelated UI build. What it costs is coverage of
# the admin locations, and tests/front-vhost.nix already owns those. What is
# still proved here is the merge: modules/containers.nix' own routes and its
# server-level security headers survive modules/setup.nix layering onto the
# same vhost.
{ pkgs }:

let
  # The Nextcloud pod's stand-in, on the loopback port modules/containers.nix
  # proxies /nextcloud to. A hostNetwork pod is nothing but a process listening
  # on the host's loopback, so an nginx server block is a faithful one, and it
  # is here only so the "the vhost still has containers.nix' routes" subtest
  # asserts a 200 rather than a 502.
  stubNextcloud =
    { config, ... }:
    {
      services.nginx.virtualHosts."stub-nextcloud" = {
        listen = [
          {
            addr = "127.0.0.1";
            port = config.losos.nextcloud.apachePort;
          }
        ];
        locations."/".extraConfig = ''return 200 "stub-nextcloud\n";'';
      };
    };
in

pkgs.testers.nixosTest {
  name = "losos-setup";

  nodes.appliance =
    { ... }:
    {
      imports = [
        ../modules/options.nix
        # The real front vhost, the real certificate generator, and the routes
        # under test — merged exactly as the `install` system merges them.
        ../modules/containers.nix
        ../modules/tls.nix
        ../modules/setup.nix
        stubNextcloud
      ];

      losos = {
        hostName = "mattbox";
        # See the file header: not a gate on the setup routes, and off here so
        # the admin UI package stays out of this closure.
        admin.enable = false;
        nextcloud.mode = "container";
        # Deliberately not the 11000 default, so a hard-coded port anywhere in
        # the proxy path fails this test instead of passing by coincidence.
        nextcloud.apachePort = 11007;
        # Nothing here requests /forgejo/, and off means no route proxying to a
        # port nothing listens on.
        forgejo.enable = false;
      };

      # For the script's own inspection of the certificate. The generator gets
      # openssl from its `path` in modules/tls.nix, and the publisher from its
      # own in modules/setup.nix; neither needs this.
      environment.systemPackages = [ pkgs.openssl ];

      virtualisation = {
        memorySize = 1024;
        cores = 2;
        # Spelled out rather than left to the framework's default: the whole
        # test turns on this node having a LAN address to send from.
        vlans = [ 1 ];
      };
    };

  testScript = ''
    import json
    from datetime import datetime, timezone

    appliance.start()
    appliance.wait_for_unit("multi-user.target")
    appliance.wait_for_unit("losos-tls-cert.service")
    appliance.wait_for_unit("losos-setup-state.service")
    appliance.wait_for_unit("nginx.service")
    appliance.wait_for_open_port(80)

    CERT = "/var/lib/losos-tls/cert.pem"
    STATE = "/var/lib/losos-setup/state.json"
    CERT_URL = "/setup/losos-ca.crt"
    STATE_URL = "/setup/state.json"

    # This node's own address on vlan 1. Spelled out rather than reached through
    # the hostname, which NixOS also maps to 127.0.0.2 — and loopback is the one
    # thing these subtests exist to tell apart.
    LAN = "192.168.1.1"

    def lan(path, extra=""):
        """curl the route as a machine on the LAN would see it."""
        return f"curl -s --interface {LAN} {extra} http://{LAN}{path}"

    def code(url, source=None):
        src = f"--interface {source} " if source else ""
        return appliance.succeed(
            f"curl -s -o /dev/null -w '%{{http_code}}' {src}{url}"
        ).strip()

    def headers(path):
        raw = appliance.succeed(lan(path, "-D - -o /dev/null"))
        out = {}
        for line in raw.splitlines()[1:]:
            if ":" in line:
                k, _, v = line.partition(":")
                out[k.strip().lower()] = v.strip()
        return out

    with subtest("the certificate downloads, byte for byte"):
        # cmp, not a fingerprint comparison: what the owner installs has to be
        # this file and not a re-encoding of it, and an `alias` pointed at a
        # stale path would still serve a perfectly valid certificate for the
        # wrong key.
        appliance.succeed(lan(CERT_URL, "-o /tmp/downloaded.crt"))
        appliance.succeed(f"cmp /tmp/downloaded.crt {CERT}")
        appliance.succeed("openssl x509 -in /tmp/downloaded.crt -noout -subject")

    with subtest("it is typed so a browser does something useful with it"):
        h = headers(CERT_URL)
        assert h.get("content-type") == "application/x-x509-ca-cert", \
            f"wrong content type; a certificate served as text is one the browser renders: {h}"
        # The filename the browser saves comes from the last path segment,
        # because there is deliberately no Content-Disposition — see
        # modules/setup.nix. So the URL has to keep ending in something a
        # person can recognise in their Downloads folder.
        assert CERT_URL.endswith("/losos-ca.crt"), CERT_URL
        assert "content-disposition" not in h, \
            "a Content-Disposition here drops the inherited security headers; " \
            "NixOS' gixy check fails the build over it"

    with subtest("the setup routes are LAN-only, like the rest of the admin surface"):
        # Not because a certificate is secret — it is not — but because
        # master-proxy tunnel traffic reaches this vhost from 127.0.0.1, so a
        # route that answers loopback is a route on the public internet the
        # moment losos.proxy.enable is on. A version of this subtest expecting
        # 200 here is testing that bug.
        for path in (CERT_URL, STATE_URL):
            got = code(f"http://127.0.0.1{path}")
            assert got == "403", f"loopback {path}: expected 403, got {got}"
            got = code(f"http://{LAN}{path}", source=LAN)
            assert got == "200", f"LAN {path}: expected 200, got {got}"

    with subtest("state.json describes the certificate that is actually on disk"):
        doc = json.loads(appliance.succeed(lan(STATE_URL)))
        assert doc["hostName"] == "mattbox", doc
        assert doc["fqdn"] == "mattbox.local", doc
        assert doc["tls"] is True, doc

        display = appliance.succeed(
            f"openssl x509 -in {CERT} -noout -fingerprint -sha256"
        ).strip().split("=", 1)[1]
        cert = doc["certificate"]
        # The machine spelling, matching what lososd reports from GET
        # /api/setup, and the spelling a browser's certificate dialog prints.
        # Both, because asking a dependency-free page to reformat 32 bytes is
        # how two spellings of one number start to disagree.
        assert cert["fingerprint"] == "sha256:" + display.replace(":", "").lower(), \
            f"fingerprint is not this certificate's: {cert} vs {display}"
        assert cert["fingerprintDisplay"] == display, \
            f"display fingerprint is not what a browser shows: {cert} vs {display}"

        # Faithful conversion, not a re-derivation of "two years" —
        # tests/tls.nix owns the lifetime.
        end = int(appliance.succeed(
            f"date -d \"$(openssl x509 -in {CERT} -noout -enddate | cut -d= -f2)\" +%s"
        ).strip())
        got = int(datetime.strptime(cert["expires"], "%Y-%m-%dT%H:%M:%SZ")
                  .replace(tzinfo=timezone.utc).timestamp())
        assert got == end, f"expiry {cert['expires']} is not the certificate's notAfter"

    with subtest("the URL the document advertises is the one that serves"):
        # The document is what the wizard follows, so a dead link in it is the
        # same outage as a missing route — and this is the assertion that
        # catches one of the two being renamed without the other.
        doc = json.loads(appliance.succeed(lan(STATE_URL)))
        appliance.succeed(lan(doc["certificate"]["url"], "-o /tmp/advertised.crt"))
        appliance.succeed(f"cmp /tmp/advertised.crt {CERT}")

    with subtest("both routes keep the vhost's inherited security headers"):
        # Neither location adds a header of its own, so the four
        # modules/containers.nix sets at server level still ride on both. This
        # is the property that goes red the day someone adds a Cache-Control or
        # a Content-Disposition here: `add_header` at location scope replaces
        # the inherited set rather than extending it, and it takes the CSP with
        # it. NixOS' build-time gixy check catches that earlier and more
        # loudly; this is the confirmation that what it checked is what the
        # running server does.
        assert headers(STATE_URL).get("content-type") == "application/json", headers(STATE_URL)
        for path in (CERT_URL, STATE_URL):
            h = headers(path)
            assert "default-src 'none'" in h.get("content-security-policy", ""), \
                f"{path}: the inherited CSP was replaced by a location-level add_header: {h}"
            assert h.get("x-content-type-options") == "nosniff", f"{path}: {h}"
            assert h.get("x-frame-options") == "DENY", f"{path}: {h}"

    with subtest("the file is readable by nginx and holds nothing secret"):
        assert appliance.succeed(f"stat -c %a {STATE}").strip() == "644", \
            "nginx has to read this, and there is nothing in it to hide"
        assert appliance.succeed("stat -c %a /var/lib/losos-setup").strip() == "755"
        # temp+rename, so a request can never catch a half-written document.
        appliance.succeed(f"test ! -e {STATE}.new")

    with subtest("the vhost containers.nix owns is still intact"):
        # modules/setup.nix merges into the same attribute path rather than
        # declaring a vhost of its own; this is the assertion that the merge did
        # not displace what was already there.
        got = code("http://127.0.0.1/nextcloud")
        assert got == "200", f"/nextcloud went missing: {got}"
        assert "stub-nextcloud" in appliance.succeed("curl -s http://127.0.0.1/nextcloud")

    with subtest("republishing changes nothing"):
        # The unit runs on every boot. It must be a function of the certificate
        # on disk and nothing else — a fingerprint that moves is one the owner
        # already wrote down and can no longer match.
        before = appliance.succeed(f"cat {STATE}")
        appliance.succeed("systemctl restart losos-setup-state.service")
        after = appliance.succeed(f"cat {STATE}")
        assert before == after, "the published setup state is not stable across a restart"
  '';
}
