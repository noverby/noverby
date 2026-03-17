{
  imports = [
    ../homepage/backend/default.nix
  ];

  devShells.homepage-dioxus = pkgs: {
    packages = with pkgs; [
      just
      dioxus-cli
      wasm-pack
    ];
  };
}
