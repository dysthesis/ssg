{inputs, ...}: {
  perSystem = {
    config,
    craneLib,
    pkgs,
    lib,
    system,
    ...
  }: let
    # compatibility with ugdb
    gdb16 =
      (import inputs.nixpkgs-gdb16 {inherit system;}).gdb;
  in {
    devShells.default = craneLib.devShell {
      inherit (config) checks;
      packages = with pkgs; [
        # Rust
        cargo-mutants
        cargo-nextest
        bacon
        mold # faster linker for dev builds
        clang

        ## Debugging
        ugdb
        rr
        gdb16

        # Misc
        just

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
      RUST_GDB = lib.getExe gdb16;
    };
  };
}
