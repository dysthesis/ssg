{inputs, ...}: {
  imports = [inputs.treefmt-nix.flakeModule];
  perSystem = _: {
    treefmt = {
      projectRootFile = "flake.nix";
      programs = {
        alejandra.enable = true;
        rustfmt.enable = true;
      };
    };
  };
}
