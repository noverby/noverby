{
  devShells.wiki-dioxus = pkgs: {
    packages = with pkgs; [
      just
      cargo
      rustc
      dioxus-cli
      wasm-pack
      binaryen
    ];
  };
}
