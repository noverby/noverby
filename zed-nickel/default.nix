{
  packages.zed-nickel = {lib}:
    lib.cleanSource ./.;

  devShells.zed-nickel = pkgs: {
    packages = with pkgs; [
      (rust-bin.stable.latest.default.override {
        targets = ["wasm32-wasip2"];
      })
    ];
  };
}
