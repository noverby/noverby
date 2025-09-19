{inputs, ...}: [
  (import inputs.rust-overlay)
  (import ./lib.nix)
  (import ./xdg-desktop-portal-cosmic.nix)
]
