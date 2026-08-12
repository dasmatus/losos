{ mkDerivation, aeson, base, bytestring, dbus, directory, filepath
, lib, mtl, optparse-applicative, process, tasty, tasty-hunit, text
, time, unix
}:
mkDerivation {
  pname = "losos-ctl";
  version = "0.1.0";
  src = ./.;
  isLibrary = true;
  isExecutable = true;
  libraryHaskellDepends = [
    aeson base bytestring dbus directory filepath mtl
    optparse-applicative process text time unix
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
