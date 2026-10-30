# The host's /nix/store reaches a test VM through virtiofsd. Without
# CAP_DAC_READ_SEARCH, as in the Nix sandbox, virtiofsd cannot name inodes
# by file handle and holds an O_PATH descriptor for every inode the guest
# still caches. The store is shared with --cache=always, so the guest keeps
# what it looked up until memory runs short, and a test that installs the
# appliance reads its whole closure. Under a hard limit of 20000 open files
# (a cloud session's), virtiofsd ran out partway through nixos-install:
# "No more file descriptors available to the guest", then "Too many open
# files in system" in the guest.
#
# Two things keep it under. Whenever the guest holds 8000 more inodes
# than after its last drop, it drops its clean page cache and its unused
# dentries and inodes (an inode with cached pages is not let go), which
# sends virtiofsd a FORGET for each and lets it close their descriptors;
# files the guest holds open stay. And virtiofsd answers directory listings
# without READDIRPLUS, which would look up every entry it lists:
# evaluating nixpkgs in the guest lists all of pkgs/by-name, some 20000
# entries, within a second, and with READDIRPLUS each one held a descriptor
# before any drop could come. A plain listing still carries each entry's
# type, so readDir needs no lookup either. With KVM and a higher limit both
# cost a few lookups and change nothing else.
#
# It takes the test's host packages, which run virtiofsd, and is imported on
# the node that installs: `(import ./virtiofs.nix pkgs)` in its imports, or
# in `defaults.imports` in a runNixOSTest test.
hostPkgs:
{ lib, ... }:
let
  virtiofsd = hostPkgs.writeShellScriptBin "virtiofsd" ''
    exec ${lib.getExe hostPkgs.virtiofsd} --no-readdirplus "$@"
  '';
in
{
  # qemu-vm.nix starts `lib.getExe hostPkgs.virtiofsd`; nothing else in the
  # package set sees the wrapper.
  virtualisation.host.pkgs = lib.mkForce (hostPkgs // { inherit virtiofsd; });

  systemd.services.virtiofs-forget = {
    description = "Keep the guest's cache of the host store small";
    wantedBy = [ "multi-user.target" ];
    # drop_caches takes 1 to 4 and nothing else: 3 is the page cache and the
    # slab, and a 4 once keeps the kernel from logging every drop after it.
    # A drop costs the guest every lookup and read it had cached, so it
    # happens only once the guest holds 8000 more inodes than after the last
    # one (the first field of inode-nr counts every inode it holds).
    script = ''
      echo 4 >/proc/sys/vm/drop_caches
      read -r floor _ </proc/sys/fs/inode-nr
      while sleep 0.2; do
        read -r inodes _ </proc/sys/fs/inode-nr
        if [ "$inodes" -gt $((floor + 8000)) ]; then
          echo 3 >/proc/sys/vm/drop_caches
          read -r floor _ </proc/sys/fs/inode-nr
        fi
      done
    '';
  };
}
