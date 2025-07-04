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
            biome.enable = true;
            alejandra.enable = true;
            statix.enable = true;
          };
          packages = with pkgs; [
            # Common
            just
            # Nix
            nixd
            nil
            alejandra
            # Rust
            rustc
            cargo
            # Mojo
            mojo
            python3
            llvmPackages_latest.llvm
            llvmPackages_latest.lld
            # Deno
            deno
          ];
        }
      ];
    };
}
