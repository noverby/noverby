{
  devShells.nix-workspace = pkgs: {
    # pkgsUnstable needed — stable nixpkgs lags behind on nickel/nls versions
    packages = with pkgs.pkgsUnstable; [
      nickel
      nls
    ];
  };
}
