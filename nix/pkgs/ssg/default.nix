{
  craneLib,
  cargoArtifacts,
  commonArgs,
  ...
}:
craneLib.buildPackage (
  commonArgs
  // {
    inherit cargoArtifacts;
    pname = "template";
    CARGO_PROFILE = "release";
  }
)
