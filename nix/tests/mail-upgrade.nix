# nixos-test-vms config for LosOS Mail's upgrade from Stalwart 0.15 to 1.0.
#
# The box boots with the 1.0 server (modules/mail.nix) and an empty store.
# The test then writes a 0.15 store the way an older LosOS build left one:
# the 0.15 server from nixpkgs, under the shared user, with the same
# administrator secret, holding Eva's account, a folder of her own, three
# messages and a filter. Then it starts `losos-mail-migrate` and checks
# that:
#
#   * every message, the folder and the filter are in 1.0, under
#     eva@<domain>, read with the address login lososd uses;
#   * mail Eva got in 1.0 before the copy is still there;
#   * the old store is renamed to db-0.15 and kept, so the copy does not
#     run again on the next boot;
#   * a second start of the server does not apply its plan again.
{ pkgs }:

let
  lososPkgs = import ../packages.nix { inherit pkgs; };
  stubUi = pkgs.runCommand "stub-admin-ui" { } ''
    mkdir -p $out
    echo "admin ui" > $out/index.html
  '';

  # What a 0.15 box's modules/mail.nix wrote, with the secret in a file the
  # test makes and a port of its own.
  seedConfig = (pkgs.formats.toml { }).generate "seed-0.15.toml" {
    server = {
      hostname = "mattbox.home.arpa";
      listener.http = {
        bind = [ "127.0.0.1:8199" ];
        protocol = "http";
      };
    };
    store.db = {
      type = "rocksdb";
      path = "/home/shared/data/mail/db";
      compression = "lz4";
    };
    storage = {
      data = "db";
      fts = "db";
      lookup = "db";
      blob = "db";
      directory = "internal";
    };
    directory.internal = {
      type = "internal";
      store = "db";
    };
    tracer.stdout = {
      type = "stdout";
      level = "info";
      enable = true;
    };
    resolver.type = "system";
    spam-filter.resource = "file://${pkgs.stalwart_0_15.spam-filter}/spam-filter.toml";
    webadmin = {
      path = "/var/tmp/seed-webadmin";
      resource = "file://${pkgs.stalwart_0_15.webadmin}/webadmin.zip";
      auto-update = false;
    };
    authentication = {
      fallback-admin = {
        user = "losos-admin";
        secret = "%{file:/run/seed-secret}%";
      };
      master = {
        user = "losos";
        secret = "%{file:/run/seed-secret}%";
      };
    };
    config.local-keys = [
      "store.*"
      "directory.*"
      "tracer.*"
      "server.*"
      "config.local-keys.*"
      "authentication.fallback-admin.*"
      "authentication.master.*"
      "storage.data"
      "storage.blob"
      "storage.lookup"
      "storage.fts"
      "storage.directory"
      "resolver.*"
      "spam-filter.resource"
      "webadmin.*"
    ];
  };
in

