{ lib, craneLib }:
let
  src = craneLib.cleanCargoSource ./.;
  cargoToml = lib.importTOML ./Cargo.toml;

  commonArgs = {
    inherit src;
    strictDeps = true;
  };

  cargoArtifacts = craneLib.buildDepsOnly commonArgs;

  package = craneLib.buildPackage (
    commonArgs
    // {
      inherit cargoArtifacts;
      doCheck = false;

      passthru.tests = {
        inherit
          clippy
          doc
          fmt
          test
          ;
      };

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
    }
  );

  clippy = craneLib.cargoClippy (
    commonArgs
    // {
      inherit cargoArtifacts;
      cargoClippyExtraArgs = "--all-targets -- --deny warnings";
    }
  );

  doc = craneLib.cargoDoc (
    commonArgs
    // {
      inherit cargoArtifacts;
      env.RUSTDOCFLAGS = "--deny warnings";
    }
  );

  fmt = craneLib.cargoFmt { inherit src; };

  test = craneLib.cargoTest (commonArgs // { inherit cargoArtifacts; });
in
package
