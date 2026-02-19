{
  devShells.wasm-mojo = pkgs: {
    packages = with pkgs; [
      just
      mojo
      deno
      llvmPackages_latest.llvm
      llvmPackages_latest.lld
    ];
  };
}
