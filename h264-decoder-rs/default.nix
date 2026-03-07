{
  devShells.h264-decoder-rs = pkgs: {
    packages = with pkgs; [
      just
    ];
  };

  packages.h264-decoder-rs = {
    lib,
    rustPlatform,
  }:
    rustPlatform.buildRustPackage {
      pname = "h264-decoder-rs";
      version = "unstable";

      src = lib.fileset.toSource {
        root = ./.;
        fileset = lib.fileset.unions [
          ./Cargo.toml
          ./Cargo.lock
          ./crates
        ];
      };

      cargoLock.lockFile = ./Cargo.lock;

      doCheck = false;

      meta = {
        description = "A pure Rust H.264 decoder library";
        homepage = "https://tangled.org/overby.me/overby.me/tree/main/h264-decoder-rs";
        license = lib.licenses.mit;
        maintainers = with lib.maintainers; [noverby];
        mainProgram = "h264-decode";
      };
    };
}
