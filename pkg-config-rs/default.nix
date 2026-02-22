{
  devShells.pkg-config-rs = pkgs: {
    packages = with pkgs; [
      just
    ];
  };

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
        homepage = "https://tangled.org/overby.me/overby.me/tree/main/pkg-config-rs";
        license = lib.licenses.isc;
        maintainers = with lib.maintainers; [noverby];
        mainProgram = "pkgconf";
      };
    };
}
