{
  imports = [
    ./backend/default.nix
  ];

  devShells.homepage = pkgs: {
    packages = with pkgs; [
      just
      deno
    ];
  };
}
