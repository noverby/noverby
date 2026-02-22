{
  packages.nix-workspace = {
    lib,
    rustPlatform,
  }:
    rustPlatform.buildRustPackage {
      pname = "nix-workspace";
      version = "0.5.0";

      src = lib.fileset.toSource {
        root = ./cli;
        fileset = lib.fileset.unions [
          ./cli/Cargo.toml
          ./cli/Cargo.lock
          ./cli/src
        ];
      };

      cargoLock.lockFile = ./cli/Cargo.lock;

      meta = {
        description = "A Nickel-powered workspace manager for Nix flakes";
        homepage = "https://tangled.org/overby.me/overby.me/tree/main/nix-workspace";
        license = lib.licenses.mit;
        maintainers = with lib.maintainers; [noverby];
        mainProgram = "nix-workspace";
      };
    };

  devShells.nix-workspace = pkgs: {
    # pkgsUnstable needed — stable nixpkgs lags behind on nickel/nls versions
    packages = with pkgs.pkgsUnstable; [
      nickel
      nls
    ];
  };
}
