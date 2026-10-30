# The lososd control-plane daemon + facade wiring.
#
# lososd runs as a root systemd service: it owns /var/lib/losos/state.json,
# runs supervised `systemd-run` rebuild transient units, exports the
# org.losos1 name on the system D-Bus (org.losos.Control1 interface: State /
# Settings / Change / Apply / FactoryReset / Status), and serves a
# Bearer-authed loopback JSON API on 127.0.0.1:<losos.admin.apiPort> —
# the same contract as backend/schema.json (see backend/src/dbus.rs for the
# bus interface and backend/src/http.rs for the HTTP surface).
#
# The `losos-ctl` facade CLI (same package) relays subcommands over D-Bus, so
# nothing but the daemon ever rewrites the flake or triggers a rebuild.
{
  pkgs,
  lib,
  config,
  ...
}:

let
  enabled = config.losos.backend.package != null;
  pkg = config.losos.backend.package;
  domainCheck = config.losos.proxy.gateways.domain;

  # System-bus policy: root owns org.losos1; members of the fixed `losos`
  # group may send to it (the facade runs as root or a losos-group caller).
  # Everyone else is denied.
  dbusPolicy = pkgs.writeTextDir "share/dbus-1/system.d/org.losos1.conf" ''
    <?xml version="1.0"?>
    <!DOCTYPE busconfig PUBLIC "-//freedesktop.org//DTD D-Bus Bus Configuration 1.0//EN"
      "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
    <busconfig>
      <policy user="root">
        <allow own="org.losos1"/>
        <allow send_destination="org.losos1"/>
      </policy>
      <policy group="losos">
        <allow send_destination="org.losos1"/>
      </policy>
      <policy context="default">
        <deny send_destination="org.losos1"/>
      </policy>
    </busconfig>
  '';
