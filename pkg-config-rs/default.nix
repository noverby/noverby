{
  packages.pkg-config-rs = {
    lib,
    rustPlatform,
  }:
    rustPlatform.buildRustPackage {
      pname = "pkg-config-rs";
      version = "unstable";

      src = lib.fileset.toSource {
        root = ./.;
        fileset = lib.fileset.unions [
          ./Cargo.toml
          ./Cargo.lock
          ./crates
          ./tests
        ];
      };

      cargoLock.lockFile = ./Cargo.lock;

      setupHook = ./setup-hook.sh;

      postInstall = ''
        ln -s $out/bin/pkgconf $out/bin/pkg-config
      '';

      meta = {
        description = "A pure Rust rewrite and drop-in replacement for pkg-config/pkgconf";
        homepage = "https://github.com/noverby/pkg-config-rs";
        license = lib.licenses.isc;
        maintainers = with lib.maintainers; [noverby];
        mainProgram = "pkgconf";
      };
    };
}
