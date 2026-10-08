# The Stripe gate's production wiring, which the Rust suite cannot reach.
#
# backend-registrar's tests start the gate and the registrar as tasks in one
# process, with plaintext fixture files. What actually ships is two systemd
# units: `losos-stripe-gate` runs as a DynamicUser, decrypts two sealed blobs
# with LoadCredentialEncrypted=, and listens on a socket the registrar reaches
# as root; the registrar has those blobs and the gate's credential directory
# marked inaccessible; and a missing blob skips the gate by
# ConditionPathExists, which must leave the registrar serving. None of that is
# in the Rust code, so none of it is tested there. This file boots one edge
# with the market on and asserts each piece:
#
#   1. the blobs on disk are ciphertext, and the plaintext exists only in the
#      gate's credential tmpfs;
#   2. the gate runs as an unprivileged dynamic user, and its socket is 0600;
#   3. the registrar cannot read the blobs or the credential directory, even as
#      root;
#   4. a webhook signed with the sealed secret is accepted (so the gate
#      decrypted it and the registrar reached the gate through the socket),
#      and a forged one is refused;
#   5. with a blob missing, the gate is skipped, the registrar stays up, and
#      the market answers 503.
#
# The market requires the mesh (`losos.edge.market.enable -> cluster.enable`),
# because paid storage is fulfilled there. The mesh itself is not what this
# file is about, and rke2's control plane cannot start in a VM without its
# airgap images, so rke2 and the RBAC extractor are switched off here. The
# registrar only reaches the cluster when it fulfils a paid order, which this
# test never makes.
{ pkgs }:

let
  lososPkgs = import ../flake/packages.nix { inherit pkgs; };

  # Fixture values. Shaped like the real thing because the gate shape-checks
  # both before it serves anything.
  stripeKey = "sk_test_vm_0123456789abcdefghijklmnop";
  webhookSecret = "whsec_vm_0123456789abcdefghijklmnopqrstuv";
  bootstrapTokenValue = "test-bootstrap-0123456789abcdef0123";

  keySealed = "/var/secrets/losos-stripe-secret-key.cred";
  webhookSealed = "/var/secrets/losos-stripe-webhook-secret.cred";
  bootstrapToken = "/var/secrets/losos-rathole-bootstrap";
  agentToken = "/var/secrets/losos-mesh-agent-token";
in

