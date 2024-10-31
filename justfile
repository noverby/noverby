nixos-build:
    sudo nixos-rebuild build --flake .#gravitas

nixos-switch:
    sudo nixos-rebuild switch --flake .#gravitas
