{
  lib,
  craneLib,
}:

let
  fs = lib.fileset;

  src = fs.toSource {
    root = ./.;
    fileset = fs.unions [
      (fs.fromSource (craneLib.cleanCargoSource ./.))
      (fs.fileFilter (file: file.hasExt "nix") ./src)
      (fs.fileFilter (file: file.hasExt "sh") ./src)
    ];
  };
  cargoToml = lib.importTOML ./Cargo.toml;

  baseArgs = {
    inherit src;
    strictDeps = true;
  };
  commonArgs = baseArgs // {
    inherit cargoArtifacts;
  };

  cargoArtifacts = craneLib.buildDepsOnly baseArgs;

  clippy = craneLib.cargoClippy (
    commonArgs // { cargoClippyExtraArgs = "--all-targets -- --deny warnings"; }
  );

  deny = craneLib.cargoDeny {
    inherit (baseArgs) src strictDeps;
    cargoDenyChecks = "bans licenses sources";
  };

  doc = craneLib.cargoDoc (commonArgs // { env.RUSTDOCFLAGS = "--deny warnings"; });

  fmt = craneLib.cargoFmt { inherit (baseArgs) src strictDeps; };

  test = craneLib.cargoTest commonArgs;
in

craneLib.buildPackage (
  commonArgs
  // {
    doCheck = false;

    meta = {
      description = cargoToml.package.description;
      homepage = cargoToml.package.homepage or cargoToml.package.repository;
      license =
        assert cargoToml.package.license == "MIT OR Apache-2.0";
        [
          lib.licenses.mit
          lib.licenses.asl20
        ];
      mainProgram = cargoToml.package.default-run;
    };

    passthru.tests = {
      inherit
        # keep-sorted start
        clippy
        deny
        doc
        fmt
        test
        # keep-sorted end
        ;
    };
  }
)
