nixos-build:
    nixos-rebuild build --flake .#gravitas --print-build-logs

nixos-switch:
    sudo nixos-rebuild switch --flake .#gravitas --print-build-logs
