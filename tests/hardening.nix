# nixos-test-vms config for the hardening layer (modules/hardening.nix).
#
# `losos.hardening.*` exists because the thing everyone reaches for first is
# gone: `nixos/modules/profiles/hardened.nix` was removed in 26.05, and in the
# nixpkgs this flake tracks `linux_hardened` is literally
# `throw "linux_hardened has been removed due to lack of maintenance"`.
# Upstream's stated reason for dropping the profile — that it "lacks a
# consistent and transparent baseline" and was "more of a 'grab bag' of
# settings than a cohesive security policy" — is also the reason this module is
# staged behind separate flags rather than one boolean.
#
# The appliance runs two Kubernetes instances (modules/cluster.nix): a local
# k3s server for its own Nextcloud and Forgejo, and an rke2 agent that joins
# the edge for Longhorn and mesh compute. That makes this test's second job at
# least as important as its first. Nearly every published hardening baseline
# was written for a machine that does not run a container runtime, and roughly
# half of what they recommend breaks one:
#
#   * blacklisting `overlay` breaks containerd's snapshotter,
#   * blacklisting `br_netfilter`, `vxlan` or the `nf_conntrack` family breaks
#     canal (flannel + calico) on the mesh cluster,
#   * blacklisting `iscsi_tcp` or `dm_crypt` breaks Longhorn's volume attach,
#   * and **strict `rp_filter` breaks Calico outright** — which is the second
#     independent reason this module sets 2 rather than 1.
#
# So there is a whole subtest asserting the things the hardening does *not*
# block. A hardening layer that quietly stops pods scheduling on a box with no
# shell is worse than no hardening layer at all.
#
# Three nodes, because a hardening option has three ways to be wrong:
#
#   * `hardened` — the default. The baseline landed, the appliance still
#     serves, mDNS still resolves, and Kubernetes' kernel surface is intact.
#   * `full` — every opt-in flag at once (AppArmor, graphene-hardened malloc,
#     nosmt, usbguard). They are opt-in precisely because they are the ones
#     that can break something, so "all of them together and the box still
#     works" is the claim that lets anyone turn them on.
#   * `plain` — `losos.hardening.enable = false`. An option that behaves the
#     same whether it is on or off is an easy bug to ship and a hard one to
#     notice.
#
# Deliberately does NOT boot k3s or rke2. Pulling the ~2.6 GiB Nextcloud image
# into a test VM is a multi-hour build that exercises no line of this module —
# the same reasoning tests/admin-vm.nix gives. What this file asserts is that
# the *kernel surface* those two need is untouched; that they then come up is
# tests/cluster-vm.nix's job.
{ pkgs }:

let
  lososPkgs = import ../flake/packages.nix { inherit pkgs; };

  # Everything all three nodes share. Imports the real modules — the point is
  # to harden the actual appliance config, not a stand-in.
  common =
    { lib, ... }:
    {
      imports = [
        ../modules/options.nix
        ../modules/configuration.nix
        ../modules/hardening.nix
        ../modules/daemon.nix
      ];

      losos.backend.package = lososPkgs.losos-ctl;

      # /dev/dri does not exist in the VM and the graphics stack is a large
      # closure for nothing here.
      losos.gpu.enable = false;

      # NetworkManager fights the test framework's own interface setup, and
      # nothing here is testing it. Avahi stays on — it is the subject of the
      # "can we still reach the box" assertion.
      networking.networkmanager.enable = lib.mkForce false;

      # nginx is the front door and one of the units the baseline sandboxes, so
      # it has to be here to prove the sandboxing did not break it.
      services.nginx = {
        enable = true;
        virtualHosts."losos-test".locations."/".return = "200 'ok'";
      };

      virtualisation = {
        memorySize = 1536;
        cores = 2;
      };
    };
in

