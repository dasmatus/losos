# Impermanence: /persist (encrypted, see disko.nix) holds all durable state;
# the root is tmpfs and is rebuilt every boot. Directories listed here are
# bind-mounted from /persist back onto the volatile root.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  persist = config.environment.persistence."/persist";

  # Numeric ids for the names impermanence's entries carry. Names are what
  # the entries say; numbers are what the activation script below must use,
  # because the one place it runs where it matters has no name database
  # (see `lososPersistentSources`). Every user and group this appliance
  # persists for has a fixed id by design (modules/configuration.nix pins
  # them: the two data domains, the workload users), so a missing one is a
  # configuration error worth stopping on, not something to paper over with
  # a `|| true`.
  uidOf =
    name:
    let
      u = config.users.users.${name} or null;
    in
    if u == null || u.uid == null then
      throw "modules/impermanence.nix: persisted directory owner '${name}' has no fixed uid"
    else
      u.uid;
  gidOf =
    name:
    let
      g = config.users.groups.${name} or null;
    in
    if g == null || g.gid == null then
      throw "modules/impermanence.nix: persisted directory group '${name}' has no fixed gid"
    else
      g.gid;

  # Every proper ancestor of an absolute path, outermost first:
  # "/home/notshared/data" -> [ "/home" "/home/notshared" ].
  ancestors =
    path:
    let
      parts = lib.filter (p: p != "") (lib.splitString "/" path);
      prefixes = lib.genList (n: "/" + lib.concatStringsSep "/" (lib.take (n + 1) parts)) (
        lib.length parts - 1
      );
    in
    prefixes;

  # A user's home directory, when a persisted path's ancestor is one, is
  # created the way userborn will later shape it (its owner and its
  # `homeMode`) rather than as a root 0755 parent. Not for correctness of the
  # bind mount — impermanence resyncs the live home from the source on every
  # activation — but so the copy under /persist is never world-listable
  # between an install and the first boot, and never disagrees with what
  # tests/impermanence.nix asserts of the live one.
  homes = lib.listToAttrs (
    map (u: lib.nameValuePair u.home u) (
      lib.filter (u: u.isNormalUser && u.home != "/var/empty") (lib.attrValues config.users.users)
    )
  );
  parentSpec =
    path:
    if homes ? ${path} then
      {
        inherit path;
        uid = homes.${path}.uid;
        gid = gidOf homes.${path}.group;
        mode = homes.${path}.homeMode;
      }
    else
      {
        inherit path;
        uid = 0;
        gid = 0;
        mode = "0755";
      };
  leafSpec = d: {
    path = d.directory;
    uid = uidOf d.user;
    gid = gidOf d.group;
    inherit (d) mode;
  };

  # Parents first, then leaves, deduplicated: `install -d` needs the parent
  # to exist, and impermanence creates its own parents in this same order.
  specs = lib.unique (
    lib.concatMap (d: map parentSpec (ancestors d.directory)) persist.directories
    ++ map leafSpec persist.directories
  );

  ensureLine =
    s: "ensure ${lib.escapeShellArg "/persist${s.path}"} ${toString s.uid} ${toString s.gid} ${s.mode}";
in
{
  # Create the sources under /persist *before* impermanence looks for them,
  # by numeric id.
  #
  # impermanence's createPersistentStorageDirs creates a missing source with
  # `chown "$user:$group"` — by name. On a running box that is fine. Inside
  # `nixos-install`, which activates the new system in a chroot of the target
  # disk, it is not: this appliance builds its users with `services.userborn`
  # (modules/configuration.nix), which populates /etc/passwd from a systemd
  # unit at boot, so during that chroot activation there is no passwd file
  # yet and even `chown root:root` fails with "invalid user". Every fresh
  # install then printed, for each persisted directory, impermanence's
  # "Source directory does not exist" warning followed by
  # `chown: invalid user: 'root:root'` and
  # `Activation script snippet 'createPersistentStorageDirs' failed (1)`,
  # in the middle of the install's own output. The directories were still
  # created, by the `mkdir --mode` that precedes the failed chown, as
  # whatever the chroot's root was: the right owner only by accident, and a
  # script marked failed on every install. With the sources already there,
  # impermanence's script has nothing to create and nothing to chown by
  # name; what it still does
  # — `chown --reference`, `chmod --reference` from source to target — needs
  # no name lookup at all. `deps = [ ]` so this runs before `users`, which
  # under userborn is not an activation step anyway; the list of what to
  # create is read back from the same option impermanence reads, so the two
  # cannot drift.
  #
  # Only while "/persist" is still declared: the demo disk image
  # (flake/disk-images.nix) forces `environment.persistence` to `{ }`, and
  # impermanence then defines no createPersistentStorageDirs at all, so a
  # bare `deps` here would leave a snippet with no `text` and fail the
  # evaluation of losos-disk-qcow2.
  system.activationScripts = lib.mkIf (config.environment.persistence ? "/persist") {
    lososPersistentSources = {
      deps = [ ];
      text = ''
        ensure() {
          # path uid gid mode; only ever creates, never changes what exists,
          # exactly like impermanence's own creation step.
          if [ ! -d "$1" ]; then
            ${pkgs.coreutils}/bin/install -d -m "$4" -o "$2" -g "$3" "$1"
          fi
        }
        ${lib.concatMapStringsSep "\n" ensureLine specs}
      '';
    };
    createPersistentStorageDirs.deps = [ "lososPersistentSources" ];
  };

  environment.persistence."/persist" = {
    hideMounts = true;

    directories = [
      "/nix" # the nix store, so packages survive reboot
      "/var" # service state: postgres, redis, nextcloud, forgejo, k3s/rke2, /var/secrets, etc.
      "/etc/ssh" # host keys
      "/etc/keys" # LUKS keyfile source, so auto-upgrade rebuilds can re-bake the initrd
      "/etc/nixos" # the flake source, so auto-upgrade can rebuild without a remote
      # Kubernetes node identity. /var already covers the bulk of both
      # instances' state (/var/lib/rancher/{k3s,rke2}), but NOT this: the
      # agent generates /etc/rancher/node/password on its first join and the
      # server stores a hash of it keyed by node name. On a tmpfs root the
      # file is regenerated every boot, and the server then refuses the
      # rejoin ("Node password rejected"). Without this line the box silently
      # falls out of the mesh on the first reboot after enrolling — and there
      # is no shell to notice it from.
      "/etc/rancher"
      "/home/notshared/data"
      "/home/shared/data"
      # Service state lives under /var/lib/{nextcloud,forgejo} and is covered
      # by the whole-/var bind mount above; so are lososd's
      # /var/lib/losos/state.json, the container images and Kubernetes state
      # under /var/lib/rancher, and the secrets at /var/secrets (the admin
      # token lososd mints, the Nextcloud admin password generated by
      # modules/nextcloud-common.nix, the proxy and mesh tokens).
    ];

    files = [
      "/etc/machine-id"
    ];
  };
}