in
{
  # The credential store every secret here lives in.
  imports = [ ./credentials.nix ];

  config = lib.mkIf enabled {
    # Callers of the facade (root plus any future in-container tooling) send
    # to org.losos1 via this group.
    users.groups.losos = { };

    # The facade CLI for root; the daemon binary is started by the unit below.
    environment.systemPackages = [ pkg ];

    services.dbus.packages = [ dbusPolicy ];

    # Only root and nginx may open a connection to the loopback API.
    #
    # lososd binds 127.0.0.1, which keeps the LAN out, but not this box: the
    # apps' pods are hostNetwork and share its loopback, and LosOS Git runs
    # as uid 1003 and is on the internet whenever the master proxy is. Going
    # around nginx skips the lanOnly guard entirely, and
    # lososd trusts X-Real-IP from any loopback peer because it assumes that
    # peer is nginx. So a compromised pod could claim an unclaimed box (the
    # claim route hands back the admin token, which is root), lock the owner
    # out by spending their address's failed-auth budget, and write false
    # addresses into the audit log.
    #
    # A unix socket with SO_PEERCRED would say the same thing in the daemon,
    # but the hardening profile gives lososd no AF_NETLINK, so it cannot ask
    # the kernel for a TCP peer's uid itself, and moving nginx to a
    # socket changes every test that talks to :8082. An owner match on
    # OUTPUT says it at the kernel instead: a locally generated packet to the
    # port is rejected unless its socket belongs to root (the daemon's own
    # tooling, the VM tests), nginx or the log reader. nix/tests/admin-vm.nix
    # asserts both halves.
    #
    # `-I`, so it sits ahead of anything appended later; the delete first
    # makes a firewall reload idempotent. nginx is named only when it runs:
    # iptables refuses an owner match for a user that does not exist, and a
    # firewall that fails to start is a box with no front door.
    networking.firewall =
      let
        rule = lib.concatStringsSep " " (
          [
            "OUTPUT -o lo -p tcp -d 127.0.0.1 --dport ${toString config.losos.admin.apiPort}"
            "-m owner ! --uid-owner 0"
          ]
          ++ lib.optional config.services.nginx.enable "-m owner ! --uid-owner ${config.services.nginx.user}"
          # The log reader (modules/log-reader.nix) asks who is signed in,
          # passing on the person nginx named to it.
          ++ lib.optional (config.users.users ? losos-logs) "-m owner ! --uid-owner losos-logs"
          ++ [ "-j REJECT --reject-with tcp-reset" ]
        );
      in
      lib.mkIf config.networking.firewall.enable {
        extraCommands = ''
          iptables -w -D ${rule} 2>/dev/null || true
          iptables -w -I ${rule}
        '';
        extraStopCommands = ''
          iptables -w -D ${rule} 2>/dev/null || true
        '';
      };

    # The seeded keys, one armoured block after another. Public keys, so the
    # store is a fine place for them; lososd reads the file at start.
    environment.etc."losos/access-keys.asc".text =
      lib.concatStringsSep "\n" config.losos.access.gpgKeys;

    # The spare key: lososd's Bearer token, sealed in the credential store
    # (modules/credentials.nix) and opened into lososd's own credential
    # directory as it starts. A box from before the store kept it as plaintext
    # at losos.admin.tokenFile; the boot unit adopts that value, so a spare
    # key the owner wrote down keeps working, and deletes the file.
    losos.credentials.entries.admin-token = {
      app = "admin";
      description = "The spare key: the token that opens the admin API whether or not LosOS Login is up.";
      format = "hex-32";
      services = [ "lososd" ];
      legacy = [ (toString config.losos.admin.tokenFile) ];
    };

    # The box's escrow key (backend/src/escrow_key.rs): with the edge's,
    # one of the three keys a USDC payment held in escrow needs two of.
    # Not rotatable: a deposit names the key it was paid with, and a new
    # value would leave every payment still held waiting on its deadline.
    losos.credentials.entries.market-escrow-key = {
      app = "market";
      description = "The box's key for payments held in escrow: it agrees to a release of what this box sold and a refund of what it bought.";
      format = "hex-32";
      services = [ "lososd" ];
    };

    systemd.services.lososd = {
      description = "losos appliance control daemon (D-Bus + admin HTTP API)";
      wantedBy = [ "multi-user.target" ];
      after = [ "dbus.service" ];
      requires = [ "dbus.service" ];
      # lososd reads the seeded keys once, at start.
      restartTriggers = [ config.environment.etc."losos/access-keys.asc".source ];
      environment = {
        LOSOS_ADMIN_PORT = toString config.losos.admin.apiPort;
        LOSOS_ADMIN_TOKEN_FILE = "%d/admin-token";
        LOSOS_ESCROW_KEY_FILE = "%d/market-escrow-key";
        # The credential list and what it is sealed to (backend/src/credentials.rs).
        LOSOS_CREDENTIALS_FILE = "/etc/losos/credentials.json";
        # Terminal access (backend/src/access.rs): the box's own OpenPGP key
        # and the token file under its state directory, and the owner's
        # seeded keys from losos.access.gpgKeys.
        LOSOS_ACCESS_DIR = "/var/lib/losos/access";
        LOSOS_ACCESS_KEYS_FILE = "/etc/losos/access-keys.asc";
        # Traefik's metrics for the statistics plugin
        # (backend/plugins/statistics, modules/apps.nix). Set on every box,
        # apps or not: lososd refuses to start a plugin whose manifest names
        # a variable it lacks, and the plugin answers "unavailable" while
        # nothing listens there.
        LOSOS_TRAEFIK_URL = "http://127.0.0.1:${toString config.losos.apps.traefikPort}";
      }
      # Where lososd looks for an edge proxy besides the LAN (backend/src/edge.rs):
      # the configured registrar, probed whether or not the master proxy is on,
      # because the question is "is there an edge anywhere this box could share
      # through", not "is this box enrolled". The LAN half needs no setting; it
      # is the `_losos-edge._tcp` browse over the resolved the box already
      # runs for its own `.local` name (modules/mdns.nix).
      // {
        LOSOS_EDGE_URL = config.losos.proxy.registrarUrl;
        LOSOS_SHARING_PAUSE_AFTER_SECS = toString config.losos.proxy.pauseSharingAfter;
        # Where the tunnel goes when that configured edge is the path. An
        # edge found on the LAN carries its own in the advert (`tunnel=`).
        LOSOS_EDGE_TUNNEL = config.losos.proxy.edgeTunnelEndpoint;
        # What a box checks an edge against before it trusts it with
        # trading (losos.proxy.gateways): the TPM makers' certificates, the
        # published gateway images and LosOS's payout addresses. Public
        # data, so store paths.
        LOSOS_GATEWAY_TPM_ROOTS = "${config.losos.proxy.gateways.tpmRoots}";
        LOSOS_GATEWAY_IMAGES = "${pkgs.writeText "losos-gateway-images.json" (
          builtins.toJSON config.losos.proxy.gateways.images
        )}";
        LOSOS_GATEWAY_PAYOUTS = "${pkgs.writeText "losos-gateway-payouts.json" (
          builtins.toJSON config.losos.proxy.gateways.payouts
        )}";
        # And the name it is reached at: DNSSEC-signed, with an ECH key
        # (losos.proxy.gateways.domain, `delv` below on the unit's path).
        LOSOS_EDGE_DOMAIN_CHECK = if domainCheck.enable then "1" else "0";
      }
      // lib.optionalAttrs (domainCheck.server != null) {
        LOSOS_EDGE_DNS_SERVER = domainCheck.server;
      }
      // lib.optionalAttrs (domainCheck.trustAnchors != null) {
        LOSOS_EDGE_DNS_ANCHORS = domainCheck.trustAnchors;
      }
      // lib.optionalAttrs (domainCheck.trustAnchorRoot != null) {
        LOSOS_EDGE_DNS_ROOT = domainCheck.trustAnchorRoot;
      }
      // {
        # Edges and official clusters the box must not use (losos.proxy.bansFile):
        # read on every scan, a missing file bans nothing.
        LOSOS_EDGE_BANS_FILE = config.losos.proxy.bansFile;
      }
      # What `cryptsetup resize` authenticates with during `losos-ctl grow`.
      #
      # In both unlock modes: the installer formats from this keyfile and
      # keeps a copy inside /persist on the TPM path too (the recovery slot),
      # so the same path works whether the chip or the initrd opened the
      # volume. Without it cryptsetup would fall back to prompting on a stdin
      # the daemon does not have and fail *after* lvextend had already run.
      // {
        LOSOS_LUKS_KEYFILE = "/etc/keys/persist-keyfile";
      }
      # Where `losos` reaches the box away from its LAN, which the host key's
      # answer names for it to remember (losos.access.throughEdge; the two
      # routes the tunnel may reach are in modules/containers.nix).
      // lib.optionalAttrs (config.losos.proxy.enable && config.losos.access.throughEdge) {
        LOSOS_ACCESS_EDGE_URL = "https://${config.losos.proxy.hostname}";
      }
      # The plugins lososd loads (backend/plugins/README.md), a store path:
      # lososd refuses one anywhere else. Unset, it serves no plugin routes.
      // lib.optionalAttrs (config.losos.backend.plugins != null) {
        LOSOS_PLUGIN_DIR = "${config.losos.backend.plugins}";
      }
      # The built admin page, for the pages without JavaScript
      # (backend/plugins/basic/): its stylesheet's name and the catalogue
      # written out for them (basic/messages.<locale>.json).
      // lib.optionalAttrs (config.losos.admin.ui != null) {
        LOSOS_ADMIN_UI_DIR = "${config.losos.admin.ui}";
      }
      // lib.optionalAttrs config.losos.lab.ordering.enable {
        LOSOS_LAB_ORDERING = "1";
      }
      # How `/api/market*` reaches the edge's market (backend/src/market.rs).
      # Only with the master proxy on: without a registrar the daemon finds
      # none of these and answers `available: false`, which is the right
      # reading of a box that has no edge to trade through. The token *file*
      # path, never its contents: lososd reads it per request and sends it to
      # curl on stdin, so it is in no environment and no argument vector.
      // lib.optionalAttrs config.losos.proxy.enable {
        LOSOS_REGISTRAR_URL = config.losos.proxy.registrarUrl;
        LOSOS_APPLIANCE_ID = config.losos.proxy.applianceId;
        LOSOS_PROXY_TOKEN_FILE = toString config.losos.proxy.tokenFile;
        # The path (handbook/docs/in-depth/edge-federation.md, "The box"): with
        # the master proxy on, every scan ends by telling modules/proxy.nix's
        # two units which edge to dial. The env file carries the chosen edge,
        # the marker says there is none, and the daemon restarts or stops the
        # units on a change. Both files are on /run, so a boot starts clean.
        # Only with the proxy on: without it there are no units to drive, and
        # the scan only answers GET /api/edge.
        LOSOS_EDGE_PATH_FILE = "/run/losos/edge-path.env";
        LOSOS_EDGE_NONE_FILE = "/run/losos/edge-none";
        # The relay pass the official edge issues with the domains view, for
        # announce to hand a local edge (modules/proxy.nix reads the same
        # path). Root-only, on /run: it is good for hours, not across a boot.
        LOSOS_RELAY_PASS_FILE = "/run/losos/relay-pass";
        # Present while the owner lets the company edge in use manage the
        # box; announce says so on its heartbeats (modules/proxy.nix).
        LOSOS_EDGE_MANAGED_FILE = "/run/losos/edge-managed";
        LOSOS_EDGE_PIN_DIR = "/var/secrets/losos-edge-pins";
        # The configured edge's tunnel pin; a LAN edge's goes under
        # LOSOS_EDGE_PIN_DIR. lososd also mints the proxy token (64 hex
        # characters, 0600) when absent, so a stock box can enrol with a LAN
        # gateway that takes any box on first contact; an official edge
        # still has to be given it out of band.
        LOSOS_EDGE_TUNNEL_PIN_FILE = toString config.losos.proxy.tunnelPinFile;
      };
      # What `losos-ctl grow` shells out to. A systemd unit's default PATH does
      # not include these, and the failure is the unhelpful kind: the grow
      # reports "No such file or directory" for lvextend after the admin has
      # already been told the disk is full.
      #
      # coreutils is for `df`, which is how the daemon measures whether the
      # filesystem actually got bigger rather than trusting an exit status.
      #
      # `path` *replaces* PATH rather than extending it, so nothing is
      # reachable by accident.
      path = [
        pkgs.lvm2
        pkgs.cryptsetup
        pkgs.e2fsprogs
        pkgs.coreutils
        # What `GET /api/apps/search` shells out to. The admin pages are served
        # under `connect-src 'self'`, so the browser cannot reach a catalogue
        # itself and lososd does the search — see backend/src/catalogue.rs for
        # why that is a subprocess rather than a client crate. Same failure
        # shape as the grow binaries above if this line goes: ENOENT at the
        # exec, reported to the owner as a search that will not come back.
        pkgs.curl
        # delv: the edge scan's validating lookup of an official edge's name
        # (losos.proxy.gateways.domain, io_backend.rs `check_domain`).
        pkgs.bind.dnsutils
        # The edge scan's LAN browse needs no binary here: it asks
        # systemd-resolved over its Varlink socket (backend/src/edge.rs), which
        # is why there is no avahi-browse on this list any more.
        # What Apply, a storage-mode change and a factory reset run. The
        # rebuild is a `systemd-run` transient unit, and systemd-run resolves a
        # bare command against the caller's PATH before PID 1 sees it: with
        # nixos-rebuild missing here, every one of them failed with "Failed to
        # find executable nixos-rebuild" and the admin UI said only "That did
        # not start". supervisor.rs passes this PATH on to the unit
        # (--setenv=PATH), which is how nix and git reach the rebuild; PID 1's
        # own default PATH has neither. The same three autoUpgrade puts on
        # its unit.
        config.system.build.nixos-rebuild
        config.nix.package.out
        pkgs.gitMinimal
      ];
      serviceConfig = {
        ExecStart = "${pkg}/bin/lososd";
        # `always`, not `on-failure`: a clean exit(0) — an unhandled shutdown
        # path, a bus disconnect the daemon treats as terminal — would
        # otherwise leave the box with no control plane and no way in (no SSH,
        # no shell logins). The tunnel and registrar units use `always` for the
        # same reason.
        Restart = "always";
        RestartSec = "2";
        # /var/lib/losos (state.json + rebuild.log) — persisted via
        # impermanence's whole-/var bind mount. 0700: state.json records the
        # appliance's mode/settings and the rebuild log quotes the flake, and
        # systemd's default 0755 makes both world-readable.
        StateDirectory = "losos";
        StateDirectoryMode = "0700";

        # The daemon needs real root: it rewrites /etc/nixos and drives
        # `nixos-rebuild`. So this is not a sandbox, it is damage limitation on
        # the one process that terminates untrusted HTTP. The rebuild itself
        # runs in a `systemd-run` transient unit, which PID 1 starts outside
        # every namespace below — so none of these restrict it.
        #
        # ProtectHome is the load-bearing one: /home/notshared/data and
        # /home/shared/data are the two isolated data domains, and a compromised
        # request handler has no business reading either.
        ProtectHome = true;
        NoNewPrivileges = true;
        PrivateTmp = true;
        ProtectKernelTunables = true;
        LockPersonality = true;
      };
    };
  };
}
