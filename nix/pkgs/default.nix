{
  perSystem =
    {
      craneLib,
      pkgs,
      commonArgs,
      cargoArtifacts,
      ...
    }:
    let
      inherit (pkgs) callPackage;
    in
    {
      packages = rec {
        ssg = callPackage ./ssg {
          inherit
            craneLib
            pkgs
            commonArgs
            cargoArtifacts
            ;
        };
        default = ssg;
      };
    };
}