pkgs.testers.nixosTest {
  name = "losos-hardening";

  nodes = {
    # Defaults: losos.hardening.enable is true, every opt-in flag is false.
    hardened =
      { ... }:
      {
        imports = [ common ];
        losos.hostName = "hardened";
      };

    # Every opt-in flag on at once.
    full =
      { ... }:
      {
        imports = [ common ];
        losos.hostName = "full";
        losos.hardening = {
          apparmor = true;
          malloc = true;
          nosmt = true;
          usbguard = true;
        };
      };

    # The off state.
    plain =
      { ... }:
      {
        imports = [ common ];
        losos.hostName = "plain";
        losos.hardening.enable = false;
      };
  };

  testScript = ''
    start_all()

    for m in (hardened, full, plain):
        m.wait_for_unit("multi-user.target")

    def sysctl(machine, key):
        """Read a sysctl from /proc — the value in force, rather than the value
        some file asked for."""
        path = "/proc/sys/" + key.replace(".", "/")
        return machine.succeed(f"cat {path}").strip()

    with subtest("the baseline sysctls are in force"):
        # Kernel address and log disclosure.
        assert sysctl(hardened, "kernel.kptr_restrict") == "2"
        assert sysctl(hardened, "kernel.dmesg_restrict") == "1"
        assert sysctl(hardened, "kernel.perf_event_paranoid") == "3"
        # ptrace_scope 2 (admin-only), not 3 (nobody, and not reversible
        # without a reboot) — the appliance's own recovery paths need root to
        # still be able to attach, and `crictl exec` into a pod is the only way
        # anyone debugs this box.
        assert sysctl(hardened, "kernel.yama.ptrace_scope") == "2"
        # security.protectKernelImage sets this one, as a boolean.
        assert sysctl(hardened, "kernel.kexec_load_disabled") == "1"
        # Link and FIFO/regular-file confusion in world-writable directories.
        assert sysctl(hardened, "fs.protected_symlinks") == "1"
        assert sysctl(hardened, "fs.protected_hardlinks") == "1"
        assert sysctl(hardened, "fs.protected_fifos") == "2"
        assert sysctl(hardened, "fs.protected_regular") == "2"
        assert sysctl(hardened, "fs.suid_dumpable") == "0"
        # Userspace-triggered page-fault handling; a recurring LPE primitive.
        assert sysctl(hardened, "vm.unprivileged_userfaultfd") == "0"
        assert sysctl(hardened, "dev.tty.ldisc_autoload") == "0"
        assert sysctl(hardened, "kernel.randomize_va_space") == "2"
        # Network stack.
        assert sysctl(hardened, "net.ipv4.tcp_syncookies") == "1"
        assert sysctl(hardened, "net.ipv4.conf.all.accept_source_route") == "0"
        assert sysctl(hardened, "net.ipv4.conf.all.accept_redirects") == "0"
        assert sysctl(hardened, "net.ipv6.conf.all.accept_redirects") == "0"

    with subtest("unprivileged eBPF is off but root eBPF is untouched"):
        # kubelet, containerd and calico load eBPF as root. This sysctl gates
        # bpf() for UNPRIVILEGED callers only, so it costs Kubernetes nothing
        # while removing a large LPE surface from everything else on the box.
        # Spelled out because "disable eBPF" on a Kubernetes node reads like a
        # mistake unless you know which half is disabled.
        assert sysctl(hardened, "kernel.unprivileged_bpf_disabled") == "1"
        assert sysctl(hardened, "net.core.bpf_jit_harden") == "2"

    # ── The compatibility half ────────────────────────────────────────────
    with subtest("the hardening does not block what Kubernetes needs"):
        # Every one of these is on somebody's hardening checklist, and every
        # one of them would break this appliance.
        for mod in (
            "overlay",        # containerd's snapshotter
            "br_netfilter",   # canal/flannel on the mesh cluster
            "bridge",
            "veth",
            "vxlan",          # flannel's backend
            "nf_conntrack",   # kube-proxy's iptables dataplane
            "dm_crypt",       # Longhorn volume encryption (cluster.nix names it)
            "iscsi_tcp",      # Longhorn volume attach (cluster.nix names it)
        ):
            hardened.succeed(f"modprobe {mod}")

        # Runtime-settable, because k3s and rke2 set these themselves at start.
        # The hardening must not pin them to 0 — a sysctl the module declares
        # is re-applied by systemd-sysctl and would fight the runtime.
        hardened.succeed("sysctl -w net.ipv4.ip_forward=1")
        hardened.succeed("sysctl -w net.bridge.bridge-nf-call-iptables=1")

    with subtest("rp_filter is loose, for two independent reasons"):
        # 1. Strict reverse-path filtering drops multicast replies on a
        #    multi-homed machine, and mDNS is the ONLY way to reach an
        #    appliance with no SSH and no shell login.
        # 2. Calico — half of the canal CNI the mesh cluster runs — does not
        #    work under strict rp_filter.
        # If someone "fixes" this to 1, this assertion is what tells them it
        # was a decision twice over.
        assert sysctl(hardened, "net.ipv4.conf.all.rp_filter") == "2"
        assert sysctl(hardened, "net.ipv4.conf.default.rp_filter") == "2"

    with subtest("user namespaces are left alone, and that is deliberate too"):
        # security.allowUserNamespaces = false sets user.max_user_namespaces
        # to 0. containerd and both kubelets need namespaces; zeroing this is
        # the standard way to make a Kubernetes node stop working.
        assert int(sysctl(hardened, "user.max_user_namespaces")) > 0

    with subtest("the KSPP kernel parameters are on the command line"):
        cmdline = hardened.succeed("cat /proc/cmdline")
        for param in (
            "slab_nomerge",
            "init_on_alloc=1",
            "init_on_free=1",
            "page_alloc.shuffle=1",
            "randomize_kstack_offset=on",
            "vsyscall=none",
            "debugfs=off",
            "pti=on",
            "nohibernate",
        ):
            assert param in cmdline, f"missing kernel param {param}: {cmdline!r}"

    with subtest("blacklisted modules cannot be loaded, not merely autoloaded"):
        # `boot.blacklistedKernelModules` alone only stops *automatic* loading
        # by alias; a plain `modprobe dccp` still succeeds. Blocking the
        # explicit path needs an `install <mod> /bin/false` line as well, which
        # is what KSPP and secureblue actually ship. Asserting on modprobe
        # rather than on the config file is the difference between testing the
        # intent and testing the effect.
        for mod in (
            "dccp", "sctp", "rds", "tipc",       # legacy/rare network protocols
            "cramfs", "freevxfs", "jffs2", "hfs", "hfsplus", "udf",  # filesystems
            "firewire-core", "vivid",            # DMA-capable and known-buggy
        ):
            hardened.fail(f"modprobe {mod}")

        # The loop above is weak on its own: `modprobe x` also fails when the
        # kernel simply has no module x, so it would pass on a kernel that
        # ships none of them. `udf` is paired against the unhardened node to
        # make one of them load-bearing — plain proves the module exists and is
        # loadable, hardened proves the block is what stopped it.
        plain.succeed("modprobe udf")
        hardened.fail("modprobe udf")

    with subtest("mount options are tightened where they can be"):
        tmp = hardened.succeed("findmnt -no OPTIONS /tmp")
        assert "nosuid" in tmp and "nodev" in tmp, f"/tmp options: {tmp!r}"
        # ...but NOT noexec, and this is load-bearing rather than an oversight.
        # Nix builds unpack and execute scripts under /tmp, and this box
        # rebuilds itself unattended every night (modules/updates.nix) with no
        # shell to fix a failed rebuild from. That self-repair path outranks
        # the hardening win.
        assert "noexec" not in tmp, "/tmp is noexec — this breaks nixos-rebuild"
        shm = hardened.succeed("findmnt -no OPTIONS /dev/shm")
        for opt in ("nosuid", "nodev", "noexec"):
            assert opt in shm, f"/dev/shm missing {opt}: {shm!r}"

    with subtest("the appliance still works"):
        # The whole point. Every assertion above is worthless if the box that
        # satisfies them cannot serve anything.
        hardened.wait_for_unit("lososd.service")
        hardened.wait_until_succeeds("losos-ctl state --json")
        state = hardened.succeed("losos-ctl state --json")
        assert '"mode"' in state, f"lososd unhealthy under hardening: {state!r}"
        hardened.wait_until_succeeds("curl -fsS localhost:8082/api/health")
        hardened.wait_for_unit("nginx.service")
        hardened.wait_for_open_port(80)
        assert "ok" in hardened.succeed("curl -fsS localhost:80/")

    with subtest("mDNS still resolves — the only way into this box"):
        hardened.wait_for_unit("avahi-daemon.service")
        hardened.wait_until_succeeds("avahi-resolve-host-name -4 hardened.local")

    with subtest("every opt-in flag can be on at once"):
        full.wait_for_unit("lososd.service")
        full.wait_until_succeeds("losos-ctl state --json")
        full.wait_for_unit("nginx.service")
        full.wait_until_succeeds("curl -fsS localhost:8082/api/health")
        full.wait_for_unit("avahi-daemon.service")
        full.wait_until_succeeds("avahi-resolve-host-name -4 full.local")
        # And the container runtime's kernel surface survives them too.
        full.succeed("modprobe overlay")
        full.succeed("modprobe br_netfilter")

    with subtest("graphene-hardened malloc is actually loaded"):
        # The path is `/etc/ld-nix.so.preload`, NOT the glibc-standard
        # `/etc/ld.so.preload`: nixpkgs' malloc module writes the former and
        # NixOS' patched loader is what reads it. Asserting the standard path
        # passes vacuously on a box where the allocator is off and fails on one
        # where it is on, which is exactly backwards.
        #
        # Worth knowing what this does NOT cover: k3s, rke2 and containerd are
        # static Go binaries and ignore preloading entirely, and a pod has its
        # own rootfs and therefore its own (absent) preload file. So this
        # hardens the host's dynamically linked processes — nginx, avahi,
        # lososd — and nothing inside Kubernetes.
        preload = full.succeed("cat /etc/ld-nix.so.preload")
        assert "hardened_malloc" in preload, f"ld-nix.so.preload: {preload!r}"

    with subtest("AppArmor is enforcing"):
        full.succeed("test -d /sys/kernel/security/apparmor")
        assert full.succeed("aa-enabled").strip() == "Yes"

    with subtest("SMT is off"):
        assert "nosmt" in full.succeed("cat /proc/cmdline")
        assert full.succeed("cat /sys/devices/system/cpu/smt/control").strip() in (
            "off", "forceoff", "notsupported",
        )

    with subtest("usbguard runs"):
        full.wait_for_unit("usbguard.service")

    with subtest("the off switch is a real off switch"):
        # If these matched the hardened node, losos.hardening.enable would be
        # doing nothing and every assertion above would be measuring the stock
        # NixOS defaults.
        assert sysctl(plain, "kernel.kptr_restrict") != "2"
        assert sysctl(plain, "kernel.perf_event_paranoid") != "3"
        assert "slab_nomerge" not in plain.succeed("cat /proc/cmdline")
        assert "install udf" not in plain.succeed("modprobe --showconfig")
        plain.fail("test -e /etc/ld-nix.so.preload")
  '';
}
