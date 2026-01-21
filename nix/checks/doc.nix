{
  craneLib,
  commonArgs,
  cargoArtifacts,
  ...
}:
craneLib.cargoDoc (
  commonArgs
  // {
    inherit cargoArtifacts;
    env.RUSTDOCFLAGS = "--deny warnings";
  }
)
