{
  pkgs,
  inputs,
}:
inputs.devenv.lib.mkShell
{
  inherit inputs pkgs;

  modules = [
    {
      git-hooks = {
        package = pkgs.prek;
        hooks = {
          denolint.enable = true;
          biome.enable = true;
          alejandra.enable = true;
          statix.enable = true;
          typos.enable = true;
          rustfmt.enable = true;
          commitlint-rs = {
            enable = true;
            package = pkgs.commitlint-rs;
            name = "prepare-commit-msg-commitlint-rs";
            entry = "${pkgs.commitlint-rs}/bin/commitlint --edit";
            stages = ["prepare-commit-msg"];
          };
        };
      };

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
