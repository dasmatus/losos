# The edge for the two-VM LAN demo (demo/edge-lan/run.sh): a losos edge that
# is also the LAN's router, built as a plain QEMU VM.
#
# `nix build .#nixosConfigurations.edge-demo.config.system.build.vm` gives a
# `run-edge-vm` script. It has two NICs, in this order:
#
#   eth0  QEMU user networking — the way out to the internet (through the
#         host), and `hostfwd` so the host can reach the registrar API.
#   eth1  the demo LAN, a VDE switch socket named by $LOSOS_LAN_SOCK. The
#         edge is 10.77.0.1 on it and hands out 10.77.0.100-200 by DHCP,
#         answers DNS, and NATs the LAN out through eth0 — so a box on the
#         switch has internet only while the edge is up, which is what makes
#         the "edge goes away" half of the demo honest: with the edge off the
#         box's configured public registrar is unreachable too.
#
# The one losos setting that matters here is `losos.edge.lan.advertise`
# (modules/edge.nix): the registrar API is announced over mDNS as
# `_losos-edge._tcp` with `url=http://edge.local:8443`, and the box's lososd
# finds it with no configuration (backend/src/edge.rs). Traefik's ACME will fail
# in the VM (no public DNS); that is logged, not fatal, as in tests/edge-vm.nix.
# The tenant token files are demo fixtures written by tmpfiles — a real edge
# gets them out of band (handbook/docs/in-depth/master-proxy.md).
{
  lib,
  pkgs,
  ...
}:

let
  lanIp = "10.77.0.1";
  lanPrefix = 24;
  dhcpRange = "10.77.0.100,10.77.0.200,12h";
  proxyToken = "/var/secrets/losos-proxy-token";
  bootstrapToken = "/var/secrets/losos-rathole-bootstrap";
in
{
  networking.hostName = "edge";
  system.stateVersion = "26.11";

  # ── The losos edge ───────────────────────────────────────────────────────
  losos.edge = {
    enable = true;
    acmeEmail = "demo@losos.cfd";
    registrarApiPort = 8443;
    ratholePortRange = "50000-50010";
    heartbeatTtl = "120s";
    reconcileInterval = "5s";
    bootstrapTokenFile = bootstrapToken;
    tenants.mattbox = {
      hostname = "mattbox.losos.cfd";
      tokenFile = proxyToken;
    };
    # The feature the demo is about.
    lan.advertise = true;
  };

  systemd.tmpfiles.rules = [
    "d /var/secrets 0700 root root - -"
    "f ${proxyToken} 0600 root root - demo-proxy-token-0123456789abcdef"
    "f ${bootstrapToken} 0600 root root - demo-bootstrap-0123456789abcdef0123"
  ];

  # ── The LAN's router: static address, DHCP + DNS, NAT ───────────────────
  networking.useDHCP = false;
  networking.interfaces.eth0.useDHCP = true;
  networking.interfaces.eth1.ipv4.addresses = [
    {
      address = lanIp;
      prefixLength = lanPrefix;
    }
  ];
  networking.nat = {
    enable = true;
    externalInterface = "eth0";
    internalInterfaces = [ "eth1" ];
  };
  services.dnsmasq = {
    enable = true;
    # The LAN side only: eth0 belongs to QEMU's own DHCP.
    settings = {
      interface = "eth1";
      bind-interfaces = true;
      dhcp-range = dhcpRange;
      dhcp-option = [
        "option:router,${lanIp}"
        "option:dns-server,${lanIp}"
      ];
      # Forward to QEMU's resolver on the user network, which the host's
      # resolver answers for. .local is mDNS's and must never be forwarded.
      server = [ "10.0.2.3" ];
      local = [ "/local/" ];
      domain-needed = true;
      bogus-priv = true;
    };
  };
  # dnsmasq binds eth1's address (`bind-interfaces`), so it must not start
  # before scripted networking has put the address there: under TCG the gap
  # between the two is long enough for the unit to fail and restart a dozen
  # times before it sticks.
  systemd.services.dnsmasq = {
    after = [ "network-addresses-eth1.service" ];
    wants = [ "network-addresses-eth1.service" ];
  };
  networking.firewall.interfaces.eth1 = {
    allowedUDPPorts = [
      53
      67
    ];
    allowedTCPPorts = [ 53 ];
  };

  # ── A VM you can watch ───────────────────────────────────────────────────
  # Serial console on stdio with root logged in, so the demo's edge pane can
  # show `journalctl -f` and `tcpdump` of the box probing the registrar.
  # This is a demo VM: nothing here is on the appliance, and the edge module
  # proper (modules/edge.nix) adds no login of its own.
  virtualisation = {
    graphics = false;
    memorySize = 2048;
    cores = 2;
    diskSize = 4096;
    # Two NICs, in the order the comment at the top gives. The run script
    # interpolates these, so $LOSOS_LAN_SOCK and $LOSOS_EDGE_API_PORT are read
    # when the VM starts, not when it is built.
    qemu.networkingOptions = lib.mkForce [
      "-netdev user,id=wan,hostfwd=tcp:127.0.0.1:\${LOSOS_EDGE_API_PORT:-8443}-:8443"
      "-device virtio-net-pci,netdev=wan,mac=52:54:00:ed:9e:00"
      "-netdev vde,id=lan,sock=\"\${LOSOS_LAN_SOCK:?set LOSOS_LAN_SOCK to the vde_switch socket}\""
      "-device virtio-net-pci,netdev=lan,mac=52:54:00:ed:9e:01"
    ];
  };
  services.getty.autologinUser = "root";
  # The `console=ttyS0` generator's getty is bound to dev-ttyS0.device, and
  # under TCG udev's coldplug can finish after that device job's 90 s timeout
  # even though the kernel has been writing to ttyS0 all along: the getty's
  # dependency fails and nothing ever retries it, so the VM boots to no
  # prompt. The demo console is its own unit instead: agetty opens the port
  # the kernel already created, with no device unit in the way.
  systemd.services."serial-getty@ttyS0".enable = false;
  systemd.services.losos-demo-console = {
    description = "demo console on ttyS0 (root, autologin)";
    wantedBy = [ "multi-user.target" ];
    after = [
      "systemd-user-sessions.service"
      "getty-pre.target"
    ];
    unitConfig.IgnoreOnIsolate = true;
    serviceConfig = {
      ExecStart = "${pkgs.util-linux}/sbin/agetty --autologin root --noclear --keep-baud ttyS0 115200,57600,38400,9600 vt220";
      Type = "idle";
      Restart = "always";
      RestartSec = 0;
      UtmpIdentifier = "ttyS0";
      TTYPath = "/dev/ttyS0";
      TTYReset = true;
      TTYVHangup = true;
      KillMode = "process";
      IgnoreSIGPIPE = false;
      SendSIGHUP = true;
      StandardInput = "tty";
      StandardOutput = "tty";
    };
  };
  environment.systemPackages = with pkgs; [
    tcpdump
    jq
    avahi
    curl
  ];
  # Resolve the box's `.local` name from the edge's shell too.
  services.avahi.nssmdns4 = true;
  # The serial pane is a tool, not a product: a plain prompt reads better on
  # video than the distribution's.
  programs.bash.promptInit = "PS1='edge# ' ";
}
