# The box half of models split across boxes (losos.inference.lend,
# modules/inference-lend.nix), on one VM with no edge:
#
#   1. losos-llm-rpc runs ggml-rpc-server as `shared` (uid 1001), never as the
#      owner's `notshared`, with its file cache under /var/lib;
#   2. /run/losos-llm/lend holds the MiB lent and nothing else, readable by
#      everyone, and is gone once the unit stops;
#   3. MemoryMax is the lent memory plus the server's overhead;
#   4. on loopback only root (rathole's client) may connect to the RPC port:
#      `notshared` and `shared` are refused;
#   5. the rathole client carries `<appliance id>:llm` to the RPC port with
#      the box's own token, and announce is handed the lend file.
#
# The registrar is a stand-in that only sleeps: announce's flag is asserted
# on the unit, and nothing here dials an edge.
{ pkgs }:

let
  proxyToken = "/var/secrets/losos-proxy-token";
  bootstrapToken = "/var/secrets/losos-rathole-bootstrap";
  lentMiB = 1536;
  # modules/inference-lend.nix's overheadMiB.
  maxBytes = (lentMiB + 256) * 1024 * 1024;
  registrarStub = pkgs.writeShellScriptBin "losos-registrar" "exec sleep infinity";
in
pkgs.testers.nixosTest {
  name = "losos-inference-lend";

  nodes.box =
    { lib, ... }:
    {
      imports = [
        ../../modules/options.nix
        ../../modules/configuration.nix
        ../../modules/proxy.nix
        ../../modules/inference-lend.nix
      ];

      # The test framework sets up the network itself.
      networking.networkmanager.enable = lib.mkForce false;

      systemd.tmpfiles.rules = [
        "d /var/secrets 0700 root root - -"
        "f ${proxyToken} 0600 root root - test-proxy-token-0123456789abcdef"
        "f ${bootstrapToken} 0600 root root - test-bootstrap-0123456789abcdef0123"
      ];

      losos = {
        hostName = "lendbox";
        proxy = {
          enable = true;
          edgeRatholeEndpoint = "edge.invalid:2333";
          registrarUrl = "http://edge.invalid:8443";
          hostname = "lendbox.losos.cfd";
          applianceId = "lendbox";
          tokenFile = proxyToken;
          bootstrapTokenFile = bootstrapToken;
          # No edge to fetch a Noise key from.
          noisePublicKeyFile = null;
          registrar.package = registrarStub;
        };
        inference.lend = {
          enable = true;
          memoryMiB = lentMiB;
          threads = 1;
        };
      };

      virtualisation = {
        memorySize = 2048;
        cores = 2;
      };
    };

  testScript = ''
    box.start()
    box.wait_for_unit("multi-user.target")
    box.wait_for_unit("losos-llm-rpc.service")

    # 1. The server runs as shared, uid 1001.
    pid = box.succeed("systemctl show -p MainPID --value losos-llm-rpc.service").strip()
    owner = box.succeed(f"ps -o uid=,user= -p {pid}").split()
    print("losos-llm-rpc runs as", owner)
    assert owner == ["1001", "shared"], f"losos-llm-rpc runs as {owner}"
    cmd = box.succeed(f"tr '\\0' ' ' < /proc/{pid}/cmdline")
    print("losos-llm-rpc command line:", cmd)
    assert "-d CPU" in cmd and "-p 50052" in cmd and "-t 1" in cmd, cmd

    # The port is up, and the file cache is under /var/lib, owned by shared.
    box.wait_until_succeeds("ss -Htln 'sport = :50052' | grep -q 127.0.0.1:50052")
    cache = box.succeed("stat -c '%U %a' /var/lib/losos-llm-rpc/rpc").strip()
    print("rpc cache:", cache)
    assert cache.startswith("shared "), f"cache owned by {cache}"

    # 2. The lend file.
    box.wait_until_succeeds("test -e /run/losos-llm/lend")
    lend = box.succeed("cat /run/losos-llm/lend")
    print("lend file:", repr(lend))
    assert lend == "${toString lentMiB}", f"lend file holds {lend!r}"
    mode = box.succeed("stat -c %a /run/losos-llm/lend").strip()
    assert mode == "644", f"lend file mode {mode}"
    box.succeed("runuser -u notshared -- cat /run/losos-llm/lend")

    # 3. MemoryMax.
    mem = box.succeed("systemctl show -p MemoryMax --value losos-llm-rpc.service").strip()
    print("MemoryMax:", mem)
    assert mem == "${toString maxBytes}", f"MemoryMax is {mem}"

    # 4. Only root may connect.
    connect = "bash -c 'exec 3<>/dev/tcp/127.0.0.1/50052'"
    box.succeed(connect)
    print("root connects to 127.0.0.1:50052")
    for user in ["notshared", "shared"]:
        out = box.fail(f"runuser -u {user} -- {connect} 2>&1")
        print(f"{user} refused:", out.strip())
    # The server took root's connection and closing it, and is still up.
    box.succeed("systemctl is-active losos-llm-rpc.service")

    # 5. The tunnel service and announce's flag.
    box.wait_until_succeeds("test -s /run/losos-rathole/client.toml")
    client = box.succeed("cat /run/losos-rathole/client.toml")
    print(client)
    assert '[client.services."lendbox:llm"]' in client, client
    llm = client.split('[client.services."lendbox:llm"]', 1)[1]
    assert 'token = "test-proxy-token-0123456789abcdef"' in llm, llm
    assert 'local_addr = "127.0.0.1:50052"' in llm, llm
    announce = box.succeed("systemctl show -p ExecStart --value losos-registrar-announce.service")
    print("announce:", announce)
    assert "--llm-lend-file /run/losos-llm/lend" in announce, announce

    # 2, the other half: stopped, the box lends nothing.
    box.succeed("systemctl stop losos-llm-rpc.service")
    box.succeed("test ! -e /run/losos-llm/lend")
    print("lend file gone after stop")
  '';
}
