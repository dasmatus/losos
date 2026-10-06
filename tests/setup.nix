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
# between source addresses on the same box, and curl --interface makes them
# without booting a second VM. The LAN client is a second address on the vlan
# (CLIENT below), not the box's own: the guard refuses a request whose source
# is the address it arrived on, because that is a local process posing as the
# LAN.
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
      # own in modules/setup.nix; neither needs this. certutil is what
      # trust.sh drives on Linux; here it stands in for the owner's laptop.
      environment.systemPackages = [
        pkgs.openssl
        pkgs.nssTools
      ];

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
    SH_URL = "/setup/trust.sh"
    PS1_URL = "/setup/trust.ps1"

    # This node's own address on vlan 1. Spelled out rather than reached through
    # the hostname, which NixOS also maps to 127.0.0.2 — and loopback is the one
    # thing these subtests exist to tell apart.
    LAN = "192.168.1.1"
    # A second address on the same vlan, standing in for another machine. The
    # box's own address will not do as a source: the guard refuses it.
    CLIENT = "192.168.1.50"
    appliance.succeed(f"ip addr add {CLIENT}/24 dev eth1")

    def lan(path, extra=""):
        """curl the route as a machine on the LAN would see it."""
        return f"curl -s --interface {CLIENT} {extra} http://{LAN}{path}"

    def code(url, source=None):
        src = f"--interface {source} " if source else ""
        return appliance.succeed(
            f"curl -s -o /dev/null -w '%{{http_code}}' {src}{url}"
        ).strip()

    def headers(path, extra=""):
        raw = appliance.succeed(lan(path, f"-D - -o /dev/null {extra}"))
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
        for path in (CERT_URL, STATE_URL, SH_URL, PS1_URL):
            got = code(f"http://127.0.0.1{path}")
            assert got == "403", f"loopback {path}: expected 403, got {got}"
            got = code(f"http://{LAN}{path}", source=CLIENT)
            assert got == "200", f"LAN {path}: expected 200, got {got}"
            # The copy of lanOnly in modules/setup.nix carries the same two
            # refusals as containers.nix: the box posing as the LAN, and the
            # mesh's pod network.
            got = code(f"http://{LAN}{path}", source=LAN)
            assert got == "403", f"box-as-LAN {path}: expected 403, got {got}"

    with subtest("a mesh pod address is refused on the setup routes"):
        appliance.succeed("ip addr add 10.42.0.7/32 dev eth1")
        for path in (CERT_URL, STATE_URL, SH_URL, PS1_URL):
            got = code(f"http://{LAN}{path}", source="10.42.0.7")
            assert got == "403", f"mesh-pod {path}: expected 403, got {got}"

    with subtest("state.json describes the certificate that is actually on disk"):
        doc = json.loads(appliance.succeed(lan(STATE_URL)))
        assert doc["hostName"] == "mattbox", doc
        assert doc["fqdn"] == "mattbox.local", doc
        assert doc["tls"] is True, doc

    with subtest("the document names the address the request reached the box on"):
        # The one-line installer in the wizard is built on this field, so it
        # has to be the box's LAN address as a LAN client sees it — not the
        # placeholder the unit wrote to disk, not loopback, not whatever
        # address DHCP had or had not handed out when the unit ran at boot.
        doc = json.loads(appliance.succeed(lan(STATE_URL)))
        assert doc["address"] == LAN, f"address is not the one the request arrived on: {doc}"
        on_disk = json.loads(appliance.succeed(f"cat {STATE}"))
        assert on_disk["address"] == "@ADDRESS@", \
            "the file must stay address-free: the unit runs before DHCP and the lease moves"
        # The substitution is a response-time property, so a second address
        # on the box yields a second answer from the same file.
        appliance.succeed("ip addr add 192.168.1.2/24 dev eth1")
        doc2 = json.loads(appliance.succeed(f"curl -s --interface {CLIENT} http://192.168.1.2{STATE_URL}"))
        assert doc2["address"] == "192.168.1.2", doc2

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
        for path in (CERT_URL, STATE_URL, SH_URL, PS1_URL):
            h = headers(path)
            assert "default-src 'none'" in h.get("content-security-policy", ""), \
                f"{path}: the inherited CSP was replaced by a location-level add_header: {h}"
            assert h.get("x-content-type-options") == "nosniff", f"{path}: {h}"
            assert h.get("x-frame-options") == "DENY", f"{path}: {h}"

    with subtest("the finder origin, and only it, may read state.json cross-origin"):
        # modules/options.nix `losos.setup.finderOrigins`: the "find my box"
        # page on the edge host reads this document from a public origin, under
        # Chrome's Local Network Access permission. The header must name that
        # origin exactly on this one route and be absent everywhere else — a
        # wildcard, or the header on the SPA or /nextcloud, would let any page
        # the owner has open take inventory of the LAN.
        FINDER = "https://losos-edge.dasmat.us"
        def cors(path, origin):
            return headers(path, f"-H 'Origin: {origin}'").get("access-control-allow-origin")
        assert cors(STATE_URL, FINDER) == FINDER, headers(STATE_URL, f"-H 'Origin: {FINDER}'")
        assert cors(STATE_URL, "https://losos-edge.dasmat.us.evil.example") is None
        assert cors(STATE_URL, "https://evil.example") is None
        assert cors(STATE_URL, "http://losos-edge.dasmat.us") is None, "scheme is part of the origin"
        assert cors(CERT_URL, FINDER) is None, "the certificate is not for other origins"
        assert cors("/nextcloud/", FINDER) is None, "the service route gets no CORS header"
        # And allowing the read did not cost the route its security headers.
        h = headers(STATE_URL, f"-H 'Origin: {FINDER}'")
        assert "default-src 'none'" in h.get("content-security-policy", ""), h

    with subtest("the install scripts carry this certificate, and are what the document advertises"):
        # The wizard's one-line command fetches whichever of the two the
        # document names; the script then installs the PEM it carries. So the
        # PEM inside each has to be the certificate on disk, byte for byte,
        # and the fingerprint printed for the owner to compare has to be the
        # one the wizard shows. A sed template that lost its marker line would
        # serve a script that installs the literal text "@PEM@".
        doc = json.loads(appliance.succeed(lan(STATE_URL)))
        install = doc["certificate"]["install"]
        assert install == {"sh": SH_URL, "ps1": PS1_URL}, install
        display = doc["certificate"]["fingerprintDisplay"]
        for path in (SH_URL, PS1_URL):
            local = "/tmp" + path.replace("/setup/", "/")
            appliance.succeed(lan(path, f"-o {local}"))
            h = headers(path)
            assert h.get("content-type", "").startswith("text/plain"), \
                f"{path}: a script must open as text in a browser tab (and as a string for irm): {h}"
            body = appliance.succeed(f"cat {local}")
            assert "@PEM@" not in body and "@FINGERPRINT@" not in body and "@HOST@" not in body, \
                f"{path}: a placeholder survived rendering"
            assert display in body, f"{path}: does not print the fingerprint the wizard shows"
            assert "mattbox" in body, f"{path}: does not name the box"
            appliance.succeed(
                f"sed -n '/-----BEGIN CERTIFICATE-----/,/-----END CERTIFICATE-----/p' {local} > {local}.pem"
                f" && cmp {local}.pem {CERT}"
            )
        appliance.succeed("sh -n /tmp/trust.sh")

    with subtest("trust.sh installs the certificate into a user's browser stores"):
        # As an ordinary user, with a home that has a Firefox profile in it:
        # the shape of the owner's laptop. Afterwards both NSS databases —
        # the one Chrome reads and the profile's — hold the certificate under
        # the box's name, trusted for TLS ("C"). Not root, so the system-store
        # branch is not taken: NixOS has none of those tools, and a script that
        # reached for sudo would fail the whole point.
        HOME = "/tmp/owner"
        PROFILE = f"{HOME}/.mozilla/firefox/abc123.default-release"
        appliance.succeed(
            f"mkdir -p {PROFILE}"
            f" && certutil -d sql:{PROFILE} -N --empty-password"
            f" && chmod -R 777 {HOME}"
            " && chmod 755 /tmp/trust.sh"
        )
        out = appliance.succeed(
            f"setpriv --reuid=nobody --regid=nogroup --clear-groups"
            f" env HOME={HOME} PATH=$PATH sh /tmp/trust.sh"
        )
        assert display in out, f"the script did not print the fingerprint to compare: {out}"
        for db in (f"{HOME}/.pki/nssdb", PROFILE):
            listing = appliance.succeed(f"certutil -d sql:{db} -L")
            assert "LosOS mattbox" in listing and "C,," in listing, f"{db}: {listing}"
        # Running it again replaces rather than stacks the entry.
        appliance.succeed(
            f"setpriv --reuid=nobody --regid=nogroup --clear-groups"
            f" env HOME={HOME} PATH=$PATH sh /tmp/trust.sh"
        )
        n = appliance.succeed(f"certutil -d sql:{HOME}/.pki/nssdb -L | grep -c 'LosOS mattbox'").strip()
        assert n == "1", f"a second run stacked a duplicate: {n} entries"
        # And a script whose certificate does not match its fingerprint
        # installs nothing. One stray character in the base64 is enough.
        appliance.succeed("sed '/^MII/s/.*/&x/' /tmp/trust.sh > /tmp/bad.sh")
        appliance.fail("env HOME=/tmp/other sh /tmp/bad.sh")
        appliance.fail("test -e /tmp/other/.pki/nssdb/cert9.db")

    with subtest("the file is readable by nginx and holds nothing secret"):
        assert appliance.succeed(f"stat -c %a {STATE}").strip() == "644", \
            "nginx has to read this, and there is nothing in it to hide"
        assert appliance.succeed("stat -c %a /var/lib/losos-setup").strip() == "755"
        for f in ("trust.sh", "trust.ps1"):
            assert appliance.succeed(f"stat -c %a /var/lib/losos-setup/{f}").strip() == "644", f
        # temp+rename, so a request can never catch a half-written document.
        appliance.succeed(f"test ! -e {STATE}.new")
        appliance.succeed("test ! -e /var/lib/losos-setup/trust.sh.new")

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
        before = appliance.succeed(f"cat {STATE} /var/lib/losos-setup/trust.sh /var/lib/losos-setup/trust.ps1")
        appliance.succeed("systemctl restart losos-setup-state.service")
        after = appliance.succeed(f"cat {STATE} /var/lib/losos-setup/trust.sh /var/lib/losos-setup/trust.ps1")
        assert before == after, "the published setup state is not stable across a restart"
  '';
}
