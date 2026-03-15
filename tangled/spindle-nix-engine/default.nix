{
  devShells.tangled-spindle-nix-engine = pkgs: {
    packages = with pkgs; [
      just
      nix
    ];
  };

  packages.tangled-spindle-nix-engine = {
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
      pname = "tangled-spindle-nix-engine";
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
        homepage = "https://tangled.org/overby.me/overby.me/tree/main/tangled/spindle-nix-engine";
        license = lib.licenses.mit;
        maintainers = with lib.maintainers; [noverby];
        mainProgram = "tangled-spindle";
      };
    };

  nixosModules.tangled-spindle-nix-engine = ./nixos-module.nix;

  checks.tangled-spindle-nix-engine-integration = pkgs:
    import ./nixos-test.nix {
      inherit pkgs;
      tangled-spindle-nix-engine = pkgs.tangled-spindle-nix-engine or (throw "tangled-spindle-nix-engine package not found");
    };
}
