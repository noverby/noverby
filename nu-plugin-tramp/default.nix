{
  packages.nu_plugin_tramp = {
    lib,
    rustPlatform,
    openssh,
  }:
    rustPlatform.buildRustPackage {
      pname = "nu_plugin_tramp";
      version = "unstable";

      src = lib.fileset.toSource {
        root = ./.;
        fileset = lib.fileset.unions [
          ./Cargo.toml
          ./Cargo.lock
          ./src
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
}
