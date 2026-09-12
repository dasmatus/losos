# System hardening, staged behind losos.hardening.* (declared in options.nix).
#
# Why this file exists rather than one line importing a profile: there is no
# profile left to import. `nixos/modules/profiles/hardened.nix` was removed in
# NixOS 26.05, and in the nixpkgs this flake tracks `linux_hardened` is
# `throw "linux_hardened has been removed due to lack of maintenance"`.
# Upstream's reasoning for dropping the profile is worth repeating because it
# shapes this module: it "lacks a consistent and transparent baseline", "may
# introduce unexpected breakage or degrade performance without clear benefit",
# and was "often more of a 'grab bag' of settings than a cohesive security
# policy".
#
# So the settings are split by what they cost:
#
#   * `losos.hardening.enable` (on by default) is the layer that costs nothing
#     this appliance was using. Nothing here can take the box off the network,
#     stop a service starting, or stop a pod scheduling.
#   * the four opt-in flags are the ones that can. They get their own switches
#     so that one breaking does not mean reverting the lot.
#
# ── The constraint that shapes everything below ──────────────────────────────
#
# This box runs two Kubernetes instances (modules/cluster.nix): a local k3s
# server for its own Nextcloud and Forgejo, and an rke2 agent that joins the
# edge for Longhorn and mesh compute. Published hardening baselines are written
# for machines that do not run a container runtime, and a large fraction of
# what they recommend breaks one. Every such setting is called out by name here
# and asserted absent in tests/hardening.nix, so none of them can be
# reintroduced later as an obvious improvement:
#
#   * `overlay` must load, or containerd has no snapshotter.
#   * `br_netfilter`, `bridge`, `veth`, `vxlan` and the `nf_conntrack` family
#     must load, or canal (flannel + calico) cannot build the mesh overlay.
#   * `iscsi_tcp` and `dm_crypt` must load, or Longhorn cannot attach a volume.
#     modules/cluster.nix names both in `boot.kernelModules`.
#   * `net.ipv4.ip_forward` and `net.bridge.bridge-nf-call-iptables` must stay
#     writable at runtime, because k3s and rke2 set them themselves at start.
#     A sysctl declared here is re-applied by systemd-sysctl and would fight
#     them.
#   * `rp_filter` must not be strict — Calico does not work under it.
#   * user namespaces must stay available, or neither kubelet starts.
#
# Every claim in this file is asserted in tests/hardening.nix against three
# booted VMs, including the claims about what is deliberately *not* set. That
# test does not boot Kubernetes (pulling the ~2.6 GiB Nextcloud image into a
# test VM is a multi-hour build); it asserts the kernel surface Kubernetes
# needs is intact, and tests/cluster-vm.nix covers the rest.
#
# Scope note: imported by the `install` system only. The ISO must keep squashfs
# loadable and its module policy permissive, and hardening a live installer
# that is discarded at the end of the install protects nothing.
{
  pkgs,
  lib,
  config,
  ...
}:

let
  cfg = config.losos.hardening;

  # Kernel modules with no role on this appliance and a history of being the
  # cheap half of an exploit chain. Three groups: network protocols nothing
  # here speaks, filesystem drivers that exist to parse attacker-supplied
  # images, and hardware paths that hand out DMA.
  #
  # What is deliberately NOT here is the interesting part — see the header.
  # Nothing in the container, CNI, or Longhorn path appears below, and adding
  # one turns tests/hardening.nix red rather than breaking a box in a cupboard.
  blockedModules = [
    # Legacy and rare network protocols. An unprivileged socket() call is
    # enough to autoload most of these, which is what makes them worth blocking
    # rather than merely not using. SCTP is included: Kubernetes can expose
    # SCTP Services, but nothing this appliance runs does, and the module has
    # its own CVE history.
    "dccp"
    "sctp"
    "rds"
    "tipc"
    "n-hdlc"
    "ax25"
    "netrom"
    "x25"
    "rose"
    "decnet"
    "econet"
    "af_802154"
    "ipx"
    "appletalk"
    "psnap"
    "p8023"
    "p8022"
    "can"
    "atm"
    # Filesystem drivers that parse untrusted on-disk structures in the kernel.
    # This box mounts ext4 on /persist, tmpfs everywhere else, overlayfs under
    # containerd and NFS for Longhorn's RWX volumes — none of which are here.
    "cramfs"
    "freevxfs"
    "jffs2"
    "hfs"
    "hfsplus"
    "udf"
    "ksmbd"
    "gfs2"
    # Hardware with direct memory access, or a bad track record.
    "firewire-core"
    "thunderbolt"
    "vivid"
  ];

  # `boot.blacklistedKernelModules` on its own only prevents *automatic*
  # loading by alias — a plain `modprobe dccp` still succeeds. Blocking the
  # explicit path needs an `install` line, which is what KSPP and secureblue
  # actually ship. A store path rather than /bin/false, which does not exist on
  # NixOS.
  modprobeDenials = lib.concatMapStringsSep "\n" (
    m: "install ${m} ${pkgs.coreutils}/bin/false"
  ) blockedModules;

  # Applied to the host services that terminate untrusted input but must keep
  # writing their own state. Deliberately no ProtectSystem: lososd rewrites
  # /etc/nixos and nginx writes /var/log/nginx, and a sandbox that stops the
  # appliance repairing itself is a worse outcome than the one it prevents.
  #
  # Deliberately NOT applied to k3s, rke2, containerd or anything they start.
  # Those units exist to create namespaces, mount filesystems, load modules and
  # write cgroups on behalf of arbitrary workloads; sandboxing them means
  # picking a fight you lose on a box with no shell, and upstream already ships
  # the unit definitions it intends.
  commonSandbox = {
    NoNewPrivileges = true;
    ProtectClock = true;
    ProtectHostname = true;
    ProtectKernelLogs = true;
    ProtectKernelModules = true;
    ProtectKernelTunables = true;
    RestrictRealtime = true;
    RestrictSUIDSGID = true;
    LockPersonality = true;
    SystemCallArchitectures = "native";
  };
