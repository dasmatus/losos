# Log limits, on the box and on an edge.
#
# Nobody reads these logs on a box with no shell, and nobody would notice
# them fill the disk either: /var is on /persist, which is also /nix, so a
# log that grows without end ends as a 03:00 rebuild with no room to build.
# Every log either machine writes is capped here, or by the program that
# writes it:
#
#   journal         Most services log only to it. Capped at
#                   losos.logs.journalMaxUse and a month.
#   pod logs        Nextcloud and every app write to stdout, which
#                   the kubelet keeps under /var/log/pods: three files of
#                   10 MiB per container, in both clusters and on the edge.
#   nginx           /var/log/nginx/*.log, rotated by nixpkgs' own rule
#                   weekly; this adds a size bound for a busy week.
#   lososd          audit.log (one line per admin call) and rebuild.log
#                   (every Apply appends a whole nixos-rebuild) are lososd's
#                   own; rotated at 10 MiB, three kept.
#   nix             A compressed build log per derivation built on the
#                   machine, under /nix/var/log/nix/drvs. The garbage
#                   collector leaves them, so they are cleaned after 30 days.
#
# Capped by the program itself, left alone here: rke2 writes its kubelet and
# containerd logs to files that it rotates at 50 MB, libvirt's virtlogd
# rotates the Lab's guest logs, the edge's apiserver rotates its audit log
# (empty under rke2's default policy), and the edge's coin daemons are set in
# modules/edge.nix (bitcoind to the journal only, the Monero wallet service
# at 10 MiB a file, two old ones kept).
#
# Imported by both `install` and nixosModules.edge.
{ config, lib, ... }:

let
  cfg = config.losos.logs;

  # Per container, so a chatty pod cannot take more than this whatever it
  # prints. The kubelet's own default is five files of 10 MiB.
  podLogs = {
    containerLogMaxSize = "10Mi";
    containerLogMaxFiles = 3;
  };
in
{
  services.journald.settings.Journal = {
    SystemMaxUse = cfg.journalMaxUse;
    MaxRetentionSec = "1month";
  };

  # A non-empty extraKubeletConfig makes the generator pass a
  # KubeletConfiguration file, which k3s and rke2 lay over their own
  # defaults; neither sets these two. Harmless where a cluster is off.
  services.k3s.extraKubeletConfig = podLogs;
  services.rke2.extraKubeletConfig = podLogs;

  services.logrotate.settings = {
    # nixpkgs sets this rule's other values with mkDefault, so these win.
    nginx = lib.mkIf config.services.nginx.enable {
      maxsize = "20M";
      rotate = 8;
    };

    # lososd opens audit.log for each line it writes and creates it 0600
    # when it is missing, so a plain rename is enough and no line is lost.
    losos-audit = lib.mkIf (config.losos.backend.package != null) {
      files = "/var/lib/losos/audit.log";
      size = "10M";
      rotate = 3;
      compress = true;
      delaycompress = true;
    };

    # A rebuild unit holds rebuild.log open in append mode for as long as it
    # runs, so the file is copied and truncated in place rather than moved.
    losos-rebuild = lib.mkIf (config.losos.backend.package != null) {
      files = "/var/lib/losos/rebuild.log";
      size = "10M";
      rotate = 3;
      compress = true;
      copytruncate = true;
    };
  };

  systemd.tmpfiles.rules = [ "e /nix/var/log/nix/drvs - - - 30d" ];
}
