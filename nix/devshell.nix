{
  pkgs,
  craneLib,
  checks ? {},
  cargoArtifacts ? null,
}:
craneLib.devShell {
  inherit checks cargoArtifacts;

  packages = with pkgs; [
    cargo-edit
    cargo-machete
    cargo-watch
    cargo-nextest
  ];

  RUST_LOG = "debug";
}
