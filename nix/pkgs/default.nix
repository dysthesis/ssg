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
    }: let
      inherit (pkgs) callPackage;
    in { packages = rec {
        ssg = callPackage ./ssg {
          inherit
            craneLib
            pkgs
            commonArgs
            cargoArtifacts
            ;
        };



        default = ssg;
    };};
}
