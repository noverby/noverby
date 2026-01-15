{
  pkgs,
  inputs,
}: let
  envJson = builtins.readFile inputs.env.outPath;
  env =
    if envJson != ""
    then builtins.fromJSON envJson
    else {PWD = "/home/noverby/Work/noverby";};
in
  inputs.devenv.lib.mkShell
  {
    inherit inputs pkgs;

    modules = with inputs.self.devenvModules; [
      git-hooks
      {
        devenv.root = env.PWD;
      }
      {
        languages = {
          rust = {
            enable = true;
          };
        };

        packages = with pkgs; [
          # IDE
          harper
          # Common
          just
          rumdl
          # Nix
          nixd
          nil
          alejandra
          (writeShellScriptBin "ragenix" ''
            exec ${ragenix}/bin/ragenix -i ~/.age/id_fido2 "$@"
          '')
          # Rust
          openssl
          # Mojo
          mojo
          python3
          llvmPackages_latest.llvm
          llvmPackages_latest.lld
          # Deno
          deno
          # Media
          cavif-rs
          presenterm
          python313Packages.weasyprint
          # DevOps
          scaleway-cli
        ];
      }
    ];
  }
