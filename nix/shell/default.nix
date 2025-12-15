{
  perSystem = {
    pkgs,
    craneLib,
    ...
  }: let
    valeConfigured = pkgs.callPackage ./vale {};
  in {
    devShells.default = craneLib.devShell {
      packages = with pkgs; [
        # Nix
        nixd
        statix
        deadnix
        alejandra

        # Rust
        cargo-audit
        cargo-expand
        cargo-nextest
        rust-analyzer
        cargo-wizard
        cargo-tarpaulin
        cargo-bloat
        bacon

        # Prose
        valeConfigured
      ];
    };
  };
}
