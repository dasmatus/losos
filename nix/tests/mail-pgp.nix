# nixos-test-vms config for OpenPGP in LosOS Mail: keys made in the browser,
# kept locked and sealed on the box, published in the box's Web Key
# Directory, mail encrypted and signed in one browser and opened in another,
# and keys for people elsewhere found by WKD, a keyserver and Autocrypt.
#
# The box runs the same pieces as nix/tests/mail.nix (Stalwart, lososd,
# LosOS Login, the front vhost) with the real admin pages. The laptop is
# Matus and Eva, each in their own Chromium profile (nix/tests/mail-pgp.mjs),
# and GnuPG for what a mail program elsewhere would do. `elsewhere` is the
# rest of the internet: the mail domain example.org with a WKD, and
# keys.example.net standing in for keys.openpgp.org, on certificates from a
# CA the box trusts.
#
#   * a key made in the page leaves it locked: the box keeps it sealed with
#     systemd-creds and never holds it unlocked, and refuses one that is;
#   * the box publishes the public key under both WKD paths, to anyone,
#     loopback (the tunnel) included, GET and HEAD only, and takes it out
#     again when the key is removed;
#   * Matus writes to Eva in the browser: her key is found on the box, the
#     message is encrypted and signed, and what Stalwart stores is
#     multipart/encrypted with no word of the text in it, carrying Matus's
#     Autocrypt header;
#   * Eva opens it in her browser after her passphrase and sees it signed
#     by Matus; Matus reads his own copy in Sent;
#   * a PGP/MIME signed message made with GnuPG goes through lososd and
#     Stalwart byte for byte, and GnuPG verifies what arrives;
#   * friend@example.org's key comes from their WKD, stranger@... from the
#     keyserver, and a key someone sent in an Autocrypt header is learned
#     once Matus opens their mail.
#
# The run keeps the screenshots and the film's stills under mail-pgp-shots/.
# With `beforeUi` (the admin pages from before this, built by the caller)
# the specialisation `before` serves them on the same box and mail, for the
# pull request's "before" pictures; the flake's check leaves it out.
{
  pkgs,
  beforeUi ? null,
}:

let
  lososPkgs = import ../packages.nix { inherit pkgs; };
  withoutLab = import ./admin-ui-without-lab.nix;

  # The outside world's certificates and keys, made once at build time.
  fixture =
    pkgs.runCommand "mail-pgp-fixture"
      {
        nativeBuildInputs = [
          pkgs.openssl
          pkgs.gnupg
        ];
      }
      ''
        mkdir -p $out
        cd $out
        openssl req -x509 -newkey rsa:2048 -nodes -days 3650 -subj "/CN=Test Internet CA" \
          -keyout ca.key -out ca.pem
        openssl req -newkey rsa:2048 -nodes -subj "/CN=example.org" -keyout server.key -out server.csr \
          -addext "subjectAltName=DNS:example.org,DNS:openpgpkey.example.org,DNS:keys.example.net"
        printf 'subjectAltName=DNS:example.org,DNS:openpgpkey.example.org,DNS:keys.example.net\n' > ext
        openssl x509 -req -in server.csr -CA ca.pem -CAkey ca.key -CAcreateserial -days 3650 \
          -extfile ext -out server.pem

        export GNUPGHOME=$(mktemp -d)
        for who in friend@example.org stranger@elsewhere.example post@example.com; do
          gpg --batch --pinentry-mode loopback --passphrase "" --quick-gen-key "$who" ed25519 sign never
          fpr=$(gpg --batch --with-colons --list-keys "$who" | awk -F: '/^fpr/ { print $10; exit }')
          gpg --batch --pinentry-mode loopback --passphrase "" --quick-add-key "$fpr" cv25519 encr never
          echo "$fpr" > "fpr-$who"
        done
        # example.org's WKD, the advanced method's layout.
        hash=$(gpg --batch --with-wkd-hash --list-keys friend@example.org | grep -o '[a-z0-9]\{32\}@example.org' | cut -d@ -f1)
        mkdir -p wkd/.well-known/openpgpkey/example.org/hu
        touch wkd/.well-known/openpgpkey/example.org/policy
        gpg --batch --export friend@example.org > "wkd/.well-known/openpgpkey/example.org/hu/$hash"
        # The keyserver's answer for stranger@elsewhere.example.
        mkdir -p vks
        gpg --batch --armor --export stranger@elsewhere.example > vks/stranger
        # Autocrypt's keydata for post@example.com: the key, base64, one line.
        gpg --batch --export post@example.com | base64 -w0 > autocrypt-post
        # Their private keys too, so the laptop can sign as them.
        gpg --batch --armor --pinentry-mode loopback --passphrase "" --export-secret-keys post@example.com > secret-post.asc
        rm -rf "$GNUPGHOME" server.csr ext ca.srl
      '';

  shots = pkgs.writeShellScriptBin "mail-pgp" ''
    export PLAYWRIGHT_CORE=${pkgs.playwright-driver}
    exec ${pkgs.bun}/bin/bun ${./mail-pgp.mjs} "$@"
  '';

  # The test numbers its machines by name: box .1, elsewhere .2, laptop .3.
  outsideHosts = "192.168.1.2 example.org openpgpkey.example.org keys.example.net";
