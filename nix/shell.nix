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
      shellHook = ''
        printf '\n'
        printf '%*s\n' "$(tput cols)" ''' | tr ' ' '-'
        ${../scripts/todo}
        printf '%*s\n' "$(tput cols)" ''' | tr ' ' '-'
        printf '\n'
      '';
    };
  };
}
