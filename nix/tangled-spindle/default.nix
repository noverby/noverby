{
  devShells.nix-tangled-spindle = pkgs: {
    packages = with pkgs; [
      just
      nix
    ];
  };

  packages.nix-tangled-spindle = {
    lib,
    rustPlatform,
    makeWrapper,
    nix,
    bash,
    coreutils,
    git,
    gnutar,
    gzip,
  }:
    rustPlatform.buildRustPackage {
      pname = "nix-tangled-spindle";
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

      nativeBuildInputs = [
        git
        makeWrapper
      ];

      # Runtime dependencies needed by the nix engine for step execution
      postInstall = let
        runtimePath = lib.makeBinPath [
          bash
          coreutils
          git
          gnutar
          gzip
          nix
        ];
      in ''
        wrapProgram $out/bin/tangled-spindle \
          --prefix PATH : ${runtimePath}
        wrapProgram $out/bin/spindle-run \
          --prefix PATH : ${runtimePath}
      '';

      doCheck = true;

      meta = {
        description = "Rust reimplementation of the Tangled Spindle CI runner with native Nix engine";
        homepage = "https://tangled.org/overby.me/overby.me/tree/main/nix/tangled-spindle";
        license = lib.licenses.mit;
        maintainers = with lib.maintainers; [noverby];
        mainProgram = "tangled-spindle";
      };
    };

  nixosModules.nix-tangled-spindle = ./nixos-module.nix;

  checks.nix-tangled-spindle-integration = pkgs:
    import ./nixos-test.nix {
      inherit pkgs;
      nix-tangled-spindle = pkgs.nix-tangled-spindle or (throw "nix-tangled-spindle package not found");
    };
}
