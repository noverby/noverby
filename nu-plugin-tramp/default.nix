{
  packages = {
    nu-plugin-tramp = {
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

    tramp-agent = {
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

    tramp-agent-cache = {
      lib,
      runCommand,
      nixpkgs ? null,
    }: let
      cross = import ./cross.nix {inherit nixpkgs lib;};
      linux = cross.allLinuxFrom "x86_64-linux";
    in
      runCommand "tramp-agent-cache" {} ''
        mkdir -p $out/x86_64-unknown-linux-musl
        mkdir -p $out/aarch64-unknown-linux-musl
        cp ${linux.x86_64-linux}/bin/tramp-agent $out/x86_64-unknown-linux-musl/
        cp ${linux.aarch64-linux}/bin/tramp-agent $out/aarch64-unknown-linux-musl/
      '';
  };

  homeModules.nu-plugin-tramp = ./hm-module.nix;
}
