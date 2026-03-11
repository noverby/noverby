{
  devShells.mojo-gui = pkgs: {
    packages = with pkgs; [
      # Build tools
      just
      mojo

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
      rustup
      pkg-config
      cmake
      python3

      # Desktop renderer (GPU + windowing runtime deps)
      fontconfig
      freetype
      libxkbcommon
      wayland
      vulkan-loader
      vulkan-headers
      libGL
      xorg.libX11
      xorg.libXcursor
      xorg.libXrandr
      xorg.libXi
      xorg.libxcb
    ];
  };
}