in

pkgs.testers.nixosTest {
  name = "losos-mail-pgp";

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
          ../../modules/mail.nix
        ];
        losos = {
          hostName = "mattbox";
          backend.package = lososPkgs.losos-ctl;
          # The mail plugin answers the folder and filter routes.
          backend.plugins = lososPkgs.losos-plugins;
          admin.ui = withoutLab lososPkgs.losos-admin-ui;
          git.enable = false;
          sharingMyStorage = true;
          cluster.enable = true;
          shared.fscrypt.enable = false;
          mail.keyserver = "https://keys.example.net";
        };
        specialisation = lib.optionalAttrs (beforeUi != null) {
          before.configuration.losos.admin.ui = lib.mkForce (withoutLab beforeUi);
        };
        users.groups.shared.gid = 1001;
        users.users.shared = {
          uid = 1001;
          isNormalUser = true;
          group = "shared";
          homeMode = "700";
        };
        systemd.tmpfiles.rules = [ "d /home/shared/data 0700 shared shared -" ];
        services.authelia.instances.losos.settings.server.endpoints.rate_limits.session_elevation_start.enable =
          false;
        security.pki.certificateFiles = [ "${fixture}/ca.pem" ];
        networking.extraHosts = outsideHosts;
        environment.systemPackages = [
          pkgs.jq
          pkgs.curl
        ];
        virtualisation = {
          memorySize = 2048;
          cores = 2;
          vlans = [ 1 ];
        };
      };

    laptop = _: {
      environment.systemPackages = [
        pkgs.curl
        pkgs.jq
        pkgs.gnupg
        shots
      ];
      networking.extraHosts = "192.168.1.1 mattbox.local mattbox.home.arpa\n${outsideHosts}";
      environment.variables = {
        PLAYWRIGHT_BROWSERS_PATH = "${pkgs.playwright-driver.browsers.override {
          withFirefox = false;
          withWebkit = false;
        }}";
        PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS = "true";
      };
      fonts = {
        packages = [
          pkgs.roboto
          pkgs.dejavu_fonts
        ];
        fontconfig.defaultFonts.sansSerif = [ "Roboto" ];
      };
      virtualisation = {
        memorySize = 3072;
        cores = 2;
        vlans = [ 1 ];
      };
    };

    elsewhere = {
      networking.firewall.allowedTCPPorts = [ 443 ];
      services.nginx = {
        enable = true;
        virtualHosts =
          let
            tls = {
              onlySSL = true;
              sslCertificate = "${fixture}/server.pem";
              sslCertificateKey = "${fixture}/server.key";
            };
          in
          {
            "openpgpkey.example.org" = tls // {
              root = "${fixture}/wkd";
              extraConfig = "default_type application/octet-stream;";
            };
            # The direct method on example.org itself has nothing, so the
            # advanced method is what answers.
            "example.org" = tls // {
              locations."/".return = "404";
            };
            "keys.example.net" = tls // {
              locations."= /vks/v1/by-email/stranger@elsewhere.example".alias = "${fixture}/vks/stranger";
              locations."/".return = "404";
            };
          };
      };
      virtualisation.vlans = [ 1 ];
    };
  };

  testScript = ''
    import base64
    import hashlib
    import json
    import re
    import time

    BOX = "192.168.1.1"
    PASSWORD = "Correct-Horse-Battery-9"
    EVA_PASSWORD = "Eva-Long-Password-3"
    MATUS_PASS = "salmon river at dawn"
    EVA_PASS = "blue heron by the lake"
    DOMAIN = "mattbox.home.arpa"
    SECRET = "The spare key is under the blue pot by the door."
    BEFORE = ${if beforeUi != null then "True" else "False"}
    FIXTURE = "${fixture}"

    # Three VMs under TCG can miss the driver's wait for a shell when they
    # boot together.
    for m in (elsewhere, box, laptop):
        m.start()
        m.succeed("true")
    box.wait_for_unit("lososd.service")
    box.wait_for_unit("nginx.service")
    box.wait_for_unit("losos-mail.service")
    box.wait_for_open_port(8096)
    box.wait_for_open_port(443)
    elsewhere.wait_for_unit("nginx.service")
    elsewhere.wait_for_open_port(443)

    def curl(args):
        return laptop.succeed(f"curl -sk {args}")

    def api(path, cookie=None, method="GET", data=None, extra=""):
        args = f"-X {method} 'https://{BOX}{path}' {extra}"
        if cookie:
            args += f" -H 'Cookie: {cookie}'"
        if data is not None:
            body = json.dumps(data).replace("'", "'\\'''")
            args += f" -H 'Content-Type: application/json' -d '{body}'"
        out = curl(f"-w '\\n%{{http_code}}' {args}")
        body, code = out.rsplit("\n", 1)
        return int(code), body

    def ok(path, cookie, **kw):
        code, body = api(path, cookie, **kw)
        assert code == 200, (path, code, body)
        return json.loads(body)

    def sign_in(name, password):
        body = json.dumps({"username": name, "password": password})
        head = curl(
            f"-D - -o /dev/null https://{BOX}/auth/api/firstfactor "
            f"-H 'Content-Type: application/json' -d '{body}'"
        )
        m = re.search(r"(?im)^set-cookie: (losos_session=[^;]+)", head)
        assert m, f"no session cookie for {name}: {head}"
        return m.group(1)

    def folder(home, role):
        return next(m["id"] for m in home["mailboxes"] if m["role"] == role)

    def wait_for(cookie, mailbox, subject, timeout=120):
        deadline = time.time() + timeout
        while time.time() < deadline:
            page = ok(f"/api/mail/messages?mailbox={mailbox}", cookie)
            found = [m for m in page["messages"] if m["subject"] == subject]
            if found:
                return found[0]
            time.sleep(2)
        raise AssertionError(f"{subject!r} never arrived in {mailbox}: {page}")

    def raw(cookie, mid):
        """The stored message, byte for byte (through base64, so no line
        ending is touched on the way to the driver)."""
        laptop.succeed(f"curl -sfk -o /tmp/raw.eml -H 'Cookie: {cookie}' https://{BOX}/api/mail/messages/{mid}/raw")
        return base64.b64decode(laptop.succeed("base64 -w0 /tmp/raw.eml")).decode()

    def put(machine, path, data):
        """Write `data` to `path` exactly, CRs and all."""
        if isinstance(data, str):
            data = data.encode()
        machine.succeed(f"echo {base64.b64encode(data).decode()} | base64 -d > {path}")

    def match(pattern, text, group=0, flags=0):
        """The group of the first match, which must be there."""
        m = re.search(pattern, text, flags)
        assert m is not None, f"{pattern!r} not in {text[:2000]!r}"
        return m.group(group)

    def wkd_hash(local):
        # z-base-32 of SHA-1 of the lower-cased local part.
        alphabet = "ybndrfg8ejkmcpqxot1uwisza345h769"
        bits = "".join(f"{b:08b}" for b in hashlib.sha1(local.lower().encode()).digest())
        return "".join(alphabet[int(bits[i:i + 5], 2)] for i in range(0, 160, 5))

    def screens(*args, timeout=3600):
        print(laptop.succeed("mail-pgp " + " ".join("'" + a + "'" for a in args), timeout=timeout))

    def box_settings():
        body = "{ ... }:\n{\n  losos.sharingMyStorage = true;\n  losos.cluster.enable = true;\n}"
        box.succeed(
            "mkdir -p /etc/nixos/modules"
            f" && cat > /etc/nixos/modules/overrides.nix <<'EOF'\n{body}\nEOF\n"
        )

    box_settings()

    with subtest("the owner claims the box and adds Eva"):
        laptop.wait_until_succeeds(f"curl -sfk https://{BOX}/api/setup/claim | jq -e .ready", timeout=300)
        code, body = api(
            "/api/setup/claim",
            method="POST",
            data={"name": "matus", "password": PASSWORD},
            extra=f"-H 'Host: {BOX}'",
        )
        assert code == 200, (code, body)
        box.wait_for_unit("authelia-losos.service")
        box.wait_for_open_port(9091)
        box.systemctl("start losos-identity-setup.service")
        box.wait_until_succeeds(f"grep -q \"domain: '{BOX}'\" /run/losos-identity/session.yml")
        box.wait_until_succeeds("systemctl is-active authelia-losos.service")
        box.wait_for_open_port(9091)
        matus = sign_in("matus", PASSWORD)
        code, body = api(
            "/api/people",
            matus,
            method="POST",
            data={"name": "eva", "displayName": "Eva", "password": EVA_PASSWORD, "groups": []},
        )
        assert code == 200, (code, body)
        laptop.wait_until_succeeds(
            f"curl -sk https://{BOX}/auth/api/firstfactor -H 'Content-Type: application/json'"
            f" -d '{json.dumps({"username": "eva", "password": EVA_PASSWORD})}' | grep -q OK",
            timeout=60,
        )
        eva = sign_in("eva", EVA_PASSWORD)
        mhome = ok("/api/mail", matus)
        ehome = ok("/api/mail", eva)
        assert ok("/api/mail/keys", matus) == {"key": None}

    with subtest("the box refuses a private key that is not locked"):
        laptop.succeed(
            "export GNUPGHOME=$(mktemp -d) && "
            f"gpg --batch --pinentry-mode loopback --passphrase ''' --quick-gen-key matus@{DOMAIN} ed25519 sign never && "
            "fpr=$(gpg --batch --with-colons --list-keys | awk -F: '/^fpr/ { print $10; exit }') && "
            "gpg --batch --pinentry-mode loopback --passphrase ''' --quick-add-key $fpr cv25519 encr never && "
            "gpg --batch --armor --export > /tmp/open-public.asc && "
            "gpg --batch --armor --pinentry-mode loopback --passphrase ''' --export-secret-keys > /tmp/open-secret.asc"
        )
        public = laptop.succeed("cat /tmp/open-public.asc")
        secret = laptop.succeed("cat /tmp/open-secret.asc")
        code, body = api("/api/mail/keys", matus, method="PUT", data={"publicKey": public, "lockedKey": secret})
        assert code == 400 and "passphrase" in body, (code, body)
        box.fail("test -e /var/lib/losos/mail-keys/matus")

    with subtest("Matus and Eva make their keys in the browser"):
        screens("key", "matus", PASSWORD, MATUS_PASS, "01")
        screens("key", "eva", EVA_PASSWORD, EVA_PASS, "-")
        mkey = ok("/api/mail/keys", matus)["key"]
        ekey = ok("/api/mail/keys", eva)["key"]
        assert len(mkey["userIds"]) == 1 and mkey["userIds"][0].endswith(f"<matus@{DOMAIN}>"), mkey["userIds"]
        assert mkey["sealed"] in ("tpm2", "host"), mkey
        # Eva cannot read Matus's key; each sees their own.
        assert ekey["fingerprint"] != mkey["fingerprint"]

    with subtest("the private key is kept sealed, and locked inside the seal"):
        box.succeed("test -e /var/lib/losos/mail-keys/matus/locked.cred")
        mode = box.succeed("stat -c '%U %a' /var/lib/losos/mail-keys/matus/locked.cred").strip()
        assert mode == "root 600", mode
        box.fail("grep -rl 'PRIVATE KEY BLOCK' /var/lib/losos/mail-keys")
        box.fail("test -e /var/lib/losos/mail-keys/matus/locked.asc")
        opened = box.succeed(
            "systemd-creds decrypt --name=mail-key-matus /var/lib/losos/mail-keys/matus/locked.cred - "
            "|| systemd-creds decrypt /var/lib/losos/mail-keys/matus/locked.cred -"
        )
        assert "BEGIN PGP PRIVATE KEY BLOCK" in opened
        put(laptop, "/tmp/matus-locked.asc", opened)
        packets = laptop.succeed("GNUPGHOME=$(mktemp -d) gpg --batch --list-packets /tmp/matus-locked.asc")
        # Both the signing key and the encryption subkey are under S2K.
        assert packets.count("S2K") >= 2 and "protected" in packets, packets
        assert opened.strip() == mkey["lockedKey"].strip()

    with subtest("the box publishes both public keys in its Web Key Directory"):
        for local, key in (("matus", mkey), ("eva", ekey)):
            h = wkd_hash(local)
            for path in (f"/.well-known/openpgpkey/hu/{h}", f"/.well-known/openpgpkey/{DOMAIN}/hu/{h}"):
                laptop.succeed(f"curl -sfk -o /tmp/wkd.bin 'https://mattbox.home.arpa{path}?l={local}'")
                shown = laptop.succeed("GNUPGHOME=$(mktemp -d) gpg --batch --with-colons --show-keys /tmp/wkd.bin")
                assert f"fpr:::::::::{key['fingerprint']}:" in shown, shown
                assert "sec:" not in shown and "ssb:" not in shown, shown
            head = curl(f"-D - -o /dev/null https://{BOX}/.well-known/openpgpkey/hu/{h}")
            assert re.search(r"(?im)^access-control-allow-origin: \*", head), head
        laptop.succeed(f"curl -sfk https://{BOX}/.well-known/openpgpkey/policy")
        laptop.succeed(f"curl -sfk https://{BOX}/.well-known/openpgpkey/{DOMAIN}/policy")
        # Public: through the tunnel (loopback) too, unlike the admin pages.
        box.succeed(f"curl -sfk -o /dev/null https://127.0.0.1/.well-known/openpgpkey/hu/{wkd_hash('matus')}")
        box.fail("curl -sfk -o /dev/null https://127.0.0.1/api/mail")
        code = curl(f"-o /dev/null -w '%{{http_code}}' -X POST https://{BOX}/.well-known/openpgpkey/hu/{wkd_hash('matus')}")
        assert code == "403", code
        code = curl(f"-o /dev/null -w '%{{http_code}}' https://{BOX}/.well-known/openpgpkey/hu/{wkd_hash('nobody')}")
        assert code == "404", code
        # The lookup for someone on the box answers from the box.
        found = ok(f"/api/mail/keys/find?email=eva@{DOMAIN}", matus)["keys"]
        assert [(k["source"], k["fingerprint"]) for k in found] == [("box", ekey["fingerprint"])], found

    with subtest("keys for people elsewhere: WKD, then the keyserver"):
        fpr = lambda who: box.succeed(f"cat {FIXTURE}/fpr-{who}").strip()
        found = ok("/api/mail/keys/find?email=friend@example.org", matus)["keys"]
        assert [(k["source"], k["fingerprint"]) for k in found] == [("wkd", fpr("friend@example.org"))], found
        found = ok("/api/mail/keys/find?email=Stranger@Elsewhere.example", matus)["keys"]
        assert [(k["source"], k["fingerprint"]) for k in found] == [("keyserver", fpr("stranger@elsewhere.example"))], found
        assert ok("/api/mail/keys/find?email=nobody@example.org", matus)["keys"] == []
        code, _ = api("/api/mail/keys/find?email=not-an-address", matus)
        assert code == 400, code
        code, _ = api(f"/api/mail/keys/find?email=eva@{DOMAIN}")
        assert code == 401, code
        screens("outside", PASSWORD, "05")

    with subtest("Matus writes to Eva in the browser: encrypted and signed"):
        screens("send", PASSWORD, MATUS_PASS, "06")
        got = wait_for(eva, folder(ehome, "inbox"), "The spare key")
        assert got["pgp"] == "encrypted", got
        message = raw(eva, got["id"])
        assert re.search(r"(?i)content-type: multipart/encrypted;\s*protocol=\"application/pgp-encrypted\"", message), message[:600]
        assert "-----BEGIN PGP MESSAGE-----" in message
        for word in ("spare", "blue pot", "door"):
            assert word not in re.split(r"\r?\n\r?\n", message, maxsplit=1)[1], word
        # His Autocrypt header carries his public key.
        m = re.search(r"(?ims)^autocrypt:(.*?)\r?\n(?=\S)", message)
        assert m, message[:800]
        value = re.sub(r"\s+", " ", m.group(1))
        assert f"addr=matus@{DOMAIN}" in value, value
        keydata = base64.b64decode(match(r"keydata=([A-Za-z0-9+/= ]+)", value, 1).replace(" ", ""))
        put(laptop, "/tmp/autocrypt.bin", keydata)
        shown = laptop.succeed("GNUPGHOME=$(mktemp -d) gpg --batch --with-colons --show-keys /tmp/autocrypt.bin")
        assert f"fpr:::::::::{mkey['fingerprint']}:" in shown, shown
        # And the copy in Sent is the same encrypted message.
        mine = wait_for(matus, folder(mhome, "sent"), "The spare key")
        assert mine["pgp"] == "encrypted", mine
        # GnuPG with Eva's key opens what Stalwart stored, signed by Matus.
        put(laptop, "/tmp/eva-locked.asc", ekey["lockedKey"])
        put(laptop, "/tmp/matus-public.asc", mkey["publicKey"])
        armored = match(r"-----BEGIN PGP MESSAGE-----.*?-----END PGP MESSAGE-----", message, flags=re.S)
        put(laptop, "/tmp/stored.asc", armored)
        out = laptop.succeed(
            "export GNUPGHOME=$(mktemp -d) && gpg --batch --import /tmp/matus-public.asc && "
            f"gpg --batch --pinentry-mode loopback --passphrase '{EVA_PASS}' --import /tmp/eva-locked.asc && "
            f"gpg --batch --pinentry-mode loopback --passphrase '{EVA_PASS}' --status-fd 1 --decrypt /tmp/stored.asc"
        )
        assert SECRET in out, out
        assert f"VALIDSIG {mkey['fingerprint']}" in out, out

    with subtest("Eva opens it in her browser, and Matus reads his copy"):
        screens("read", EVA_PASSWORD, EVA_PASS, "08")
        screens("sent", PASSWORD, MATUS_PASS)

    if BEFORE:
        with subtest("the same screens on the pages from before OpenPGP"):
            box.succeed("/run/current-system/specialisation/before/bin/switch-to-configuration test")
            box.wait_for_unit("nginx.service")
            box.wait_for_unit("lososd.service")
            screens("before", EVA_PASSWORD, PASSWORD)
            box.succeed("/run/booted-system/bin/switch-to-configuration test")
            box.wait_for_unit("nginx.service")
            box.wait_for_unit("lososd.service")
            laptop.succeed("rm -rf /tmp/shared/mail-pgp-before")
            laptop.copy_from_machine("/tmp/shots-before", "mail-pgp-before")

    with subtest("a GnuPG-signed PGP/MIME part reaches Stalwart byte for byte"):
        part = (
            "Content-Type: text/plain; charset=utf-8\r\n"
            "Content-Transfer-Encoding: quoted-printable\r\n"
            "\r\n"
            "Signed with GnuPG.\r\n=46rom the laptop."
        )
        put(laptop, "/tmp/part.txt", part)
        laptop.succeed(
            "export GNUPGHOME=/tmp/signer && mkdir -p -m 700 $GNUPGHOME && "
            f"gpg --batch --pinentry-mode loopback --passphrase ''' --import {FIXTURE}/secret-post.asc && "
            "gpg --batch --armor --pinentry-mode loopback --passphrase ''' --digest-algo SHA256 "
            "--detach-sign -o /tmp/part.sig /tmp/part.txt"
        )
        signature = laptop.succeed("cat /tmp/part.sig")
        ok("/api/mail/send", eva, method="POST", data={
            "to": [f"matus@{DOMAIN}"],
            "subject": "Signed elsewhere",
            "text": "",
            "pgp": {"mime": "signed", "part": part, "signature": signature, "micalg": "pgp-sha256"},
        })
        got = wait_for(matus, folder(mhome, "inbox"), "Signed elsewhere")
        assert got["pgp"] == "signed", got
        message = raw(matus, got["id"])
        boundary = match(r'boundary="([^"]+)"', message, 1)
        first = message.split(f"--{boundary}\r\n", 2)[1]
        arrived = first[: first.rindex("\r\n")]
        assert arrived == part, (arrived, part)
        put(laptop, "/tmp/arrived.txt", arrived)
        laptop.succeed("GNUPGHOME=/tmp/signer gpg --batch --verify /tmp/part.sig /tmp/arrived.txt")
        # A part that is not plain 7-bit CRLF text is refused before it is sent.
        code, body = api("/api/mail/send", eva, method="POST", data={
            "to": [f"matus@{DOMAIN}"], "subject": "x", "text": "",
            "pgp": {"mime": "signed", "part": "Content-Type: text/plain\n\nbare LF", "signature": signature, "micalg": "pgp-sha256"},
        })
        assert code == 400, (code, body)

    with subtest("a key sent in an Autocrypt header is learned when the mail is opened"):
        assert ok("/api/mail/keys/find?email=post@example.com", matus)["keys"] == []
        keydata = box.succeed(f"cat {FIXTURE}/autocrypt-post").strip()
        folded = "\r\n ".join(keydata[i:i + 72] for i in range(0, len(keydata), 72))
        letter = (
            "From: Post Office <post@example.com>\r\n"
            f"To: matus@{DOMAIN}\r\n"
            "Subject: Your parcel\r\n"
            "Date: Thu, 22 Oct 2026 08:00:00 +0000\r\n"
            "Message-ID: <parcel-1@example.com>\r\n"
            f"Autocrypt: addr=post@example.com; prefer-encrypt=mutual; keydata=\r\n {folded}\r\n"
            "MIME-Version: 1.0\r\n"
            "Content-Type: text/plain; charset=utf-8\r\n"
            "\r\n"
            "Your parcel is at the post office.\r\n"
        )
        put(box, "/tmp/parcel.eml", letter)
        secret = box.succeed(
            "systemd-creds decrypt --name=mail-master /etc/credstore.encrypted/mail-master -"
        ).strip()
        jmap = f"curl -sf -u 'matus%losos:{secret}' http://127.0.0.1:8096"
        session = json.loads(box.succeed(f"{jmap}/jmap/session"))
        account = session["primaryAccounts"]["urn:ietf:params:jmap:mail"]
        blob = json.loads(box.succeed(
            f"{jmap}/jmap/upload/{account}/ -H 'Content-Type: message/rfc822' --data-binary @/tmp/parcel.eml"
        ))["blobId"]
        call = {
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:mail"],
            "methodCalls": [["Email/import", {"accountId": account, "emails": {"p": {
                "blobId": blob, "mailboxIds": {folder(mhome, "inbox"): True},
            }}}, "i"]],
        }
        box.succeed(f"cat > /tmp/import.json <<'EOF'\n{json.dumps(call)}\nEOF")
        out = json.loads(box.succeed(f"{jmap}/jmap/ -H 'Content-Type: application/json' -d @/tmp/import.json"))
        assert "p" in out["methodResponses"][0][1].get("created", {}), out
        parcel = wait_for(matus, folder(mhome, "inbox"), "Your parcel")
        # Eva opening her own mail teaches Matus nothing; Matus opening this does.
        ok(f"/api/mail/messages/{parcel['id']}", matus)
        found = ok("/api/mail/keys/find?email=post@example.com", matus)["keys"]
        want = box.succeed(f"cat {FIXTURE}/fpr-post@example.com").strip()
        assert [(k["source"], k["fingerprint"]) for k in found] == [("autocrypt", want)], found
        assert ok("/api/mail/keys/find?email=post@example.com", eva)["keys"] == []

    with subtest("removing a key takes it out of the directory"):
        ok("/api/mail/keys", eva, method="DELETE")
        assert ok("/api/mail/keys", eva) == {"key": None}
        h = wkd_hash("eva")
        code = curl(f"-o /dev/null -w '%{{http_code}}' https://{BOX}/.well-known/openpgpkey/hu/{h}")
        assert code == "404", code
        box.fail("test -e /var/lib/losos/mail-keys/eva/locked.cred")
        assert ok(f"/api/mail/keys/find?email=eva@{DOMAIN}", matus)["keys"] == []

    laptop.succeed("mail-pgp manifest losos-mail-pgp")
    laptop.succeed("rm -rf /tmp/shared/mail-pgp-shots")
    laptop.copy_from_machine("/tmp/shots", "mail-pgp-shots")
  '';
}