in
{
  config = lib.mkMerge [

    # ── Baseline ────────────────────────────────────────────────────────────
    (lib.mkIf cfg.enable {

      # KSPP kernel command line. Allocator hardening first: slab_nomerge stops
      # same-size caches sharing, and init_on_alloc/init_on_free zero pages on
      # both ends, which closes the whole use-after-free-reads-secrets class.
      # init_on_free in particular is a measurable throughput cost — it is here
      # anyway because the alternative is a box that leaks the LUKS and fscrypt
      # key material it just finished using.
      boot.kernelParams = [
        "slab_nomerge"
        "init_on_alloc=1"
        "init_on_free=1"
        "page_alloc.shuffle=1"
        "randomize_kstack_offset=on"
        # The legacy vsyscall page is a fixed-address executable mapping, i.e.
        # a permanent ROP gadget source. Nothing built this decade needs it.
        "vsyscall=none"
        "debugfs=off"

        # CPU side channels. `mitigations=auto,nosmt` is deliberately NOT
        # used: the `nosmt` half halves the core count of a mini-PC that
        # transcodes video for Nextcloud, and it has its own opt-in flag
        # (losos.hardening.nosmt) so that trade stays a decision rather than a
        # side effect of this line. Everything below is the half that costs
        # nothing a default kernel was not already paying.
        #
        # Each of these only *forces* a mitigation the kernel already applies
        # by default on affected hardware; on unaffected CPUs they are no-ops.
        # They are here because "default" is a moving target across kernel
        # versions, and this box updates itself unattended at 03:00.
        "spectre_v2=on"
        "spec_store_bypass_disable=on"
        "l1tf=flush"
        "mds=full"
        "tsx=off"
        "tsx_async_abort=full"
        # Transparent huge pages were the mitigation's own escape hatch; this
        # is the KSPP-recommended setting rather than the kernel default.
        "kvm.nx_huge_pages=force"

        # DMA. A device plugged into a Thunderbolt or ExpressCard port can
        # read main memory before the IOMMU is configured unless the firmware
        # is told to shut early PCI DMA down; that is a live risk on an
        # appliance sitting on a shelf. Complements — does not replace — the
        # firewire-core/thunderbolt entries in the module blacklist below,
        # which only stop the drivers, not the DMA.
        "efi=disable_early_pci_dma"
        "intel_iommu=on"
        "amd_iommu=force_isolation"
        "iommu.passthrough=0"
        "iommu.strict=1"
      ];

      # Sets kernel.kexec_load_disabled and nohibernate. Both matter here:
      # kexec replaces the running kernel without touching the bootloader (so
      # Secure Boot never sees it), and hibernation writes RAM — with the
      # /persist LUKS key and the fscrypt protector in it — to disk.
      security.protectKernelImage = true;
      security.forcePageTableIsolation = true;

      boot.kernel.sysctl = {
        # Kernel address and log disclosure. kptr_restrict=2 hides pointers
        # even from root, because on this box "root" is what an attacker is
        # trying to become, not a person.
        "kernel.kptr_restrict" = 2;
        "kernel.dmesg_restrict" = 1;
        "kernel.printk" = "3 3 3 3";
        "kernel.perf_event_paranoid" = 3;
        # 2 = admin-only, not 3 = nobody. 3 cannot be undone without a reboot,
        # and `crictl exec` into a pod is the only way anyone ever debugs this
        # box.
        "kernel.yama.ptrace_scope" = 2;
        # SysRq is a physical-console privilege escalation on a machine whose
        # threat model includes someone carrying it away.
        "kernel.sysrq" = 0;

        # ── eBPF: the unprivileged half only ──────────────────────────────
        # This reads like a mistake on a Kubernetes node, so: kubelet,
        # containerd and calico load eBPF as root, and this sysctl gates bpf()
        # for *unprivileged* callers. It removes a large local-privilege-
        # escalation surface from everything else on the box and costs
        # Kubernetes nothing.
        "kernel.unprivileged_bpf_disabled" = 1;
        "net.core.bpf_jit_harden" = 2;

        # Recurring local-privilege-escalation primitives.
        "vm.unprivileged_userfaultfd" = 0;
        "dev.tty.ldisc_autoload" = 0;

        # Link, FIFO and regular-file confusion in world-writable directories.
        "fs.protected_symlinks" = 1;
        "fs.protected_hardlinks" = 1;
        "fs.protected_fifos" = 2;
        "fs.protected_regular" = 2;
        "fs.suid_dumpable" = 0;

        # ASLR at full strength, and a core dump that goes nowhere: a crash
        # dump of lososd contains the admin token.
        "kernel.randomize_va_space" = 2;
        "kernel.core_pattern" = "|${pkgs.coreutils}/bin/false";

        # Network stack. Redirects and source routing let an on-path host steer
        # traffic, and the LAN is inside the trust boundary only in the sense
        # that docs/security-model.md admits it is.
        "net.ipv4.tcp_syncookies" = 1;
        "net.ipv4.conf.all.accept_source_route" = 0;
        "net.ipv4.conf.default.accept_source_route" = 0;
        "net.ipv6.conf.all.accept_source_route" = 0;
        "net.ipv6.conf.default.accept_source_route" = 0;
        "net.ipv4.conf.all.accept_redirects" = 0;
        "net.ipv4.conf.default.accept_redirects" = 0;
        "net.ipv4.conf.all.secure_redirects" = 0;
        "net.ipv4.conf.default.secure_redirects" = 0;
        "net.ipv6.conf.all.accept_redirects" = 0;
        "net.ipv6.conf.default.accept_redirects" = 0;
        "net.ipv4.icmp_echo_ignore_broadcasts" = 1;
        "net.ipv4.icmp_ignore_bogus_error_responses" = 1;

        # ── Reverse-path filtering: 2 (loose), not 1 (strict) ─────────────
        # Every hardening guide says 1, and 1 is wrong here for two independent
        # reasons.
        #
        # First, strict reverse-path filtering drops packets whose source would
        # not route back out the interface they arrived on, which is what
        # multicast replies look like on a multi-homed machine — and mDNS is
        # the ONLY way to reach an appliance with no SSH and no shell login.
        # modules/configuration.nix makes the same argument about not pinning
        # Avahi to a fixed interface list.
        #
        # Second, Calico — half of the canal CNI the mesh cluster runs — does
        # not work under strict rp_filter at all.
        #
        # A box you cannot reach is not a hardened box, and neither is one that
        # cannot schedule a pod. tests/hardening.nix asserts the 2.
        "net.ipv4.conf.all.rp_filter" = 2;
        "net.ipv4.conf.default.rp_filter" = 2;

        # ── Deliberately absent from this attrset ─────────────────────────
        # net.ipv4.ip_forward and net.bridge.bridge-nf-call-iptables are NOT
        # declared, in either direction. k3s and rke2 set them at start;
        # anything declared here is re-applied by systemd-sysctl and would
        # fight the runtime. Not setting them is the setting.
        #
        # net.ipv4.conf.all.send_redirects is also absent: a node running a pod
        # network legitimately forwards, and CIS' send_redirects=0 advice is
        # written for hosts that do not.
      };

      # ── Deliberately NOT set ────────────────────────────────────────────
      # `security.allowUserNamespaces = false` (user.max_user_namespaces = 0)
      # is on every hardening list and would stop both kubelets and containerd
      # dead. `security.lockKernelModules` is the same story twice over:
      # NetworkManager, the CNI plugins and Longhorn's iSCSI path all load
      # modules on demand long after boot. Both are asserted absent in
      # tests/hardening.nix.

      boot.blacklistedKernelModules = blockedModules;
      boot.extraModprobeConfig = modprobeDenials;

      # A separate tmpfs for /tmp, which upstream mounts nosuid,nodev and
      # deliberately not noexec — exactly the split this appliance wants.
      #
      # Do NOT reach for `fileSystems."/tmp"` instead. NixOS masks tmp.mount
      # unless this option is set, so the declaration silently produces no
      # mount at all and /tmp quietly inherits the root filesystem's options.
      # That is not a hypothetical: it is how the first version of this module
      # failed its own test.
      boot.tmp.useTmpfs = true;

      # /dev/shm has no such excuse — nothing legitimately executes from it.
      # `options` is a list and merges by concatenation, so this adds noexec to
      # the nosuid,nodev,strictatime,mode=1777 NixOS already sets.
      boot.specialFileSystems."/dev/shm".options = [ "noexec" ];

      # dbus-broker over the reference implementation: it enforces policy per
      # message in a much smaller amount of C, and lososd's whole privileged
      # surface is a system-bus interface (modules/daemon.nix).
      services.dbus.implementation = "broker";

      # Sudo has no role here — there is no interactive account to escalate
      # from — but leaving the general case open costs nothing to close.
      security.sudo.execWheelOnly = true;

      systemd.services = lib.mkMerge [
        {
          nginx.serviceConfig = commonSandbox // {
            # The front door terminates every request from the LAN. It serves
            # the store and /var; it has no business in either data home.
            ProtectHome = true;
          };

          avahi-daemon.serviceConfig = commonSandbox // {
            ProtectHome = true;
            # No RestrictAddressFamilies: Avahi needs AF_NETLINK to watch
            # interfaces come and go, and losing that is losing mDNS.
          };
        }

        # Only when there is a daemon to harden. Defining serviceConfig for a
        # unit modules/daemon.nix did not create would synthesise a broken one.
        (lib.mkIf (config.losos.backend.package != null) {
          lososd.serviceConfig = commonSandbox // {
            # On top of what modules/daemon.nix already sets (ProtectHome,
            # PrivateTmp, NoNewPrivileges, ProtectKernelTunables,
            # LockPersonality). This process terminates untrusted HTTP as root,
            # so it gets the strictest set that still lets it work.
            #
            # No ProtectSystem: lososd rewrites /etc/nixos, and the rebuild
            # runs in a systemd-run transient unit that PID 1 starts outside
            # every namespace here.
            ProtectProc = "invisible";
            ProcSubset = "pid";
            MemoryDenyWriteExecute = true;
            RestrictAddressFamilies = [
              "AF_UNIX"
              "AF_INET"
              "AF_INET6"
            ];
            SystemCallFilter = [ "@system-service" ];
          };
        })
      ];
    })

    # ── Opt-in: mandatory access control ───────────────────────────────────
    (lib.mkIf cfg.apparmor {
      security.apparmor = {
        enable = true;
        packages = [ pkgs.apparmor-profiles ];
        # killUnconfinedConfinables is deliberately left at its default. On a
        # box with no shell the recovery cost of killing the wrong daemon is a
        # reinstall.
      };
    })

    # ── Opt-in: hardened allocator ─────────────────────────────────────────
    (lib.mkIf cfg.malloc {
      # Works by LD_PRELOAD via /etc/ld-nix.so.preload — NixOS' patched loader
      # reads that path, not the glibc-standard /etc/ld.so.preload, which never
      # exists here. Asserting the standard path passes while the allocator is
      # off and fails once it is on; tests/hardening.nix uses the real one.
      # Worth being precise about
      # the blast radius: k3s, rke2 and containerd are static Go binaries and
      # ignore LD_PRELOAD entirely, and a pod has its own rootfs and therefore
      # its own (absent) /etc/ld.so.preload. So this hardens the host's
      # dynamically linked processes — nginx, avahi, lososd — and nothing
      # inside Kubernetes. That is a smaller win than it first appears, and the
      # reason it is opt-in rather than on.
      environment.memoryAllocator.provider = "graphene-hardened";
    })

    # ── Opt-in: no simultaneous multithreading ─────────────────────────────
    (lib.mkIf cfg.nosmt {
      security.allowSimultaneousMultithreading = false;
    })

    # ── Opt-in: USB device policy ──────────────────────────────────────────
    (lib.mkIf cfg.usbguard {
      services.usbguard = {
        enable = true;
        # Whatever was plugged in at boot keeps working; nothing new is
        # accepted. That is the right shape for a machine meant to sit in a
        # cupboard, and the reason it is not on by default is the case where it
        # is not sitting in a cupboard yet.
        implicitPolicyTarget = "block";
        presentDevicePolicy = "keep";
      };
    })
  ];
}
