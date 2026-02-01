{
  devShells.mojo-wasm = pkgs: {
    packages = with pkgs; [
      just
      mojo
      python3
      llvmPackages_latest.llvm
      llvmPackages_latest.lld
    ];
  };
}
