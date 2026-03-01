{lib, ...}: {
  zedExtensions.zed-mojo = lib.cleanSource ./.;

  devShells.zed-mojo = pkgs: {
    packages = with pkgs; [
      (rust-bin.stable.latest.default.override {
        targets = ["wasm32-wasip2"];
      })
    ];
  };
}
