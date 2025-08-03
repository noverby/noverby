{inputs, ...}: {
  default = {pkgs}:
    inputs.devenv.lib.mkShell
    {
      inherit inputs pkgs;

      modules = [
        {
          devenv.root = builtins.readFile inputs.devenv-root.outPath;
        }
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
          packages = with pkgs; [
            # Common
            just
            # Nix
            nixd
            nil
            alejandra
            # Rust
            rustup
            # Mojo
            mojo
            python3
            llvmPackages_latest.llvm
            llvmPackages_latest.lld
            # Deno
            deno
            # Media
            cavif-rs
          ];
        }
      ];
    };
}
