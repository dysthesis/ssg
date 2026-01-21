# This is the entrypoint of the flake. This file should define constants that
# are shared across various modules.
{
  description = "Static site generator";

  outputs = inputs @ {
    flake-parts,
    crane,
    advisory-db,
    ...
  }:
    flake-parts.lib.mkFlake {inherit inputs;} {
      perSystem = {system, ...}: let
        pkgs = import inputs.nixpkgs {
          inherit system;
          overlays = [
            inputs.rust-overlay.overlays.default
          ];
        };

        craneLib = crane.mkLib pkgs;

        # Common arguments can be set here to avoid repeating them later
        # NOTE: changes here will rebuild all dependency crates
        src = ./.;
        commonArgs = {
          inherit src;
          strictDeps = true;

          buildInputs = pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.libiconv
          ];

          nativeBuildInputs = [
            pkgs.pkg-config
          ];
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
      in {
        _module.args = {
          inherit
            craneLib
            cargoArtifacts
            commonArgs
            src
            advisory-db
            pkgs
            ;
        };
      };
      imports = [
        ./nix/shell
        ./nix/pkgs
        ./nix/checks
        ./nix/formatter
        inputs.treefmt-nix.flakeModule
      ];

      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
        "x86_64-darwin"
      ];
    };

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs";
    flake-parts.url = "github:hercules-ci/flake-parts";

    # General rust stuff
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };

    rustowl.url = "github:nix-community/rustowl-flake";

    # Formatting
    treefmt-nix.url = "github:numtide/treefmt-nix";
  };
}
