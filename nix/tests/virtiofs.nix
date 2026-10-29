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
# Every second this drops the guest's clean page cache and its unused
# dentries and inodes (an inode with cached pages is not let go), which
# sends virtiofsd a FORGET for each and lets it close their descriptors.
# Files the guest holds open stay. With KVM and a higher limit it
# costs a few lookups and changes nothing else.
#
# The node that installs imports it (`./virtiofs.nix` in its imports, or
# `defaults.imports` in a runNixOSTest test).
{
  systemd.services.virtiofs-forget = {
    description = "Keep the guest's cache of the host store small";
    wantedBy = [ "multi-user.target" ];
    script = ''
      # 3 is the page cache and the slab; 4 keeps the kernel from logging
      # each drop to the console.
      while sleep 1; do
        echo 7 >/proc/sys/vm/drop_caches
      done
    '';
  };
}
