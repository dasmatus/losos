# The OCI image the LOCAL k3s cluster runs.
#
# One of them now: the sandbox (pause) image every pod on the local cluster is
# built around. The box's own apps used to run here as pods too; LosOS Cloud
# (modules/cloud.nix) and LosOS Git (modules/git.nix) are plain services now,
# and the cluster runs only the apps of the box's shared side. The image is
# preloaded through the cluster's `images` option, which symlinks the tarball
# into /var/lib/rancher/k3s/agent/images for the agent to import at start, so
# nothing is ever fetched from a container registry. The name is under
# `losos.local/` for the same reason: a pull fails on a name that cannot
# resolve instead of quietly reaching Docker Hub.
#
# The image is NOT built on the appliance. It comes from this project's own
# Nix binary cache, and a cache that is unreachable at 03:00 means the rebuild
# fails and the previous generation keeps running.
#
# The tag is left to dockerTools, which derives it from the output hash: a
# hand-written tag would stay put across a content change, and the kubelet,
# told never to pull, would keep running the previous image forever.
{ pkgs }:

let
  # The sandbox image every pod on the local cluster is built around: PID 1 of
  # the pod's namespaces, whose whole job is to hold them open and reap
  # whatever gets reparented onto it. tini + `sleep inf` is what nixpkgs' own
  # nixos/tests/rancher/single-node.nix passes to --pause-image, and this is
  # the same use.
  pauseEnv = pkgs.buildEnv {
    name = "losos-image-pause-root";
    paths = [
      pkgs.tini
      pkgs.coreutils
    ];
  };
in
{
  losos-image-pause = pkgs.dockerTools.buildLayeredImage {
    name = "losos.local/pause";
    compressor = "zstd";
    contents = pauseEnv;
    config.Entrypoint = [
      "/bin/tini"
      "-g"
      "--"
      "/bin/sleep"
      "inf"
    ];
  };
}
