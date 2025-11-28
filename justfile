flake-update:
    nix flake update --option access-tokens "github.com=$(gh auth token)"

gravitas-build:
    nixos-rebuild build --flake .#gravitas --print-build-logs

gravitas-switch:
    sudo nixos-rebuild switch --flake .#gravitas --print-build-logs

gravitas-vetero-build:
    nixos-rebuild build --flake .#gravitas-vetero --print-build-logs

gravitas-vetero-switch:
    sudo nixos-rebuild switch --flake .#gravitas-vetero --print-build-logs