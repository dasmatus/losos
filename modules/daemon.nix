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
# nothing but the daemon ever rewrites the flake or triggers a rebuild. This
# module replaces the old sudoers bridge (nextcloud user -> sudo losos-ctl).
{
  pkgs,
  lib,
  config,
  ...
}:

let
  enabled = config.losos.backend.package != null;
  pkg = config.losos.backend.package;

  # Which way `losos-ctl set-password` has to reach into Nextcloud.
  #
  # There is no default on the daemon's side and there must not be: lososd
  # treats an unset LOSOS_NEXTCLOUD_MODE as a hard error naming this module,
  # because guessing runs `crictl` against a box that has no containerd or
  # `nextcloud-occ` against a box whose closure never contained it — and both
  # failures surface as "the owner cannot set their password", on an appliance
  # with no shell to investigate from.
  ncMode = config.losos.nextcloud.mode;
  ncContainer = ncMode == "container";

  # modules/cluster.nix's `localCriSocket`, which is a `let` binding there and
  # so cannot be read from here. Repeated as a literal rather than promoted to
  # an option: it is containerd's compiled-in default and nothing in this repo
  # moves it. If it ever does move, it moves in both files or set-password
  # starts reporting that the pod is not running.
  #
  # The box's *own* k3s containerd. Deliberately not /run/k3s/…, which belongs
  # to the mesh rke2 agent — that one runs the edge's workloads and has never
  # heard of this box's Nextcloud.
  localCriSocket = "unix:///run/containerd/containerd.sock";

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
    # Nextcloud and Forgejo pods are hostNetwork and share its loopback, as
    # uid 1002 and 1003, and both are on the internet whenever the master
    # proxy is. Going around nginx skips the lanOnly guard entirely, and
    # lososd trusts X-Real-IP from any loopback peer because it assumes that
    # peer is nginx. So a compromised pod could claim an unclaimed box (the
    # claim route hands back the admin token, which is root), lock the owner
    # out by spending their address's failed-auth budget, and write false
    # addresses into the audit log.
    #
    # A unix socket with SO_PEERCRED would say the same thing in the daemon,
    # but the hardening profile gives lososd ProcSubset=pid and no AF_NETLINK,
    # so it cannot look a TCP peer's uid up itself, and moving nginx to a
    # socket changes every test that talks to :8082. An owner match on
    # OUTPUT says it at the kernel instead: a locally generated packet to the
    # port is rejected unless its socket belongs to root (the daemon's own
    # tooling, the VM tests) or nginx. tests/admin-vm.nix asserts both halves.
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

    systemd.services.lososd = {
      description = "losos appliance control daemon (D-Bus + admin HTTP API)";
      wantedBy = [ "multi-user.target" ];
      after = [ "dbus.service" ];
      requires = [ "dbus.service" ];
      environment = {
        LOSOS_ADMIN_PORT = toString config.losos.admin.apiPort;
        LOSOS_ADMIN_TOKEN_FILE = toString config.losos.admin.tokenFile;
        # See the `ncMode` binding above for why this is always set and never
        # defaulted daemon-side.
        LOSOS_NEXTCLOUD_MODE = ncMode;
      }
      # Only in container mode: in native mode lososd never looks at it, and
      # setting it anyway would suggest a CRI socket is consulted on a box that
      # runs no containers.
      // lib.optionalAttrs ncContainer {
        LOSOS_CRI_SOCKET = localCriSocket;
      }
      # Where `POST /api/sign-in` asks Nextcloud whether the owner's password
      # is right (backend/src/signin.rs): one loopback request to the OCS
      # route that only authenticates. Container mode reaches the pod's
      # Apache on the host's loopback at the subpath the front vhost
      # preserves; native mode goes through the box's own nginx to the
      # services.nextcloud vhost, which is why that one needs the Host
      # header spelled out. `localhost` is trusted by Nextcloud whatever
      # trusted_domains says, so the container probe can never be answered
      # "Untrusted domain" — but the native vhost is selected by name.
      // {
        LOSOS_NEXTCLOUD_LOGIN_URL =
          if ncContainer then
            "http://127.0.0.1:${toString config.losos.nextcloud.apachePort}/nextcloud/ocs/v2.php/cloud/user"
          else
            "http://127.0.0.1:80/ocs/v2.php/cloud/user";
        LOSOS_NEXTCLOUD_LOGIN_HOST = if ncContainer then "localhost" else config.losos.nextcloud.hostName;
      }
      # Where lososd looks for an edge proxy besides the LAN (backend/src/edge.rs):
      # the configured registrar, probed whether or not the master proxy is on,
      # because the question is "is there an edge anywhere this box could share
      # through", not "is this box enrolled". The LAN half needs no setting; it
      # is the `_losos-edge._tcp` browse over the Avahi the box already runs.
      // {
        LOSOS_EDGE_URL = config.losos.proxy.registrarUrl;
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
      };
      # What `losos-ctl grow` shells out to. A systemd unit's default PATH does
      # not include these, and the failure is the unhelpful kind: the grow
      # reports "No such file or directory" for lvextend after the admin has
      # already been told the disk is full.
      #
      # coreutils is for `df`, which is how the daemon measures whether the
      # filesystem actually got bigger rather than trusting an exit status.
      #
      # `set-password` adds one entry per mode, and only the one that mode
      # uses. `path` *replaces* PATH rather than extending it, so neither
      # binary is reachable by accident: without the line below, the command
      # fails at the exec with ENOENT after it has already staged a plaintext
      # password on disk.
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
        # What the edge scan browses the LAN with (backend/src/edge.rs):
        # `avahi-browse` for `_losos-edge._tcp`, under coreutils' `timeout`.
        # Missing, the scan reports the LAN as unsearched and only the
        # configured edge can open the sharing gate — a box on a LAN with an
        # edge and no internet would then never be allowed to share.
        pkgs.avahi
        # What every rebuild is started with. `systemd-run` resolves a relative
        # program name against the *caller's* PATH before it writes the
        # transient unit, and fails with "Failed to find executable
        # nixos-rebuild" when it cannot — so with this entry missing, every
        # `change`, `apply` and factory reset died at the launch with a 500
        # ("failed to start rebuild: systemd-run exited 1") and nothing was
        # ever rebuilt from the admin UI. tests/edge-lan.nix drives a mode
        # change through the daemon and would catch the line going again.
        config.system.build.nixos-rebuild
      ]
      # crictl, to find and exec into the Nextcloud workload container.
      ++ lib.optional ncContainer pkgs.cri-tools
      # The nixpkgs `nextcloud-occ` wrapper. Gated on native mode for more than
      # tidiness: in container mode services.nextcloud is never enabled, so
      # this attribute pulls a whole second Nextcloud closure into a system
      # that does not run one. `lib.optional false` does not force its
      # argument, so the attribute is not even evaluated there.
      ++ lib.optional (!ncContainer) config.services.nextcloud.occ;
      serviceConfig = {
        ExecStart = "${pkg}/bin/lososd";
        # `always`, not `on-failure`: a clean exit(0) — an unhandled shutdown
        # path, a bus disconnect the daemon treats as terminal — would
        # otherwise leave the box with no control plane and no way in (no SSH,
        # no shell logins). The rathole/registrar units use `always` for the
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
