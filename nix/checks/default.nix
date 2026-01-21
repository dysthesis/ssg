{
  perSystem = {
    lib,
    pkgs,
    self',
    advisory-db,
    craneLib,
    commonArgs,
    cargoArtifacts,
    src,
    ...
  }: {
    checks = let
      inherit (lib) fold;
      defaultCheckArgs = {
        inherit
          craneLib
          commonArgs
          cargoArtifacts
          src
          pkgs
          ;
        inherit
          advisory-db
          ;
        inherit self';
      };

      mkCheck = name: {
        "package-${name}" =
          import (./. + "/${name}.nix") defaultCheckArgs;
      };

      checkNames = [
        "clippy"
        "doc"
        "fmt"
        "audit"
        # "deny" # TODO: enable this once licensing is decided
        "nextest"
      ];
    in
      fold
      (curr: acc: acc // mkCheck curr)
      {ssg = self'.packages.default;}
      checkNames;
  };
}
