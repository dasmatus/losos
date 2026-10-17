# The Stripe gate's production wiring, which the Rust suite cannot reach.
#
# edge/registrar's tests start the gate and the registrar as tasks in one
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
#   5. the gate settles a held card payment over its socket, from inside
#      its sandbox: it reads the charge back from Stripe, refunds the
#      buyer's share once and transfers the seller's, and refuses a
#      disputed charge or a refund after the seller was paid (a stub on
#      the edge plays Stripe);
#   6. the slot length and held-rails-only reach the registrar's command
#      line;
#   7. with a blob missing, the gate is skipped, the registrar stays up, and
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
  lososPkgs = import ../packages.nix { inherit pkgs; };

  # Fixture values. Shaped like the real thing because the gate shape-checks
  # both before it serves anything.
  stripeKey = "sk_test_vm_0123456789abcdefghijklmnop";
  webhookSecret = "whsec_vm_0123456789abcdefghijklmnopqrstuv";
  bootstrapTokenValue = "test-bootstrap-0123456789abcdef0123";

  keySealed = "/var/secrets/losos-stripe-secret-key.cred";
  webhookSealed = "/var/secrets/losos-stripe-webhook-secret.cred";
  bootstrapToken = "/var/secrets/losos-rathole-bootstrap";
  agentToken = "/var/secrets/losos-mesh-agent-token";

  # Plays Stripe for the settle subtest: two held market charges, one of
  # them disputed, and a log of every POST with its Idempotency-Key. It
  # answers each repeated key with the first result, as Stripe does.
  stripeStub = pkgs.writeText "stripe-stub.py" ''
    import json
    from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
    from urllib.parse import parse_qs, urlparse

    def charge(pi, disputed):
        return {
            "id": pi, "status": "succeeded", "amount_received": 2000,
            "currency": "eur",
            "metadata": {"order_id": "ord_" + pi[3:],
                         "losos_destination": "acct_seller1",
                         "losos_fee": "80"},
            "latest_charge": {"id": "ch_" + pi[3:], "amount_refunded": 0,
                              "disputed": disputed},
        }

    PAYMENTS = {"pi_split": charge("pi_split", False),
                "pi_disputed": charge("pi_disputed", True)}
    TRANSFERS, POSTS, SEEN = [], [], {}

    class Stub(BaseHTTPRequestHandler):
        def reply(self, body):
            data = json.dumps(body).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

        def do_GET(self):
            url = urlparse(self.path)
            q = parse_qs(url.query)
            if url.path == "/log":
                return self.reply({"posts": POSTS})
            if url.path == "/v1/transfers":
                group = q.get("transfer_group", [""])[0]
                return self.reply({"data": [t for t in TRANSFERS
                                            if t["transfer_group"] == group]})
            pi = url.path.rsplit("/", 1)[-1]
            self.reply(PAYMENTS[pi])

        def do_POST(self):
            n = int(self.headers.get("Content-Length", 0))
            form = {k: v[0] for k, v in parse_qs(self.rfile.read(n).decode()).items()}
            key = self.headers.get("Idempotency-Key")
            POSTS.append({"path": self.path, "key": key, "form": form})
            if key in SEEN:
                return self.reply(SEEN[key])
            if self.path == "/v1/refunds":
                c = PAYMENTS[form["payment_intent"]]["latest_charge"]
                c["amount_refunded"] += int(form["amount"])
                out = {"id": "re_" + str(len(POSTS))}
            else:
                TRANSFERS.append(form)
                out = {"id": "tr_" + str(len(POSTS))}
            SEEN[key] = out
            self.reply(out)

    ThreadingHTTPServer(("127.0.0.1", 12111), Stub).serve_forever()
  '';
in

pkgs.testers.nixosTest {
  name = "losos-edge-market";

  nodes.edge =
    { lib, ... }:
    {
      imports = [
        ../../modules/options.nix
        ../../modules/edge.nix
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
          stripeApi = "http://127.0.0.1:12111";
          offlineRefundAfter = 7200;
        };
      };

      # See the header: the mesh is required by the market but not exercised.
      services.rke2.enable = lib.mkForce false;
      systemd.services.losos-mesh-rbac.enable = false;

      systemd.services.stripe-stub = {
        wantedBy = [ "multi-user.target" ];
        before = [ "losos-stripe-gate.service" ];
        serviceConfig.ExecStart = "${pkgs.python3}/bin/python3 ${stripeStub}";
      };
      environment.systemPackages = [ pkgs.socat ];

      systemd.tmpfiles.rules = [
        "d /var/secrets 0700 root root - -"
        "f ${bootstrapToken} 0600 root root - ${bootstrapTokenValue}"
        "f ${agentToken} 0600 root root - test-agent-token"
      ];

      # How an operator provisions the secrets
      # (handbook/docs/in-depth/market.md), done at boot: sealed from stdin
      # under the names the gate's LoadCredentialEncrypted= expects, so no
      # plaintext file ever exists. There is no TPM in the VM, so systemd-creds
      # seals with the host key, which is the no-TPM edge's real path too.
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

    def gate(request):
        """One request over the gate's socket, as the registrar sends it."""
        line = json.dumps(request)
        out = edge.succeed(f"printf '%s\\n' '{line}' | socat -t 30 - UNIX-CONNECT:{SOCKET}")
        return json.loads(out)

    def posts():
        return json.loads(edge.succeed("curl -sf http://127.0.0.1:12111/log"))["posts"]

    with subtest("the gate settles a held card payment once, from inside its sandbox"):
        edge.wait_for_open_port(12111)
        # 1401 of 2000 back to the buyer; the seller gets the other 599 less
        # the fee scaled to it, 80 * 599 / 2000 rounded down = 23.
        reply = gate({"op": "settle", "payment_intent": "pi_split", "refund": 1401})
        assert reply.get("refunded") == 1401 and reply.get("transferred") == 576, reply
        sent = posts()
        assert [p["path"] for p in sent] == ["/v1/refunds", "/v1/transfers"], sent
        refund, transfer = sent
        assert refund["key"] == "losos-refund-pi_split-1401", refund
        assert refund["form"]["amount"] == "1401", refund
        assert transfer["key"] == "losos-transfer-pi_split", transfer
        assert transfer["form"]["amount"] == "576", transfer
        assert transfer["form"]["destination"] == "acct_seller1", transfer
        assert transfer["form"]["source_transaction"] == "ch_split", transfer
        assert transfer["form"]["transfer_group"] == "ord_split", transfer

    with subtest("a repeat sends nothing, and nothing goes back once the seller is paid"):
        reply = gate({"op": "settle", "payment_intent": "pi_split", "refund": 1401})
        assert reply.get("transferred") == 0 and "error" not in reply, reply
        reply = gate({"op": "settle", "payment_intent": "pi_split", "refund": 2000})
        assert reply["error"]["kind"] == "refused", reply
        assert "already been paid" in reply["error"]["message"], reply
        assert len(posts()) == 2, posts()

    with subtest("a disputed charge moves nowhere"):
        reply = gate({"op": "settle", "payment_intent": "pi_disputed", "refund": 0})
        assert reply["error"]["kind"] == "refused", reply
        assert "disputed" in reply["error"]["message"], reply
        assert len(posts()) == 2, posts()

    with subtest("the slot rules reach the registrar, and only held rails are offered"):
        cmd = edge.succeed(f"systemctl show -p ExecStart --value {REGISTRAR}")
        assert "--market-offline-refund-after 7200" in cmd, cmd
        assert "--market-direct-pay" not in cmd, cmd

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
