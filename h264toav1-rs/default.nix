{
  devShells.h264toav1-rs = pkgs: {
    packages = with pkgs; [
      just
    ];
  };

  packages.h264toav1-rs = {
    lib,
    rustPlatform,
  }:
    rustPlatform.buildRustPackage {
      pname = "h264toav1";
      version = "unstable";

      src = lib.fileset.toSource {
        root = ./..;
        fileset = lib.fileset.unions [
          ./Cargo.toml
          ./Cargo.lock
          ./src
          ../h264-decoder-rs/Cargo.toml
          ../h264-decoder-rs/Cargo.lock
          ../h264-decoder-rs/crates
        ];
      };

      sourceRoot = "source/h264toav1-rs";

      cargoLock.lockFile = ./Cargo.lock;

      doCheck = false;

      meta = {
        description = "A CLI tool to transcode H.264 video to AV1 using h264-decode and rav1e";
        homepage = "https://tangled.org/overby.me/overby.me/tree/main/h264toav1-rs";
        license = lib.licenses.mit;
        maintainers = with lib.maintainers; [noverby];
        mainProgram = "h264toav1";
      };
    };
}
