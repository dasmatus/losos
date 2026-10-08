# nixos-test-vms config for LosOS Git's federation (losos.forgejo.federation,
# modules/options.nix).
#
# Forgejo's ActivityPub side is one line of app.ini, and both deployment modes
# carry that line from the same option: modules/services.nix writes it into
# services.forgejo.settings, modules/workloads.nix into the losos.ini the pod
# mounts. The failure this guards against is the quiet one — a `[federation]`
# section that renders under a misspelt key, or on one path and not the other,
# shows up as nothing: the endpoints answer 404 exactly as they do with the
# feature off, and no other server ever says why it could not find the box.
# So the real Forgejo is booted here and asked.
#
# Two VMs:
#
#   box   losos.forgejo.mode = "native": the real Forgejo from nixpkgs, on
#         its native port, with federation on (the default). The test
#         asks it for what the fediverse would: nodeinfo discovery, the
#         server actor, an account actor. A specialisation with the option
#         off is switched into afterwards, and the same addresses have to
#         answer 404 — the toggle is real in both directions.
#   pod   losos.forgejo.mode = "container" (the shipped default), with a
#         stand-in image so that modules/workloads.nix renders the static
#         pod and the losos.ini it mounts without a cluster running. The
#         test reads that file off the manifest's hostPath and asserts the
#         section is there with the two values the module promises.
#
# One thing to know when reading the assertions: the two modes ship different
# Forgejo majors. services.forgejo's package defaults to nixpkgs' forgejo-lts
# (15.x) and the pod image (flake/images.nix) is built from pkgs.forgejo
# (16.x). Both federate the same way and answer the same addresses; what moved
# between them is the order of two checks on an account's actor — 15 looks the
# account up before it verifies the request's HTTP signature, 16 the other way
# round — which is why the test creates an account before asking for one.
{ pkgs }:

let
  # What the pod's losos.ini has to carry, in the shape pkgs.formats.ini
  # renders (`key=value`, no spaces). The entrypoint in
  # flake/images.nix concatenates this file after the base and brand
  # fragments, so these are the lines Forgejo reads last.
  wantIni = [
    "[federation]"
    "ENABLED=true"
    "SHARE_USER_STATISTICS=false"
  ];
in

