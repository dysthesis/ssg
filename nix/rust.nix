{inputs, ...}: {
  perSystem = {pkgs, ...}: let
    toolchainFor = rustPkgs: let
      rust-bin = inputs.rust-overlay.lib.mkRustBin {} rustPkgs;
      selectProfile = toolchains:
        toolchains.minimal.override {
          extensions = [
            "rust-analyzer"
            "rust-src"
            "rustfmt"
            "llvm-tools-preview"
            "clippy"
          ];
          targets = [];
        };
    in
      rust-bin.selectLatestNightlyWith selectProfile;

    craneLib = (inputs.crane.mkLib pkgs).overrideToolchain toolchainFor;
    src = craneLib.cleanCargoSource ../.;
    commonArgs = {
      inherit src;
      strictDeps = true;
      buildInputs = pkgs.lib.optionals pkgs.stdenv.hostPlatform.isDarwin [
        pkgs.libiconv
      ];
    };
    cargoArtifacts = craneLib.buildDepsOnly commonArgs;
  in {
    _module.args = {
      inherit cargoArtifacts commonArgs craneLib src;
    };
  };
}
