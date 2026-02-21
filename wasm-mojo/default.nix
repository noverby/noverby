{
  devShells.wasm-mojo = pkgs: {
    packages = with pkgs; [
      just
      mojo
      deno
      wabt
      llvmPackages_latest.llvm
      llvmPackages_latest.lld
      wasmtime.lib
      wasmtime.dev
    ];
  };
}
