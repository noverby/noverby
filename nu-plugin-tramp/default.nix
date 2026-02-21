{
  packages.nu-plugin-tramp = {
    lib,
    rustPlatform,
    openssh,
  }:
    rustPlatform.buildRustPackage {
      pname = "nu-plugin-tramp";
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

      nativeCheckInputs = [
        openssh
      ];

      doCheck = false;

      meta = {
        description = "A TRAMP-inspired remote filesystem plugin for Nushell";
        homepage = "https://tangled.org/overby.me/overby.me/tree/main/nu-plugin-tramp";
        license = lib.licenses.mit;
        maintainers = with lib.maintainers; [noverby];
        mainProgram = "nu_plugin_tramp";
      };
    };

  packages.tramp-agent = {
    lib,
    rustPlatform,
  }:
    rustPlatform.buildRustPackage {
      pname = "tramp-agent";
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

      cargoBuildFlags = ["-p" "tramp-agent"];
      cargoTestFlags = ["-p" "tramp-agent"];

      # Use the size-optimised release profile for the agent binary
      CARGO_PROFILE = "release-agent";

      meta = {
        description = "Lightweight RPC agent for nu-plugin-tramp remote filesystem operations";
        homepage = "https://tangled.org/overby.me/overby.me/tree/main/nu-plugin-tramp";
        license = lib.licenses.mit;
        maintainers = with lib.maintainers; [noverby];
        mainProgram = "tramp-agent";
      };
    };

  homeModules.nu-plugin-tramp = ./hm-module.nix;
}
