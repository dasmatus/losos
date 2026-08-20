{ mkDerivation, aeson, base, bytestring, dbus, directory, filepath
, http-types, lib, mtl, optparse-applicative, process, tasty
, tasty-hunit, text, time, unix, wai, warp
}:
mkDerivation {
  pname = "losos-ctl";
  version = "0.1.0";
  src = ./.;
  isLibrary = true;
  isExecutable = true;
  # No profiling consumer exists, and the profiling pass doubles GHC's work —
  # it OOM-killed the ISO build on codeberg-medium CI runners (exit 137).
  enableLibraryProfiling = false;
  libraryHaskellDepends = [
    aeson base bytestring dbus directory filepath http-types mtl
    optparse-applicative process text time unix wai warp
  ];
  executableHaskellDepends = [
    aeson base bytestring dbus optparse-applicative text
  ];
  testHaskellDepends = [
    aeson base bytestring tasty tasty-hunit text
  ];
  description = "losos appliance control — lososd daemon + losos-ctl facade";
  license = lib.meta.getLicenseFromSpdxId "AGPL-3.0-or-later";
}
