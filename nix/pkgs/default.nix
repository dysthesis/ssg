{
  perSystem =
    {
      craneLib,
      pkgs,
      commonArgs,
      cargoArtifacts,
      lib,
      inputs',
      charonToolchain,
      lake2nix,
      ...
    }:
    let
      inherit (pkgs) callPackage;
    in
    {
      ssg = rec {
        package = callPackage ./ssg {
          inherit
            craneLib
            pkgs
            commonArgs
            cargoArtifacts
            ;
        };

        default = package;
      };
    };
}
