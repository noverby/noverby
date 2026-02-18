(final: prev: {
  # Add unstable packages to attributeset to pkgs.pkgsUnstable
  pkgsUnstable =
    (import prev.inputs.nixpkgs-unstable {
      inherit (final) system;
      config.allowUnfree = true;
    })
    // prev.inputs.self.packages.${final.system};
})
