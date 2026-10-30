# Stalwart 1.0, the mail server behind LosOS Mail (modules/mail.nix).
#
# nixpkgs carries 0.15 (`stalwart`) and 0.16 (`stalwart_0_16`); 1.0, with
# its new database layout and the settings schema LosOS writes, is built
# here from the tagged source the same way nixpkgs builds 0.16, until
# nixpkgs has it. Only what a LosOS box uses is compiled in: RocksDB, no
# SQL, S3, Redis, NATS or Azure stores, and never the `enterprise` feature,
# so the binary is the AGPL edition and no licence key can turn on what
# LosOS does not ship. The release's test suites are kept private upstream;
# LosOS's own VM tests (nix/tests/mail*.nix) are the check.
#
# 1.0.1 rather than 1.0.0: 1.0.0 locks a git fork of opentelemetry-rust that
# upstream has since deleted, so its vendor step can no longer fetch, and
# 1.0.1 drops OpenTelemetry for exporters of its own. Its RocksDB is 11.8.1
# (a fix for spurious compare-and-swap failures), newer than nixpkgs', so
# librocksdb-sys builds the copy it carries instead of linking nixpkgs'.
#
# CI builds it in the `stalwart` job and pushes it to the LosOS cache, so a
# box substitutes it and never compiles it.
{
  lib,
  rustPlatform,
  fetchFromGitHub,
  pkg-config,
  protobuf,
  bzip2,
  openssl,
  zstd,
  stalwart_0_16,
}:

rustPlatform.buildRustPackage (finalAttrs: {
  pname = "losos-stalwart";
  version = "1.0.1";

  __structuredAttrs = true;

  src = fetchFromGitHub {
    owner = "stalwartlabs";
    repo = "stalwart";
    tag = "v${finalAttrs.version}";
    hash = "sha256-i2Ng4tyo4aIlOc99z6xq+hZbJ/1xpEPdkSfGtEOhD8s=";
  };

  cargoHash = "sha256-0xu4FVCv+UJehsDzJUVVvY/SC8c0PxXRaWLRinqMLgo=";

  env = {
    OPENSSL_NO_VENDOR = true;
    OPENSSL_DIR = lib.getDev openssl;
    OPENSSL_LIB_DIR = "${lib.getLib openssl}/lib";
    ZSTD_SYS_USE_PKG_CONFIG = true;
    # Upstream's release profile is fat LTO in one codegen unit: the last
    # crate alone ran past an hour and a half of one core without
    # finishing. Thin LTO over 16 units builds it in a fraction of that on
    # CI's four cores, for a slightly larger binary.
    CARGO_PROFILE_RELEASE_LTO = "thin";
    CARGO_PROFILE_RELEASE_CODEGEN_UNITS = "16";
  };

  depsBuildBuild = [
    pkg-config
    zstd
  ];
  nativeBuildInputs = [
    protobuf
    rustPlatform.bindgenHook
  ];
  buildInputs = [
    bzip2
    openssl
    zstd
  ];

  buildNoDefaultFeatures = true;
  buildFeatures = [ "rocks" ];
  cargoBuildFlags = [
    "-p"
    "stalwart"
  ];
  doCheck = false;

  passthru = {
    # The spam filter's rules, read from the store instead of GitHub.
    inherit (stalwart_0_16) spam-filter;
  };

  meta = {
    description = "Stalwart mail and collaboration server, as LosOS builds it";
    homepage = "https://github.com/stalwartlabs/stalwart";
    license = lib.licenses.agpl3Only;
    mainProgram = "stalwart";
    platforms = lib.platforms.linux;
  };
})
