# The image Mesh CI's jobs run in unless they name one (modules/ci.nix): the
# tools a workflow expects of ubuntu-latest that matter most, small enough
# for a 4 GiB box. JavaScript actions run with the host's node through
# /nix/store, so node is not in here. The tag is dockerTools' hash of the
# contents, so a new build is a new name and is loaded again: the pm plugin
# (nix/ci-pm-plugin.nix) loads it before a job's containers start, unless
# Podman has it.
{ pkgs }:

pkgs.dockerTools.buildLayeredImage {
  name = "losos-ci-base";
  contents = with pkgs; [
    bashInteractive
    coreutils
    findutils
    diffutils
    gnugrep
    gnused
    gawk
    gnutar
    gzip
    xz
    zstd
    unzip
    which
    procps
    gnumake
    jq
    git
    curl
    cacert
    python3Minimal
    dockerTools.usrBinEnv
    dockerTools.binSh
    dockerTools.caCertificates
    dockerTools.fakeNss
  ];
  extraCommands = ''
    mkdir -p tmp
    chmod 1777 tmp
  '';
  config = {
    Env = [
      "PATH=/usr/local/bin:/usr/bin:/bin"
      "SSL_CERT_FILE=/etc/ssl/certs/ca-bundle.crt"
      "GIT_SSL_CAINFO=/etc/ssl/certs/ca-bundle.crt"
      "LANG=C.UTF-8"
    ];
    WorkingDir = "/github/workspace";
  };
}
