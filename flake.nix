{
  description = "Utilities for working with deploy-rs";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    systems.url = "github:nix-systems/default";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    inputs:
    let
      inherit (inputs.nixpkgs) lib;

      inherit (builtins) head mapAttrs zipAttrsWith;
      eachSystem =
        systems: f:
        zipAttrsWith (k: zipAttrsWith (k: head)) (
          map (system: mapAttrs (k: v: { ${system} = v; }) (f system)) systems
        );
    in
    eachSystem (import inputs.systems) (
      system:
      let
        pkgs = inputs.nixpkgs.legacyPackages.${system};

        craneLib = (import inputs.crane { inherit pkgs; }).overrideToolchain (
          pkgs':
          let
            rust-bin = inputs.rust-overlay.lib.mkRustBin { } pkgs';
          in
          rust-bin.fromRustupToolchainFile ./rust-toolchain.toml
        );

        treefmtEval = (import inputs.treefmt-nix).evalModule pkgs {
          projectRootFile = "flake.nix";
          settings.on-unmatched = "info";
          programs.nixfmt.enable = true;
          programs.keep-sorted.enable = true;
          programs.rustfmt.enable = true;
        };

        packageName = "deploy-utils";
        package = pkgs.callPackage ./package.nix { inherit craneLib; };
      in
      {
        packages = {
          default = package;
          ${packageName} = package;
        };

        checks = {
          ${packageName} = package;
          treefmt-check = treefmtEval.config.build.check (
            lib.fileset.toSource {
              root = ./.;
              fileset = lib.fileset.gitTracked ./.;
            }
          );
        }
        // lib.mapAttrs' (testName: lib.nameValuePair "${packageName}-${testName}") package.tests;

        devShells.default = craneLib.devShell {
          inputsFrom = [
            package
            treefmtEval.config.build.devShell
          ];
          packages = [
            pkgs.cargo-deny
          ];
        };

        formatter = treefmtEval.config.build.wrapper;
      }
    );
}
