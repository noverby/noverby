{
  pkgs,
  inputs,
}:
inputs.devenv.lib.mkShell
{
  inherit inputs pkgs;

  modules = with inputs.self.devenvModules; [
    devenv-root
    git-hooks
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