pkgs.testers.nixosTest {
  name = "losos-edge-market";

  nodes.edge =
    { lib, ... }:
    {
      imports = [
        ../modules/options.nix
        ../modules/edge.nix
      ];

      _module.args.self = {
        packages.x86_64-linux.losos-registrar = lososPkgs.losos-registrar;
      };

      losos.edge = {
        enable = true;
        acmeEmail = "test@losos.cfd";
        bootstrapTokenFile = bootstrapToken;
        cluster = {
          enable = true;
          advertiseAddr = "192.0.2.1";
          agentTokenFile = agentToken;
        };
        market = {
          enable = true;
          returnUrl = "https://losos.cfd/market";
          stripeSecretKeySealed = keySealed;
          webhookSecretSealed = webhookSealed;
          hardware.enable = true;
        };
      };

      # See the header: the mesh is required by the market but not exercised.
      services.rke2.enable = lib.mkForce false;
      systemd.services.losos-mesh-rbac.enable = false;

      systemd.tmpfiles.rules = [
        "d /var/secrets 0700 root root - -"
        "f ${bootstrapToken} 0600 root root - ${bootstrapTokenValue}"
        "f ${agentToken} 0600 root root - test-agent-token"
      ];

      # How an operator provisions the secrets (wiki/Market.md), done at boot:
      # sealed from stdin under the names the gate's LoadCredentialEncrypted=
      # expects, so no plaintext file ever exists. There is no TPM in the VM,
      # so systemd-creds seals with the host key, which is the no-TPM edge's
      # real path too.
      systemd.services.seal-stripe-fixtures = {
        wantedBy = [ "multi-user.target" ];
        before = [ "losos-stripe-gate.service" ];
        requiredBy = [ "losos-stripe-gate.service" ];
        after = [ "systemd-tmpfiles-setup.service" ];
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
        };
        script = ''
          printf '%s' '${stripeKey}' \
            | systemd-creds encrypt --name=stripe-secret-key - ${keySealed}
          printf '%s\n' '${webhookSecret}' \
            | systemd-creds encrypt --name=stripe-webhook-secret - ${webhookSealed}
        '';
      };

      virtualisation = {
        memorySize = 1024;
        cores = 2;
      };
    };

  testScript = ''
    import hashlib
    import hmac
    import json

    API = "http://127.0.0.1:8443"
    GATE = "losos-stripe-gate.service"
    REGISTRAR = "losos-registrar.service"
    SOCKET = "/run/losos-stripe-gate/gate.sock"
    SECRET = "${webhookSecret}"

    def webhook(signature):
        """POST a harmless event and return the HTTP status as a string."""
        return edge.succeed(
            "curl -s -o /dev/null -w '%{http_code}' -X POST "
            f"-H 'Stripe-Signature: {signature}' "
            "-H 'Content-Type: application/json' "
            f"--data-binary @/tmp/event.json {API}/market/webhook"
        ).strip()

    def signed(body, secret):
        # The VM's clock, not the driver's: the registrar refuses a timestamp
        # more than five minutes off its own.
        t = edge.succeed("date +%s").strip()
        mac = hmac.new(secret.encode(), f"{t}.{body}".encode(), hashlib.sha256)
        return f"t={t},v1={mac.hexdigest()}"

    edge.start()
    edge.wait_for_unit("seal-stripe-fixtures.service")
    edge.wait_for_unit(GATE)
    edge.wait_for_unit(REGISTRAR)
    edge.wait_for_open_port(8443)
    edge.wait_for_file(SOCKET)

    with subtest("the secrets are ciphertext on disk and plaintext only in the gate's tmpfs"):
        edge.fail("grep -rqF '${stripeKey}' /var/secrets")
        edge.fail("grep -rqF '${webhookSecret}' /var/secrets")
        pid = edge.succeed(f"systemctl show -p MainPID --value {GATE}").strip()
        creds = f"/proc/{pid}/root/run/credentials/{GATE}"
        assert edge.succeed(f"cat {creds}/stripe-secret-key") == "${stripeKey}"

    with subtest("the gate runs as a dynamic user and its socket is 0600"):
        user = edge.succeed(f"ps -o user= -p {pid}").strip()
        assert user not in ("root", "0"), f"gate runs as {user}"
        mode = edge.succeed(f"stat -c '%a' {SOCKET}").strip()
        assert mode == "600", f"gate socket mode is {mode}"

    with subtest("the registrar cannot read the blobs or the gate's credentials"):
        # Seen through the registrar's own mount namespace. InaccessiblePaths
        # overmounts each path with an empty mode-0000 node, which root can
        # still open (CAP_DAC_OVERRIDE) but which holds nothing, so the
        # assertion is "empty there, non-empty here", not "open fails".
        reg = edge.succeed(f"systemctl show -p MainPID --value {REGISTRAR}").strip()
        root = f"/proc/{reg}/root"
        for blob in ["${keySealed}", "${webhookSealed}"]:
            edge.succeed(f"test -s {blob}")
            edge.succeed(f"test ! -s {root}{blob}")
        edge.succeed(f"test -n \"$(ls -A {creds})\"")
        edge.succeed(f"test -z \"$(ls -A {root}/run/credentials/{GATE} 2>/dev/null)\"")

    with subtest("a webhook signed with the sealed secret is accepted, a forged one is not"):
        body = json.dumps({"id": "evt_vm", "type": "ping", "data": {"object": {}}})
        edge.succeed(f"cat > /tmp/event.json <<'EOF'\n{body}\nEOF")
        body = edge.succeed("cat /tmp/event.json")
        status = webhook(signed(body, SECRET))
        assert status == "200", f"genuine webhook answered {status}"
        status = webhook(signed(body, "whsec_not_the_sealed_one_0123456789"))
        assert status == "400", f"forged webhook answered {status}"

    with subtest("both units load the hardware catalogue from the store"):
        catalogue = json.loads(edge.succeed(f"curl -sf {API}/market/hardware"))
        assert [i["sku"] for i in catalogue["items"]] == ["box", "gateway"], catalogue
        assert catalogue["currency"] == "eur", catalogue

    with subtest("without a blob the gate is skipped and only the market goes dark"):
        edge.succeed(f"systemctl stop {GATE}")
        edge.succeed("mv ${keySealed} /root/stripe-key.cred.away")
        edge.succeed(f"systemctl start {GATE}")
        result = edge.succeed(f"systemctl show -p ConditionResult --value {GATE}").strip()
        assert result == "no", f"gate ConditionResult is {result}"
        edge.fail(f"systemctl is-active {GATE}")
        edge.succeed(f"systemctl is-active {REGISTRAR}")
        edge.succeed(f"curl -sf {API}/health")
        status = webhook(signed(body, SECRET))
        assert status == "503", f"market without a gate answered {status}"
  '';
}
