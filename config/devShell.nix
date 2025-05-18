{
  inputs,
  pkgs,
  ...
}:
inputs.devenv.lib.mkShell
{
  inherit inputs pkgs;

  modules = [
    {
      devenv.root = builtins.readFile inputs.devenv-root.outPath;
    }
    {
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
        # Nodejs
        yarn
        nodejs
      ];
    }
  ];
}
