# nixos-test-vms config for the appliance's self-signed certificate
# (modules/tls.nix) and the binary cache wiring (modules/cache.nix).
#
# Two subjects in one VM because both are "did this setting actually reach the
# running system", and booting a second appliance to read a second config file
# is not worth the minute.
#
# The certificate matters for a reason beyond tidiness: WebAuthn refuses to run
# outside a secure context, so passkeys are impossible until :443 works. It also
# ends two limitations docs/security-model.md accepts today, the admin token and
# every Nextcloud login crossing the LAN in clear text.
#
# What is asserted here is the shape of the certificate rather than the front
# door's behaviour. Whether the admin surface stays LAN-only over TLS belongs
# with the rest of that guard in tests/front-vhost.nix.
{ pkgs }:

let
  lososPkgs = import ../flake/packages.nix { inherit pkgs; };
in

pkgs.testers.nixosTest {
  name = "losos-tls";

  nodes.machine =
    { ... }:
    {
      imports = [
        ../modules/options.nix
        ../modules/cache.nix
        ../modules/tls.nix
        ../modules/daemon.nix
      ];

      losos.hostName = "mattbox";
      losos.backend.package = lososPkgs.losos-ctl;

      # modules/tls.nix attaches its listener to the front vhost, which
      # modules/containers.nix owns. That module drags in the whole workload
      # stack, so the vhost is declared here instead and the TLS module layers
      # onto it exactly as it would there.
      services.nginx = {
        enable = true;
        virtualHosts."losos-front".locations."/".return = "200 'losos'";
      };

      # For the test script's own inspection of the certificate. The generator
      # unit gets openssl from its `path` in modules/tls.nix and does not need
      # this; only the assertions below do.
      environment.systemPackages = [ pkgs.openssl ];

      virtualisation = {
        memorySize = 1024;
        cores = 2;
      };
    };

  testScript = ''
    machine.start()
    machine.wait_for_unit("multi-user.target")
    machine.wait_for_unit("losos-tls-cert.service")
    machine.wait_for_unit("nginx.service")

    CERT = "/var/lib/losos-tls/cert.pem"
    KEY = "/var/lib/losos-tls/key.pem"

    with subtest("the key is readable by nginx and nobody else"):
        # 0640 root:nginx, not 0600 root. NixOS runs the whole nginx unit as the
        # nginx user rather than starting as root and dropping privileges, so a
        # root-only key is one nginx cannot load -- and it fails at the
        # `nginx -t` in ExecStartPre, before anything serves at all.
        assert machine.succeed(f"stat -c %a {KEY}").strip() == "640", \
            "the private key mode is wrong for a unit that runs as nginx"
        assert machine.succeed(f"stat -c %U:%G {KEY}").strip() == "root:nginx", \
            "the private key is not group-readable by nginx"
        assert machine.succeed(f"stat -c %a {CERT}").strip() == "644", \
            "the certificate is not readable; the wizard has to serve it"

        # Its own directory, not /var/lib/losos/tls. modules/daemon.nix owns
        # that one at 0700 root so state.json stays root-only, which makes
        # anything beneath it unreadable to nginx no matter its own mode.
        assert machine.succeed("stat -c %a /var/lib/losos-tls").strip() == "750"
        machine.succeed("stat -c %a /var/lib/losos | grep -qx 700")

    with subtest("it names the appliance in a SAN, not only a CN"):
        # This is the assertion worth having. Every browser released this decade
        # ignores the Common Name and matches subjectAltName, so a certificate
        # carrying only a CN fails to validate while looking entirely correct in
        # `openssl x509` output.
        text = machine.succeed(f"openssl x509 -in {CERT} -noout -text")
        assert "DNS:mattbox.local" in text, f"no SAN for the mDNS name:\n{text}"
        assert "DNS:localhost" in text, f"no SAN for localhost:\n{text}"
        assert "IP Address:127.0.0.1" in text, f"no SAN for loopback:\n{text}"

    with subtest("it is valid for two years, not ninety days"):
        # Renewal here is a person re-trusting the certificate on every device
        # they own. Nothing on this box can run certbot and nothing can prompt
        # them, so a short lifetime would mean an appliance that quietly stops
        # answering on :443.
        start = machine.succeed(f"date -d \"$(openssl x509 -in {CERT} -noout -startdate | cut -d= -f2)\" +%s")
        end = machine.succeed(f"date -d \"$(openssl x509 -in {CERT} -noout -enddate | cut -d= -f2)\" +%s")
        days = (int(end.strip()) - int(start.strip())) // 86400
        assert 725 <= days <= 735, f"certificate is valid {days} days, expected ~730"

    with subtest("HTTPS serves, and the certificate validates against itself"):
        machine.wait_for_open_port(443)
        # --cacert rather than -k: this proves the served certificate is the one
        # on disk, which is what the owner will trust. -k would pass against any
        # certificate at all, including a wrong one.
        out = machine.succeed(
            f"curl -sS --cacert {CERT} --resolve mattbox.local:443:127.0.0.1"
            " https://mattbox.local/"
        )
        assert "losos" in out, f"HTTPS did not serve the vhost: {out!r}"

    with subtest("plain HTTP still serves"):
        # :443 is useless until the owner has trusted the certificate, and the
        # page that hands it to them is served over :80. Closing :80 would make
        # the appliance unreachable for exactly the people who have not set it
        # up yet.
        machine.wait_for_open_port(80)
        assert "losos" in machine.succeed("curl -sS http://127.0.0.1/")

    with subtest("the certificate is stable across a restart"):
        # ConditionPathExists=! is what stops a new certificate being minted on
        # every boot. Without it the owner's devices would reject the appliance
        # after any reboot, with nothing saying why.
        before = machine.succeed(f"openssl x509 -in {CERT} -noout -fingerprint -sha256").strip()
        machine.succeed("systemctl restart losos-tls-cert.service")
        after = machine.succeed(f"openssl x509 -in {CERT} -noout -fingerprint -sha256").strip()
        assert before == after, "the certificate was regenerated; every trusted device breaks"

    with subtest("the binary cache reached the running nix"):
        # Eight files in this repo discussed "our own binary cache" and none
        # configured a substituter, so a fresh install built the 2.3 GiB
        # Nextcloud image on the target. Assert the setting is in force rather
        # than merely written down.
        conf = machine.succeed("nix --extra-experimental-features nix-command config show")
        assert "losos.cachix.org" in conf, f"substituter not in force:\n{conf}"
        assert "losos.cachix.org-1:" in conf, f"public key not trusted:\n{conf}"
        # cache.nixos.org still serves most of any closure here; ours is added,
        # not substituted for it.
        assert "cache.nixos.org" in conf, "the upstream cache was replaced rather than extended"
        # A cache that is unreachable must not wedge the 03:00 rebuild on a box
        # with no shell.
        assert "fallback = true" in conf, f"fallback is off:\n{conf}"
  '';
}
