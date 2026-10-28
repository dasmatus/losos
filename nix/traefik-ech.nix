# Traefik with Encrypted Client Hello, for an official edge
# (losos.edge.ech). No released Traefik decrypts ECH yet; this is the head
# of traefik/traefik#13758, which adds the TLS options `echKeys` and
# `echDecryptOnlyKeys` (RFC 9934 PEM files) on TCP and HTTP/3 and
# `traefik ech generate`. Built the way nixpkgs builds Traefik, with the
# Go that the branch asks for; drop this file for nixpkgs' own once a
# release carries the change.
#
# The branch's webui/static holds only a placeholder (the release tarball
# nixpkgs uses carries the built dashboard), so this Traefik serves no
# dashboard. The edge never enables it.
{
  lib,
  traefik,
  fetchFromGitHub,
  buildGo127Module,
}:

let
  rev = "0a5dbf5ac7e0846b33cc7f16dab082dd37d50f3e";
in
(traefik.override { buildGo126Module = buildGo127Module; }).overrideAttrs (
  finalAttrs: old: {
    version = "${old.version}-ech-${builtins.substring 0 7 rev}";
    src = fetchFromGitHub {
      owner = "traefik";
      repo = "traefik";
      inherit rev;
      hash = "sha256-csc1/5lsAiXodZaTy9nyJgbQw+4WD8ayaNA4kA/h3KU=";
    };
    vendorHash = "sha256-Cufp+YM9eC9YDhDbTSlxV75w0jbBoouoyEiGF2Fy+PA=";
    # nixosTests.traefik is nixpkgs' Traefik; this one is tested by
    # nix/tests/edge-ech.nix.
    passthru = old.passthru // {
      tests = { };
    };
  }
)
