# nixos-test-vms config for the on-box Actions runner (modules/git-runner.nix).
#
# Native Forgejo (services.forgejo on :8888, Postgres over the socket) plus the
# runner, and the registration path asserted end to end rather than by
# reading the unit file:
#
#   * the secret is generated on first start — 40 hex characters, 0600 root —
#     and nothing else writes it;
#   * `forgejo-cli actions register` wrote exactly one runner row, whose UUID
#     is the one the runner derives from the same secret (its first sixteen
#     characters as bytes, a detail the first run of this test corrected);
#   * the daemon actually connects: Forgejo stamps `last_online` on the row
#     the first time the runner polls it, and the row carries the `losos`
#     label the daemon declared. A config that only *looked* right (wrong
#     UUID, token not readable through the credential) stays at 0 forever;
#   * a restart re-registers idempotently: still one row, same UUID.
#
# Container mode is the same script against the same paths, with the pod
# writing the app.ini instead of forgejo.service; it needs k3s and the image,
# which this test does not boot. What it does share is the register script
# and the config template, so a mistake in either shows up here.
{ pkgs }:

let
  lososPkgs = import ../flake/packages.nix { inherit pkgs; };
in

pkgs.testers.nixosTest {
  name = "losos-git-runner";

  nodes.box =
    { ... }:
    {
      imports = [
        ../modules/options.nix
        # services.nix reads the shared Nextcloud stack even in container
        # mode (the mkIf is on the value, the attribute is still looked up).
        ../modules/nextcloud-common.nix
        ../modules/services.nix
        ../modules/git-runner.nix
      ];

      losos = {
        hostName = "mattbox";
        forgejo.mode = "native";
        forgejo.enable = true;
        # So the registrar lands on the runner's PATH, as on the real box
        # (modules/defaults.nix wires it there).
        proxy.registrar.package = lososPkgs.losos-registrar;
      };

      virtualisation = {
        memorySize = 2048;
        cores = 2;
      };
    };

  testScript = ''
    import re

    SECRET = "/var/secrets/losos-git-runner-secret"

    def sql(q):
        # Peer auth as the forgejo user, the way the register step itself
        # reaches the database.
        return box.succeed(f"runuser -u forgejo -- psql -d forgejo -tAc \"{q}\"").strip()

    start_all()
    box.wait_for_unit("forgejo.service")
    box.wait_for_open_port(8888)
    box.wait_for_unit("losos-git-runner.service")

    with subtest("the secret is generated once, root-only, 40 hex characters"):
        secret = box.succeed(f"cat {SECRET}").strip()
        assert re.fullmatch(r"[0-9a-f]{40}", secret), secret
        assert box.succeed(f"stat -c %a:%U {SECRET}").strip() == "600:root"

    # forgejo-cli's derivation: the secret's first 16 characters, as bytes.
    h = secret[0:16].encode().hex()
    uuid = f"{h[0:8]}-{h[8:12]}-{h[12:16]}-{h[16:20]}-{h[20:32]}"

    with subtest("Forgejo knows exactly one runner, by the secret's UUID"):
        rows = sql("select uuid || ' ' || name from action_runner").splitlines()
        assert rows == [f"{uuid} losos-box"], rows

    with subtest("the daemon connects with that identity and declares its label"):
        # Forgejo writes last_online the first time the runner polls it; a
        # runner that never connects leaves it at 0.
        retry(
            lambda _: sql(f"select last_online from action_runner where uuid='{uuid}'") != "0",
            timeout_seconds=180,
        )
        labels = sql(f"select agent_labels from action_runner where uuid='{uuid}'")
        assert "losos" in labels, labels

    with subtest("the runner's config carries the UUID and reads the token as a credential"):
        conf = box.succeed("cat /var/lib/losos-git-runner/config.yaml")
        assert uuid in conf, conf
        assert "$CREDENTIALS_DIRECTORY/secret" in conf, conf
        assert "losos:host" in conf, conf
        assert secret not in conf, "the secret must not be copied into the config"

    with subtest("a restart re-registers idempotently"):
        box.systemctl("restart losos-git-runner.service")
        box.wait_for_unit("losos-git-runner.service")
        rows = sql("select uuid from action_runner").splitlines()
        assert rows == [uuid], rows
        assert box.succeed(f"cat {SECRET}").strip() == secret

    with subtest("the registrar is on the job PATH"):
        box.succeed("systemctl show -p Environment losos-git-runner.service | grep -q losos-registrar")
  '';
}
