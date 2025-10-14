{inputs, ...}: {
  default = {pkgs}:
    inputs.devenv.lib.mkShell
    {
      inherit inputs pkgs;

      modules = [
        {
          git-hooks.hooks = {
            denolint.enable = true;
            biome.enable = true;
            alejandra.enable = true;
            statix.enable = true;
            typos.enable = true;
            commitlint-rs = {
              enable = true;
              package = pkgs.commitlint-rs;
              name = "prepare-commit-msg-commitlint-rs";
              entry = "${pkgs.commitlint-rs}/bin/commitlint --edit";
              stages = ["prepare-commit-msg"];
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
            # Nix
            nixd
            nil
            alejandra
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
    };
}
