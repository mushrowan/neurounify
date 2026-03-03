{
  craneLib,
  src,
}: let
  commonArgs = {
    inherit src;
    pname = "neurounify";
    version = "0.1.0";
    strictDeps = true;
  };

  cargoArtifacts = craneLib.buildDepsOnly commonArgs;

  neurounify = craneLib.buildPackage (commonArgs
    // {
      inherit cargoArtifacts;
    });

  clippy = craneLib.cargoClippy (commonArgs
    // {
      inherit cargoArtifacts;
      cargoClippyExtraArgs = "--all-targets -- --deny warnings";
    });

  test = craneLib.cargoNextest (commonArgs
    // {
      inherit cargoArtifacts;
      partitions = 1;
      partitionType = "count";
      cargoNextestExtraArgs = "--no-tests=warn";
    });

  fmt = craneLib.cargoFmt {inherit src;};
in {
  inherit neurounify clippy test fmt cargoArtifacts;
}
