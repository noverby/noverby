{
  devShells.mojo-gui = pkgs: let
    # Rust toolchain with Windows cross-compilation target (from rust-overlay).
    # This is NOT added to PATH (to avoid shadowing the devenv-provided Rust).
    # Instead, its sysroot is exposed via the `rust-sysroot-windows` helper script
    # so that cross-compilation recipes can pass --sysroot to rustc.
    rustWithWindows = pkgs.rust-bin.stable.latest.default.override {
      extensions = ["rust-src"];
      targets = ["x86_64-unknown-linux-gnu" "x86_64-pc-windows-gnu"];
    };
  in {
    packages = with pkgs; [
      # Build tools
      just
      mojo
      mojo-windows

      # Web renderer (WASM + TypeScript)
      deno
      wabt
      llvmPackages_latest.llvm
      llvmPackages_latest.lld
      wasmtime.lib
      wasmtime.dev
      servo
      jq

      # Desktop renderer (Blitz shim build)
      pkg-config
      cmake
      python3

      # Desktop renderer (Wayland + GPU runtime deps)
      fontconfig
      freetype
      libxkbcommon
      wayland
      vulkan-loader
      vulkan-headers
      libGL

      # Windows cross-compilation (MinGW-w64 linker for x86_64-pc-windows-gnu)
      # Strip nix-support/ to avoid setup hooks that set CC/AR/etc. to cross names,
      # which would break native builds. We only need the binaries on PATH.
      (symlinkJoin {
        name = "mingw-w64-cc-noenv";
        paths = [pkgsCross.mingwW64.stdenv.cc];
        postBuild = "rm -rf $out/nix-support";
      })

      # Windows verification (Wine)
      wine64Packages.stable

      # Helper: prints the Rust sysroot path that includes x86_64-pc-windows-gnu std.
      # Usage in justfile: _rust-sysroot-windows := `rust-sysroot-windows`
      (writeShellScriptBin "rust-sysroot-windows" ''
        echo -n "${rustWithWindows}"
      '')

      # Helper: prints the MinGW-w64 library path (contains libpthread.a etc.)
      # Usage in justfile: _mingw-lib-path := `mingw-lib-path`
      (writeShellScriptBin "mingw-lib-path" ''
        echo -n "${pkgsCross.mingwW64.windows.pthreads}/lib"
      '')

      # Helper: prints the MinGW-w64 mcfgthread library path
      # Usage in justfile: _mingw-mcf-lib-path := `mingw-mcf-lib-path`
      (writeShellScriptBin "mingw-mcf-lib-path" ''
        echo -n "${pkgsCross.mingwW64.windows.mcfgthreads}/lib"
      '')
    ];
  };
}
