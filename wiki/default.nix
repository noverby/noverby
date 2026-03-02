{
  devShells.wiki = pkgs: {
    packages = with pkgs; [
      just
      deno
    ];
  };

  devShells.wiki-server = pkgs: {
    packages = with pkgs; [
      just
      deno
      postgresql
    ];
  };
}
