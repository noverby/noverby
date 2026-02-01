{
  devShells.homepage = pkgs: {
    packages = with pkgs; [
      just
      deno
    ];
  };
}
