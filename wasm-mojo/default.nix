{
  devShells.wasm-mojo = pkgs: {
    packages = with pkgs; [
      just
      mojo
      python3
      deno
      llvmPackages_latest.llvm
      llvmPackages_latest.lld
    ];
  };
}
