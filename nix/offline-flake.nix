# What an offline install needs from the flake, without cloning or fetching.
#
# nix/tests/iso-install.nix and nix/tests/installer-edge.nix spell out why:
# a bare git mirror of this tree for LOSOS_FLAKE_URL=file://…, and every
# flake input's source under the NAR hash flake.lock pins, so evaluating the
# clone touches no network. losos-disk-iso puts both on the medium squashfs
# beside the install system's closure.
{
  self,
  pkgs,
}:

let
  inherit (pkgs) lib;

  walk = inputs: lib.concatMap (i: [ i.outPath ] ++ walk (i.inputs or { })) (lib.attrValues inputs);

  repo = pkgs.runCommand "losos-repo" { nativeBuildInputs = [ pkgs.git ]; } ''
    cp -r ${self} tree
    chmod -R u+w tree
    cd tree
    git init -q -b main
    # A commit starts `git maintenance run --auto --detach`, whose repack
    # renames packs under the clone below ("failed to copy file to …tmp_pack").
    git config maintenance.auto false
    git add -A
    git -c user.name=losos -c user.email=losos@local commit -qm tree
    git clone -q --bare . $out
  '';

  inputSources = lib.unique (walk (removeAttrs self.inputs [ "self" ]));
in
{
  inherit repo inputSources;

  flakeUrl = "file://${repo}";
}