pkgs.testers.nixosTest {
  name = "losos-mail-upgrade";

  nodes.box =
    { ... }:
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
        admin.ui = stubUi;
        git.enable = false;
        sharingMyStorage = true;
        cluster.enable = true;
        shared.fscrypt.enable = false;
      };
      users.groups.shared.gid = 1001;
      users.users.shared = {
        uid = 1001;
        isNormalUser = true;
        group = "shared";
        homeMode = "700";
      };
      systemd.tmpfiles.rules = [ "d /home/shared/data 0700 shared shared -" ];
      environment.systemPackages = [
        pkgs.jq
        pkgs.curl
      ];
      virtualisation = {
        memorySize = 2048;
        cores = 2;
      };
    };

  testScript = ''
    import json

    DOMAIN = "mattbox.home.arpa"

    box.wait_for_unit("losos-mail.service")
    box.wait_for_open_port(8096)
    secret = box.succeed(
        "systemd-creds decrypt --name=mail-master /etc/credstore.encrypted/mail-master -"
    ).strip()

    def post(url, user, body):
        box.succeed(f"cat > /tmp/body.json <<'EOF'\n{json.dumps(body)}\nEOF")
        return json.loads(box.succeed(
            f"curl -sf -u '{user}:{secret}' {url} -H 'Content-Type: application/json' -d @/tmp/body.json"
        ))

    def jmap(base, user, calls, using=("core", "mail")):
        body = {"using": [f"urn:ietf:params:jmap:{u}" for u in using], "methodCalls": calls}
        return [r[1] for r in post(f"{base}/jmap/", user, body)["methodResponses"]]

    def account(base, user):
        s = json.loads(box.succeed(f"curl -sf -u '{user}:{secret}' {base}/jmap/session"))
        return s["primaryAccounts"]["urn:ietf:params:jmap:mail"]

    def deliver(base, user, acct, mailbox, subject):
        raw = (
            f"From: Matus <matus@{DOMAIN}>\r\nTo: Eva <eva@{DOMAIN}>\r\n"
            f"Subject: {subject}\r\nDate: Tue, 27 Oct 2026 09:00:00 +0000\r\n"
            f"Message-ID: <{subject.replace(' ', '-')}@{DOMAIN}>\r\n\r\n{subject}, in full.\r\n"
        )
        box.succeed(f"printf '%s' '{raw}' > /tmp/msg.eml")
        blob = json.loads(box.succeed(
            f"curl -sf -u '{user}:{secret}' -H 'Content-Type: message/rfc822' "
            f"--data-binary @/tmp/msg.eml {base}/jmap/upload/{acct}/"
        ))["blobId"]
        [done] = jmap(base, user, [["Email/import", {"accountId": acct, "emails": {
            "m": {"blobId": blob, "mailboxIds": {mailbox: True}, "keywords": {"$seen": True}}
        }}, "i"]])
        assert "m" in done.get("created", {}), done

    NEW = "http://127.0.0.1:8096"
    EVA_NEW = f"eva@{DOMAIN}%losos"

    with subtest("Eva already has mail in 1.0 before the copy"):
        # The registry's calls take Stalwart's own capability.
        doms = post(f"{NEW}/jmap/", "losos", {
            "using": ["urn:ietf:params:jmap:core", "urn:stalwart:jmap"],
            "methodCalls": [["x:Domain/get", {"ids": None}, "d"]],
        })["methodResponses"][0][1]["list"]
        dom = next(d["id"] for d in doms if d["name"] == DOMAIN)
        made = post(f"{NEW}/jmap/", "losos", {
            "using": ["urn:ietf:params:jmap:core", "urn:stalwart:jmap"],
            "methodCalls": [["x:Account/set", {"create": {"a": {
                "@type": "User", "name": "eva", "domainId": dom, "description": "Eva"}}}, "a"]],
        })["methodResponses"][0][1]
        assert "a" in made.get("created", {}), made
        acct = account(NEW, EVA_NEW)
        [boxes] = jmap(NEW, EVA_NEW, [["Mailbox/get", {"accountId": acct}, "b"]])
        inbox = next(m["id"] for m in boxes["list"] if m["role"] == "inbox")
        deliver(NEW, EVA_NEW, acct, inbox, "Arrived after the update")

    with subtest("a 0.15 store, as an older build left it"):
        # printf, not a here-string: the secret has no newline at its end.
        box.succeed(f"printf '%s' '{secret}' | install -o shared -g shared -m 0400 /dev/stdin /run/seed-secret")
        box.succeed(
            "systemd-run --unit=seed-0-15 -p User=shared -p Group=shared "
            "${pkgs.lib.getExe pkgs.stalwart_0_15} --config=${seedConfig}"
        )
        box.wait_for_open_port(8199)
        OLD = "http://127.0.0.1:8199"
        for doc in [
            {"type": "domain", "name": DOMAIN},
            {"type": "individual", "name": "eva", "description": "Eva",
             "emails": [f"eva@{DOMAIN}"], "roles": ["user"]},
        ]:
            post(f"{OLD}/api/principal", "losos-admin", doc)
        old = account(OLD, "eva%losos")
        [boxes] = jmap(OLD, "eva%losos", [["Mailbox/get", {"accountId": old}, "b"]])
        old_inbox = next(m["id"] for m in boxes["list"] if m["role"] == "inbox")
        [made] = jmap(OLD, "eva%losos", [["Mailbox/set", {"accountId": old, "create": {
            "r": {"name": "Receipts", "parentId": None}}}, "s"]])
        receipts = made["created"]["r"]["id"]
        deliver(OLD, "eva%losos", old, old_inbox, "Lunch on Friday")
        deliver(OLD, "eva%losos", old, old_inbox, "Photos from the lake")
        deliver(OLD, "eva%losos", old, receipts, "Your train ticket")
        sieve = 'require ["fileinto"];\r\nif header :contains "subject" "ticket" { fileinto "Receipts"; }\r\n'
        box.succeed(f"printf '%s' '{sieve}' > /tmp/losos.sieve")
        sblob = json.loads(box.succeed(
            f"curl -sf -u 'eva%losos:{secret}' -H 'Content-Type: application/sieve' "
            f"--data-binary @/tmp/losos.sieve {OLD}/jmap/upload/{old}/"
        ))["blobId"]
        [made] = jmap(OLD, "eva%losos", [["SieveScript/set", {"accountId": old, "create": {
            "sc": {"name": "losos", "blobId": sblob}}, "onSuccessActivateScript": "#sc"}, "s"]],
            ("core", "sieve"))
        assert "sc" in made.get("created", {}), made
        box.succeed("systemctl stop seed-0-15.service")
        box.succeed("test -d /home/shared/data/mail/db")

    with subtest("the copy carries everything into 1.0"):
        box.succeed("systemctl start losos-mail-migrate.service", timeout=900)
        box.succeed("test -d /home/shared/data/mail/db-0.15")
        box.fail("test -e /home/shared/data/mail/db")
        box.fail("test -e /home/shared/data/mail/migrate")
        [boxes] = jmap(NEW, EVA_NEW, [["Mailbox/get", {"accountId": acct}, "b"]])
        names = sorted(m["name"] for m in boxes["list"])
        assert "Receipts" in names, names
        assert names.count("Inbox") == 1, names
        [q, got] = jmap(NEW, EVA_NEW, [
            ["Email/query", {"accountId": acct}, "q"],
            ["Email/get", {"accountId": acct, "#ids": {"resultOf": "q", "name": "Email/query", "path": "/ids"},
                           "properties": ["subject", "mailboxIds"]}, "g"],
        ])
        subjects = sorted(e["subject"] for e in got["list"])
        assert subjects == ["Arrived after the update", "Lunch on Friday", "Photos from the lake", "Your train ticket"], subjects
        receipts_new = next(m["id"] for m in boxes["list"] if m["name"] == "Receipts")
        ticket = next(e for e in got["list"] if e["subject"] == "Your train ticket")
        assert receipts_new in ticket["mailboxIds"], ticket
        [scripts] = jmap(NEW, EVA_NEW, [["SieveScript/get", {"accountId": acct}, "g"]], ("core", "sieve"))
        assert [s["name"] for s in scripts["list"]] == ["losos"], scripts

    with subtest("the copy does not run again, and neither does the plan"):
        box.succeed("systemctl restart losos-mail.service")
        box.wait_for_open_port(8096)
        box.wait_until_succeeds(
            "journalctl -u losos-mail.service -b | grep -q \"already this plan's\""
        )
        box.succeed("systemctl start losos-mail-migrate.service")
        cond = box.succeed("systemctl show -p ConditionResult --value losos-mail-migrate.service").strip()
        assert cond == "no", cond
  '';
}
