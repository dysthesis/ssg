{
  perSystem = {
    config,
    craneLib,
    pkgs,
    ...
  }: {
    devShells.default = craneLib.devShell {
      inherit (config) checks;
      packages = with pkgs; [
        # Rust
        cargo-mutants
        bacon
        # Nix
        statix
        deadnix
        alejandra
        jq
      ];
    };
  };
}
