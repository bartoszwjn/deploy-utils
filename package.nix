{
  lib,
  craneLib,
}:

let
  src = craneLib.cleanCargoSource ./.;
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
        clippy
        doc
        fmt
        test
        ;
    };
  }
)
