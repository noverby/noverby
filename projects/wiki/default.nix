{
  devShells.wiki = pkgs: {
    packages = with pkgs; [
      just
      deno
    ];
  };
}