pkgs.testers.nixosTest {
  name = "losos-forgejo-federation";

  nodes = {
    box =
      { ... }:
      {
        imports = [
          ../modules/options.nix
          ../modules/services.nix
          # services.nix names lososInternal.nextcloudStack inside a mkIf; the
          # module system still has to see the option declared to distribute
          # that mkIf over services.nextcloud.*, so the declaring module comes
          # along. Nothing of Nextcloud runs: the mode below keeps it off.
          ../modules/nextcloud-common.nix
        ];

        losos = {
          hostName = "mattbox";
          forgejo.enable = true;
          forgejo.mode = "native";
          # Keeps services.nix from enabling the native Nextcloud stack; only
          # Forgejo is under test. The Nextcloud pod is not run here either.
          nextcloud.mode = "container";
        };

        # The other half of the toggle, switched into from the test script.
        # `switch-to-configuration test` restarts forgejo.service because its
        # generated app.ini changed; Forgejo then runs without the section.
        specialisation.federation-off.configuration = {
          losos.forgejo.federation.enable = false;
        };

        environment.systemPackages = [
          pkgs.curl
          pkgs.jq
        ];

        # initdb under emulation on a loaded host has overrun systemd's
        # default start timeout; the default is for a box with KVM, and this
        # check also has to pass without it.
        systemd.services.postgresql.serviceConfig.TimeoutStartSec = "15min";

        virtualisation = {
          memorySize = 2048;
          cores = 2;
        };
      };

    pod =
      { ... }:
      {
        imports = [
          ../modules/options.nix
          ../modules/workloads.nix
        ];

        losos = {
          hostName = "mattbox";
          forgejo.enable = true;
          forgejo.mode = "container";
          # No Nextcloud pod: nothing here needs it, and its manifest would
          # want the real image.
          nextcloud.mode = "native";
          # Any derivation with the two attributes imageRef reads. The
          # kubelet is not running, so the reference is never pulled; what
          # matters is the manifest and the config directory it points at.
          workloads.forgejoImage = pkgs.hello // {
            imageName = "losos-forgejo-stand-in";
            imageTag = "test";
          };
        };

        virtualisation = {
          memorySize = 1024;
          cores = 1;
        };
      };
  };

  testScript =
    { nodes, ... }:
    ''
      import json
      import re
      from urllib.parse import urlparse

      PORT = 8888  # services.nix: the native HTTP_PORT
      NODEINFO_REL = "http://nodeinfo.diaspora.software/ns/schema/2.1"
      # The box's own Forgejo CLI, run the way the unit runs the server: as its
      # user, on its state directory, so it reads the same app.ini.
      FORGEJO = (
          "runuser -u ${nodes.box.services.forgejo.user} -- env"
          " FORGEJO_WORK_DIR=${nodes.box.services.forgejo.stateDir}"
          " FORGEJO_CUSTOM=${nodes.box.services.forgejo.customDir}"
          " HOME=${nodes.box.services.forgejo.stateDir}"
          " ${pkgs.lib.getExe nodes.box.services.forgejo.package}"
      )

      def get(path):
          return box.succeed(
              f"curl -s -o /tmp/body -w '%{{http_code}}' http://127.0.0.1:{PORT}{path}; echo; cat /tmp/body"
          ).split("\n", 1)

      start_all()

      with subtest("the native Forgejo is up with the LosOS settings"):
          box.wait_for_unit("forgejo.service")
          box.wait_for_open_port(PORT)
          # The page must come from the configured instance, not a default one.
          box.wait_until_succeeds(f"curl -sf http://127.0.0.1:{PORT}/ | grep -q 'LosOS Git'")

      with subtest("nodeinfo discovery answers at the host's root"):
          code, body = get("/.well-known/nodeinfo")
          assert code == "200", f"/.well-known/nodeinfo: {code} {body!r}"
          links = json.loads(body)["links"]
          link = next((l for l in links if l["rel"] == NODEINFO_REL), None)
          assert link, f"no 2.1 link: {links!r}"
          # The host Forgejo advertises itself under (ROOT_URL's, port included),
          # which is also the only host its webfinger answers for.
          HOST = urlparse(link["href"]).netloc
          code, body = get("/api/v1/nodeinfo")
          assert code == "200", f"/api/v1/nodeinfo: {code} {body!r}"
          info = json.loads(body)
          assert info["software"]["name"] == "forgejo", info
          assert "activitypub" in info["protocols"], info
          # SHARE_USER_STATISTICS = false: the usage block carries no totals.
          assert "total" not in json.dumps(info["usage"]), f"statistics published: {info['usage']!r}"

      with subtest("the server actor is an ActivityPub Application with a key"):
          code, body = get("/api/v1/activitypub/actor")
          assert code == "200", f"actor: {code} {body!r}"
          actor = json.loads(body)
          assert actor["type"] == "Application", actor
          assert actor["publicKey"]["publicKeyPem"].startswith("-----BEGIN PUBLIC KEY-----"), actor
          assert actor["inbox"].endswith("/api/v1/activitypub/actor/inbox"), actor

      with subtest("an account's actor exists and refuses an unsigned request"):
          # With federation off this route does not exist (404). On, every
          # request for an actor has to carry an HTTP signature (Forgejo's
          # SIGNATURE_ENFORCED default), so a bare curl is refused — 400, with
          # the reason — which is what a stranger on the internet gets. The
          # account is created first because Forgejo 15 answers 404 for an
          # unknown account before it looks at the signature (see the header).
          box.succeed(
              f"{FORGEJO} admin user create --username alice --email alice@example.org"
              " --password 'Alice-passw0rd!' --must-change-password=false"
          )
          code, body = get(f"/.well-known/webfinger?resource=acct:alice@{HOST}")
          assert code == "200", f"webfinger for alice: {code} {body!r}"
          actor_url = next(
              l["href"] for l in json.loads(body)["links"] if l.get("type") == "application/activity+json"
          )
          actor_path = re.sub(r"^https?://[^/]+", "", actor_url)
          assert actor_path.startswith("/api/v1/activitypub/user-id/"), actor_url
          code, body = get(actor_path)
          assert code == "400", f"{actor_path}: {code} {body!r}"
          assert "signature" in body, f"{actor_path} refused for another reason: {body!r}"

      with subtest("the container manifest's losos.ini carries the same section"):
          manifest = pod.succeed("cat /var/lib/losos/k3s-static-pods/forgejo.yaml")
          paths = re.findall(r"(/nix/store/[^\s\"']*losos-forgejo-conf)", manifest)
          assert paths, f"no losos-forgejo-conf hostPath in the manifest:\n{manifest}"
          ini = pod.succeed(f"cat {paths[0]}/losos.ini")
          for line in ${builtins.toJSON wantIni}:
              assert line in ini.splitlines(), f"{line!r} missing from losos.ini:\n{ini}"

      with subtest("with losos.forgejo.federation.enable = false the endpoints are gone"):
          box.succeed(
              "/run/booted-system/specialisation/federation-off/bin/switch-to-configuration test"
          )
          box.wait_for_unit("forgejo.service")
          box.wait_for_open_port(PORT)
          box.wait_until_succeeds(f"curl -sf http://127.0.0.1:{PORT}/ | grep -q 'LosOS Git'")
          for path in ["/.well-known/nodeinfo", "/api/v1/nodeinfo", "/api/v1/activitypub/actor", actor_path]:
              code, body = get(path)
              assert code == "404", f"federation off, {path}: expected 404, got {code} {body!r}"
    '';
}
