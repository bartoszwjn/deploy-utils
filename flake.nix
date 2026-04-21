{
  description = "Additional utilities for working with deploy-rs";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    systems.url = "github:nix-systems/default";
    crane.url = "github:ipetkov/crane";
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
        craneLib = import inputs.crane { inherit pkgs; };

        treefmtEval = (import inputs.treefmt-nix).evalModule pkgs {
          projectRootFile = "flake.nix";
          settings.on-unmatched = "info";
          programs.nixfmt.enable = true;
          programs.keep-sorted.enable = true;
          programs.rustfmt.enable = true;
        };

        deploy-utils = import ./package.nix { inherit lib craneLib; };
      in
      {
        packages = {
          inherit deploy-utils;
          default = deploy-utils;
        };

        checks = {
          inherit deploy-utils;
          treefmt-check = treefmtEval.config.build.check (
            lib.fileset.toSource {
              root = ./.;
              fileset = lib.fileset.gitTracked ./.;
            }
          );
        }
        // lib.mapAttrs' (testName: lib.nameValuePair "deploy-utils-${testName}") deploy-utils.tests;

        devShells.default = craneLib.devShell {
          inputsFrom = [
            deploy-utils
            treefmtEval.config.build.devShell
          ];
        };

        formatter = treefmtEval.config.build.wrapper;
      }
    );
}
