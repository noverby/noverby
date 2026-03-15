{
  devShells.rust-h264toav1 = pkgs: {
    packages = with pkgs; [
      just
    ];
  };

  packages.rust-h264toav1 = {
    lib,
    rustPlatform,
  }:
    rustPlatform.buildRustPackage {
      pname = "rust-h264toav1";
      version = "unstable";

      src = lib.fileset.toSource {
        root = ./..;
        fileset = lib.fileset.unions [
          ./Cargo.toml
          ./Cargo.lock
          ./src
          ../h264-decoder/Cargo.toml
          ../h264-decoder/Cargo.lock
          ../h264-decoder/crates
          ../h265-decoder/Cargo.toml
          ../h265-decoder/Cargo.lock
          ../h265-decoder/crates
        ];
      };

      sourceRoot = "source/h264toav1";

      cargoLock.lockFile = ./Cargo.lock;

      doCheck = false;

      meta = {
        description = "A CLI tool to transcode H.264/H.265 video to AV1 using h264-decode, h265-decode, and rav1e";
        homepage = "https://tangled.org/overby.me/overby.me/tree/main/rust/h264toav1";
        license = lib.licenses.mit;
        maintainers = with lib.maintainers; [noverby];
        mainProgram = "h264toav1";
      };
    };
}
