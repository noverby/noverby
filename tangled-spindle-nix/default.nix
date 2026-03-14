{
  devShells.tangled-spindle-nix = pkgs: {
    packages = with pkgs; [
      just
      nix
    ];
  };

  packages.tangled-spindle-nix = {
    lib,
    rustPlatform,
    makeWrapper,
    nix,
    bash,
    coreutils,
    git,
    gnutar,
    gzip,
    pkg-config,
    openssl,
  }:
    rustPlatform.buildRustPackage {
      pname = "tangled-spindle-nix";
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
        pkg-config
      ];

      buildInputs = [
        openssl
      ];

      # Runtime dependencies needed by the nix engine for step execution
      postInstall = ''
        wrapProgram $out/bin/tangled-spindle \
          --prefix PATH : ${lib.makeBinPath [
          bash
          coreutils
          git
          gnutar
          gzip
          nix
        ]}
      '';

      doCheck = true;

      meta = {
        description = "Rust reimplementation of the Tangled Spindle CI runner with native Nix engine";
        homepage = "https://tangled.org/overby.me/overby.me/tree/main/tangled-spindle-nix";
        license = lib.licenses.mit;
        maintainers = with lib.maintainers; [noverby];
        mainProgram = "tangled-spindle";
      };
    };

  nixosModules.tangled-spindle-nix = ./nixos-module.nix;

  checks.tangled-spindle-nix-integration = pkgs:
    import ./nixos-test.nix {
      inherit pkgs;
      tangled-spindle-nix = pkgs.tangled-spindle-nix or (throw "tangled-spindle-nix package not found");
    };
}
